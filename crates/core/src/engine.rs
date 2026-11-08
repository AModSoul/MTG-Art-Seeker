//! A download run: turns an input into jobs, one per card asked for, and works
//! through them a few at a time, reporting each job's progress as it goes.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Instant, SystemTime},
};

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{
    card::Card,
    carddb::{CardDb, NameQuery},
    decks::deck_cards,
    faces::{plan_faces, whole_card, FacePlan},
    history::{History, Remembered},
    http::{Http, ImageKind},
    imagesize::{image_size, ImageSize},
    list::{parse_list, CardLine, SkippedLine},
    manual::{refetch_found_copy, FoundCopyAt, Refetched},
    mtgpics::{self, MtgpicsArt},
    naming::{file_stem, sanitize, NameParts},
    presets::preset_names,
    scryfall::{self, Direction, Identifier, SearchOptions, Unique},
    settings::{Printing, Settings, SourceMode},
    Result,
};

/// How many cards are worked on at once. The rate limits set the real pace;
/// this only keeps a few downloads streaming while lookups wait their turn.
pub const CONCURRENCY: usize = 4;
/// The most cards a Scryfall search is followed to.
pub const MAX_SEARCH_CARDS: usize = 5000;
/// The most printings fetched for one name (basic lands have hundreds of arts).
const MAX_PRINTINGS: usize = 1750;
/// How many cards a page of Scryfall search results holds.
const SEARCH_PAGE: usize = 175;

/// What to download.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RunInput {
    /// A card list, one card per line. With [`Settings::list_folders`] on, its
    /// art is saved in a folder of this name inside the output folder.
    List {
        text: String,
        #[serde(default)]
        #[ts(optional)]
        folder: Option<String>,
    },
    /// Every card a Scryfall search matches.
    Search { query: String },
    /// A ready-made list, by [`crate::presets::PresetInfo::id`].
    Preset { id: String },
    /// A preconstructed deck's cards, by [`crate::decks::DeckSummary::file_name`].
    /// With [`Settings::list_folders`] on, its art is saved in a folder of
    /// this name inside the output folder.
    Deck {
        #[serde(rename = "fileName")]
        file_name: String,
        #[serde(default)]
        #[ts(optional)]
        folder: Option<String>,
    },
}

impl RunInput {
    /// The folder inside the output folder a list or deck asked to be saved in.
    pub fn folder(&self) -> Option<&str> {
        match self {
            Self::List { folder, .. } | Self::Deck { folder, .. } => folder.as_deref(),
            Self::Search { .. } | Self::Preset { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "event", rename_all = "camelCase")]
#[ts(export)]
pub enum RunEvent {
    /// The jobs a run will work through, all queued.
    Planned {
        jobs: Vec<JobInfo>,
        skipped: Vec<SkippedLine>,
        /// The folder this run saves to.
        folder: String,
    },
    /// A job's status changed.
    Job { id: u32, status: JobStatus },
    /// The run couldn't start: a search failed, say.
    Error { message: String },
    /// The run is over, cancelled or not. Always the last event.
    Finished { summary: RunSummary },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobInfo {
    pub id: u32,
    /// `Name (SET) 123`, or as much of it as the line gave.
    pub label: String,
    /// A line the list parser reads back as this card, for retrying it.
    pub line: String,
    /// How many images it should save, when its card is known before it starts.
    pub images: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum JobStatus {
    Queued,
    /// Looking the card up on Scryfall.
    Resolving,
    /// Finding and saving its art: `images` of them, unless some can't be found.
    Downloading {
        images: u32,
    },
    /// At least one image saved. `missing` names faces that weren't.
    Done {
        files: Vec<SavedFile>,
        missing: Vec<String>,
    },
    Failed {
        error: String,
    },
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ArtSource {
    Mtgpics,
    Scryfall,
    /// An image the user found themselves, anywhere on the web, and saved in
    /// place of Scryfall's; see [`crate::manual`].
    Web,
}

impl ArtSource {
    /// The folder its art goes in, with [`Settings::source_folders`] on.
    fn folder(self) -> &'static str {
        match self {
            Self::Mtgpics => "mtgpics",
            Self::Scryfall => "scryfall",
            Self::Web => "web",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SavedFile {
    pub path: String,
    /// The name of the face this is the art of.
    pub face: String,
    /// Which of the card's images it is, from 0; see [`FacePlan::index`].
    pub face_index: u32,
    pub source: ArtSource,
    pub set: String,
    pub number: String,
    pub artist: String,
    /// The image's size in pixels, if its header could be read.
    pub size: Option<ImageSize>,
    /// The page a [`ArtSource::Web`] image was saved from.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub page: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunSummary {
    pub jobs: u32,
    /// Jobs with every image saved.
    pub done: u32,
    /// Jobs with some images saved and some missing.
    pub partial: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub mtgpics_files: u32,
    pub scryfall_files: u32,
    /// Images the user found themselves, saved again from where they were.
    pub web_files: u32,
    pub seconds: f64,
}

/// What a job was asked for.
enum Request {
    /// A card from a list. `card` is filled in by the batched lookup.
    Line { line: CardLine, card: Option<Card> },
    /// A card a search already found.
    Card(Card),
}

struct Job {
    id: u32,
    request: Request,
}

/// Everything a job reads while it runs.
struct Context<'a> {
    http: &'a Http,
    settings: &'a Settings,
    /// Scryfall's card data, kept locally; asked before Scryfall itself is.
    cards: Option<&'a CardDb>,
    /// Where each printing's MTGPics art was found before.
    history: Option<&'a History>,
    /// The folder art is saved under: the output folder, or a list's folder in it.
    output_dir: PathBuf,
    cache: mtgpics::Cache,
    paths: PathReserver,
    emit: &'a (dyn Fn(RunEvent) + Sync),
}

/// What a run reads, and keeps, on disk between runs. Without either, it asks
/// Scryfall and MTGPics about everything.
#[derive(Clone, Copy, Default)]
pub struct Local<'a> {
    /// The local copy of Scryfall's card data. A list's cards are looked up in
    /// it; Scryfall is asked about the rest: cards newer than the copy,
    /// misspelt names, and every card when there's no copy yet.
    pub cards: Option<&'a CardDb>,
    /// The log of where each printing's MTGPics art was found, so it's saved
    /// from there next time rather than found again.
    pub history: Option<&'a History>,
}

/// Runs `input` to the end, or until `cancel` is cancelled, sending each change
/// to `emit`. The last event is always [`RunEvent::Finished`].
pub async fn run(
    http: &Http,
    input: RunInput,
    settings: &Settings,
    local: Local<'_>,
    cancel: &CancellationToken,
    emit: &(dyn Fn(RunEvent) + Sync),
) -> RunSummary {
    let started = Instant::now();
    let output_dir = output_dir(&input, settings);
    let planned = tokio::select! {
        biased;
        _ = cancel.cancelled() => Ok(None),
        planned = plan(http, input, settings) => planned.map(Some),
    };
    let (mut jobs, skipped) = match planned {
        Ok(Some(planned)) => planned,
        Ok(None) => return finish(Vec::new(), started, emit),
        Err(error) => {
            emit(RunEvent::Error {
                message: error.to_string(),
            });
            return finish(Vec::new(), started, emit);
        }
    };
    // Cards a list names a set for are found in the card data, before the
    // jobs are announced so each says how many images it will save; the rest
    // are looked up on Scryfall together, 75 to a request.
    let mut batched: Vec<(usize, Identifier)> = Vec::new();
    for (index, job) in jobs.iter_mut().enumerate() {
        let Request::Line { line, card } = &mut job.request else {
            continue;
        };
        let Some(id) = identifier(line) else {
            continue;
        };
        match local.cards.and_then(|db| find_local(db, &id)) {
            Some(found) => *card = Some(found),
            None => batched.push((index, id)),
        }
    }
    emit(RunEvent::Planned {
        jobs: jobs.iter().map(|job| job.info(settings)).collect(),
        skipped,
        folder: output_dir.to_string_lossy().into_owned(),
    });

    if !batched.is_empty() {
        for (index, _) in &batched {
            emit(RunEvent::Job {
                id: jobs[*index].id,
                status: JobStatus::Resolving,
            });
        }
        let identifiers: Vec<Identifier> = batched.iter().map(|(_, id)| id.clone()).collect();
        let found = tokio::select! {
            biased;
            _ = cancel.cancelled() => Vec::new(),
            found = scryfall::collection(http, &identifiers) => found,
        };
        for ((index, _), card) in batched.iter().zip(found) {
            if let Request::Line { card: slot, .. } = &mut jobs[*index].request {
                *slot = card;
            }
        }
    }

    let context = Context {
        http,
        settings,
        cards: local.cards,
        history: local.history,
        output_dir,
        cache: mtgpics::Cache::default(),
        paths: PathReserver::default(),
        emit,
    };
    let statuses = stream::iter(jobs)
        .map(|job| {
            let context = &context;
            async move {
                let id = job.id;
                let status = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => JobStatus::Cancelled,
                    status = process(job, context) => status,
                };
                emit(RunEvent::Job {
                    id,
                    status: status.clone(),
                });
                status
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    finish(statuses, started, emit)
}

/// How many cards a Scryfall search would download, with these settings.
pub async fn count_search(http: &Http, query: &str, settings: &Settings) -> Result<u32> {
    scryfall::count(http, query, &search_options(settings, None)).await
}

fn finish(statuses: Vec<JobStatus>, started: Instant, emit: &(dyn Fn(RunEvent) + Sync)) -> RunSummary {
    let mut summary = RunSummary {
        jobs: statuses.len() as u32,
        seconds: started.elapsed().as_secs_f64(),
        ..RunSummary::default()
    };
    for status in &statuses {
        match status {
            JobStatus::Done { files, missing } => {
                if missing.is_empty() {
                    summary.done += 1;
                } else {
                    summary.partial += 1;
                }
                for file in files {
                    match file.source {
                        ArtSource::Mtgpics => summary.mtgpics_files += 1,
                        ArtSource::Scryfall => summary.scryfall_files += 1,
                        ArtSource::Web => summary.web_files += 1,
                    }
                }
            }
            JobStatus::Failed { .. } => summary.failed += 1,
            JobStatus::Cancelled => summary.cancelled += 1,
            JobStatus::Queued | JobStatus::Resolving | JobStatus::Downloading { .. } => {}
        }
    }
    emit(RunEvent::Finished {
        summary: summary.clone(),
    });
    summary
}

async fn plan(http: &Http, input: RunInput, settings: &Settings) -> Result<(Vec<Job>, Vec<SkippedLine>)> {
    let numbered =
        |requests: Vec<Request>| -> Vec<Job> { (1..).zip(requests).map(|(id, request)| Job { id, request }).collect() };
    let lines = |lines: Vec<CardLine>| {
        lines
            .into_iter()
            .map(|line| Request::Line { line, card: None })
            .collect()
    };
    Ok(match input {
        RunInput::List { text, .. } => {
            let parsed = parse_list(&text);
            (numbered(lines(parsed.cards)), parsed.skipped)
        }
        RunInput::Preset { id } => {
            let names = preset_names(http, &id).await?;
            (
                numbered(lines(names.into_iter().map(CardLine::named).collect())),
                Vec::new(),
            )
        }
        RunInput::Deck { file_name, .. } => {
            let deck = deck_cards(http, &file_name).await?;
            (numbered(lines(deck.lines)), Vec::new())
        }
        RunInput::Search { query } => {
            let results = scryfall::search(http, &query, &search_options(settings, None), MAX_SEARCH_CARDS).await?;
            (
                numbered(results.cards.into_iter().map(Request::Card).collect()),
                Vec::new(),
            )
        }
    })
}

impl Job {
    fn info(&self, settings: &Settings) -> JobInfo {
        let (label, line, card) = match &self.request {
            Request::Line { line, card } => (line.label(), line.line.clone(), card.as_ref()),
            Request::Card(card) => (card.label(), card.label(), Some(card)),
        };
        JobInfo {
            id: self.id,
            label,
            line,
            images: card.map(|card| image_count(std::slice::from_ref(card), settings)),
        }
    }
}

/// How many images saving `cards` should make: one per face that has its own art.
fn image_count(cards: &[Card], settings: &Settings) -> u32 {
    let faces: usize = cards.iter().map(|card| plan_faces(card, settings).len()).sum();
    u32::try_from(faces).unwrap_or(u32::MAX)
}

/// Where a run saves: the output folder, or for a list or deck with
/// [`Settings::list_folders`] on and a folder name, that folder inside it.
fn output_dir(input: &RunInput, settings: &Settings) -> PathBuf {
    output_dir_for(input.folder(), settings)
}

/// Where a run with these settings saves, for a list named `list_folder` or
/// for any other input.
pub fn output_dir_for(list_folder: Option<&str>, settings: &Settings) -> PathBuf {
    let base = PathBuf::from(&settings.output_dir);
    match list_folder {
        Some(folder) if settings.list_folders && !folder.trim().is_empty() => base.join(sanitize(folder)),
        _ => base,
    }
}

/// What to batch-look-up a line as: lines that give a set.
fn identifier(line: &CardLine) -> Option<Identifier> {
    let set = line.set.clone()?;
    Some(match &line.number {
        Some(number) => Identifier::SetNumber {
            set,
            collector_number: number.clone(),
        },
        None => Identifier::NameSet {
            name: line.name.clone(),
            set,
        },
    })
}

/// A line that gives a set, found in the card data. A miss, or a database
/// that can't be read, leaves it to Scryfall.
fn find_local(db: &CardDb, id: &Identifier) -> Option<Card> {
    match id {
        Identifier::SetNumber { set, collector_number } => db.by_number(set, collector_number),
        Identifier::NameSet { name, set } => db.by_name_in_set(name, set),
        Identifier::Name { .. } => return None,
    }
    .ok()
    .flatten()
}

fn search_options(settings: &Settings, by_release: Option<Direction>) -> SearchOptions {
    SearchOptions {
        unique: if settings.unique_art {
            Unique::Art
        } else {
            Unique::Prints
        },
        include_extras: settings.include_extras,
        by_release,
    }
}

async fn process(job: Job, context: &Context<'_>) -> JobStatus {
    let emit = |status| (context.emit)(RunEvent::Job { id: job.id, status });
    let cards = match job.request {
        Request::Card(card) => vec![card],
        Request::Line { card: Some(card), .. } => vec![card],
        Request::Line { line, card: None } => {
            emit(JobStatus::Resolving);
            match resolve(&line, context).await {
                Ok(cards) if cards.is_empty() => {
                    return JobStatus::Failed {
                        error: "Not found on Scryfall".into(),
                    }
                }
                Ok(cards) => cards,
                Err(error) => {
                    return JobStatus::Failed {
                        error: format!("Scryfall: {error}"),
                    }
                }
            }
        }
    };

    emit(JobStatus::Downloading {
        images: image_count(&cards, context.settings),
    });
    let mut files = Vec::new();
    let mut missing = Vec::new();
    let mut errors = Vec::new();
    for card in &cards {
        download_card(card, context, &mut files, &mut missing, &mut errors).await;
    }
    if files.is_empty() {
        JobStatus::Failed {
            error: errors
                .into_iter()
                .next()
                .unwrap_or_else(|| "No art found on MTGPics or Scryfall".into()),
        }
    } else {
        JobStatus::Done { files, missing }
    }
}

/// The printings a list line asks for.
async fn resolve(line: &CardLine, context: &Context<'_>) -> Result<Vec<Card>> {
    let http = context.http;
    let settings = context.settings;
    // A line with a set the batch couldn't match: maybe a misspelling.
    if line.set.is_some() {
        return Ok(scryfall::named_fuzzy(http, &line.name, line.set.as_deref())
            .await?
            .into_iter()
            .collect());
    }
    let direction = match settings.printing {
        Printing::Newest => Direction::Newest,
        Printing::Oldest => Direction::Oldest,
    };
    let local = NameQuery {
        unique_art: settings.unique_art,
        include_extras: settings.include_extras,
        newest_first: direction == Direction::Newest,
    };
    let mut cards = context
        .cards
        .and_then(|db| db.printings(&line.name, local).ok())
        .unwrap_or_default();
    if cards.is_empty() {
        // Only as many as are kept: a basic land has hundreds of printings, at
        // half a second a page.
        let wanted = if settings.download_all {
            MAX_PRINTINGS
        } else if settings.exclude_full_art {
            SEARCH_PAGE
        } else {
            1
        };
        let query = scryfall::exact_name_query(&line.name);
        cards = scryfall::search(http, &query, &search_options(settings, Some(direction)), wanted)
            .await?
            .cards;
    }
    if cards.is_empty() {
        cards.extend(scryfall::named_fuzzy(http, &line.name, None).await?);
    }
    if settings.exclude_full_art {
        cards.retain(|card| !card.full_art);
    }
    if !settings.download_all {
        cards.truncate(1);
    }
    Ok(cards)
}

async fn download_card(
    card: &Card,
    context: &Context<'_>,
    files: &mut Vec<SavedFile>,
    missing: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let settings = context.settings;
    let faces = plan_faces(card, settings);
    let use_mtgpics = settings.sources != SourceMode::ScryfallOnly;
    let history = context.history.filter(|_| use_mtgpics);
    let remembered = history.and_then(|history| history.remembered(card, faces.len()).ok().flatten());
    let mut arts = MtgpicsArts {
        remembered: remembered.as_ref(),
        found: None,
    };
    // Per face, where its MTGPics art was saved from; `None` if it wasn't.
    let mut log = Vec::with_capacity(faces.len());
    let mut mtgpics_failed = false;
    // This card's files start here: a job saves every printing it's asked for.
    let first_file = files.len();
    // A split card's halves MTGPics had no art for: Scryfall has none of a half.
    let mut halves_missing = Vec::new();

    // A split card saved whole before, as Scryfall has it or as MTGPics' one
    // picture of a Room, may have a copy the user found of the whole card.
    // It's used unless they found one of a half since.
    if faces.iter().any(FacePlan::is_split_half) && !faces.iter().any(|face| has_found_by_hand(face, context)) {
        if let Some(file) = found_by_hand(card, &whole_card(card, settings), context, errors).await {
            files.push(file);
            return;
        }
    }

    for (index, face) in faces.iter().enumerate() {
        // An image the user picked themselves comes before anything found for them.
        if let Some(file) = found_by_hand(card, face, context, errors).await {
            files.push(file);
            // MTGPics wasn't asked about this face: log what was known, or,
            // with nothing known, don't log the card as MTGPics having none.
            match remembered.as_ref() {
                Some(remembered) => log.push(remembered.mtgpics.get(index).cloned().flatten()),
                None => {
                    log.push(None);
                    mtgpics_failed = true;
                }
            }
            continue;
        }
        if use_mtgpics {
            match mtgpics_art(card, (index, face), faces.len(), &mut arts, context, errors).await {
                FromMtgpics::Saved(file, url) => {
                    files.push(file);
                    log.push(Some(url));
                    continue;
                }
                FromMtgpics::None => {}
                FromMtgpics::Failed => mtgpics_failed = true,
            }
        }
        log.push(None);
        if face.is_split_half() {
            halves_missing.push(face.name.clone());
            continue;
        }
        if settings.sources != SourceMode::MtgpicsOnly {
            match &face.scryfall_url {
                Some(url) => match save(card, face, ArtSource::Scryfall, url, context).await {
                    Ok(Some(file)) => {
                        files.push(file);
                        continue;
                    }
                    Ok(None) => errors.push("Scryfall's image is missing".into()),
                    Err(error) => errors.push(format!("Scryfall: {error}")),
                },
                None => errors.push("Scryfall has no image of this card".into()),
            }
        }
        missing.push(face.name.clone());
    }

    // A split card's halves from MTGPics: its page doesn't always list them
    // in order, and may file the whole card's image under each, as it does
    // Duskmourn's Rooms.
    if halves_missing.is_empty() && faces.iter().any(FacePlan::is_split_half) {
        let halves = match &files[first_file..] {
            [first, second] => split_halves(card, first, second, context).await,
            _ => Halves::AsSaved,
        };
        match halves {
            Halves::AsSaved => {}
            Halves::OnePicture => {
                let second = files.pop().expect("two files");
                let _ = tokio::fs::remove_file(&second.path).await;
                let first = files.pop().expect("two files");
                let whole = whole_card(card, settings);
                match rename_to_face(first, card, &whole, context).await {
                    Ok(file) => files.push(file),
                    Err(error) => {
                        errors.push(error.to_string());
                        missing.push(whole.name);
                    }
                }
            }
            Halves::Swapped => {
                let second = files.pop().expect("two files");
                let first = files.pop().expect("two files");
                match swap_pictures(&first, &second).await {
                    Ok((first, second)) => {
                        files.extend([first, second]);
                        // Logged the right way round, for the next run.
                        log.swap(0, 1);
                    }
                    // Left as they were saved: the wrong way round, but saved.
                    Err(error) => {
                        errors.push(error.to_string());
                        files.extend([first, second]);
                    }
                }
            }
        }
    }

    if !halves_missing.is_empty() {
        if settings.sources == SourceMode::MtgpicsOnly {
            missing.extend(halves_missing);
        } else {
            // Not both halves' art from MTGPics: the whole card, as Scryfall
            // has it, rather than one half from each.
            for file in files.drain(first_file..) {
                let _ = tokio::fs::remove_file(&file.path).await;
            }
            let whole = whole_card(card, settings);
            match &whole.scryfall_url {
                Some(url) => match save(card, &whole, ArtSource::Scryfall, url, context).await {
                    Ok(Some(file)) => files.push(file),
                    Ok(None) => {
                        errors.push("Scryfall's image is missing".into());
                        missing.push(whole.name);
                    }
                    Err(error) => {
                        errors.push(format!("Scryfall: {error}"));
                        missing.push(whole.name);
                    }
                },
                None => {
                    errors.push("Scryfall has no image of this card".into());
                    missing.push(whole.name);
                }
            }
        }
    }

    // Logged once MTGPics' answer for every face is known. A face it failed
    // to answer for — a timeout, say — mustn't be logged as one it has no art for.
    if let Some(history) = history.filter(|_| !mtgpics_failed) {
        let checked_at = match (&arts.found, &remembered) {
            (None, Some(remembered)) => remembered.checked_at,
            _ => SystemTime::now(),
        };
        let _ = history.remember(card, &log, checked_at);
    }
}

/// What a split card's two halves saved from MTGPics turned out to be.
enum Halves {
    AsSaved,
    /// One picture twice: the whole card's.
    OnePicture,
    /// Each the other half's painting.
    Swapped,
}

async fn split_halves(card: &Card, first: &SavedFile, second: &SavedFile, context: &Context<'_>) -> Halves {
    let (first_path, second_path) = (Path::new(&first.path), Path::new(&second.path));
    if same_contents(first_path, second_path).await {
        return Halves::OnePicture;
    }
    // A found copy was picked by the user, half by half.
    if first.source != ArtSource::Mtgpics || second.source != ArtSource::Mtgpics {
        return Halves::AsSaved;
    }
    // Scryfall's art crop shows the halves side by side, the first on the left.
    let (Some(crop_url), Some(dir)) = (card.art_crop(0), first_path.parent()) else {
        return Halves::AsSaved;
    };
    let crop = TempFile::beside(dir);
    if !matches!(context.http.download(crop_url, crop.path()).await, Ok(Some(_))) {
        return Halves::AsSaved;
    }
    let paths = (crop.path().to_owned(), first_path.to_owned(), second_path.to_owned());
    let swapped =
        tokio::task::spawn_blocking(move || crate::verify::halves_swapped(&paths.0, &paths.1, &paths.2)).await;
    match swapped {
        Ok(Ok(true)) => Halves::Swapped,
        _ => Halves::AsSaved,
    }
}

/// Swaps two saved images' pictures, so each face's name holds its own: each
/// keeps its type's extension, and its size, source and page go with it.
/// If a rename fails, the ones done are undone, so both are as they were.
async fn swap_pictures(first: &SavedFile, second: &SavedFile) -> Result<(SavedFile, SavedFile)> {
    let (first_path, second_path) = (PathBuf::from(&first.path), PathBuf::from(&second.path));
    let extension = |path: &Path| path.extension().map(std::ffi::OsStr::to_os_string).unwrap_or_default();
    // The first's picture goes under the second's name, and the other way round.
    let first_picture_to = second_path.with_extension(extension(&first_path));
    let second_picture_to = first_path.with_extension(extension(&second_path));
    let parked = first_path.with_extension("swapping.part");
    tokio::fs::rename(&first_path, &parked).await?;
    if let Err(error) = tokio::fs::rename(&second_path, &second_picture_to).await {
        let _ = tokio::fs::rename(&parked, &first_path).await;
        return Err(error.into());
    }
    if let Err(error) = tokio::fs::rename(&parked, &first_picture_to).await {
        let _ = tokio::fs::rename(&second_picture_to, &second_path).await;
        let _ = tokio::fs::rename(&parked, &first_path).await;
        return Err(error.into());
    }
    let path = |path: &Path| path.to_string_lossy().into_owned();
    Ok((
        SavedFile {
            path: path(&second_picture_to),
            face: first.face.clone(),
            face_index: first.face_index,
            artist: first.artist.clone(),
            ..second.clone()
        },
        SavedFile {
            path: path(&first_picture_to),
            face: second.face.clone(),
            face_index: second.face_index,
            artist: second.artist.clone(),
            ..first.clone()
        },
    ))
}

/// Whether two files hold the same bytes.
async fn same_contents(a: &Path, b: &Path) -> bool {
    match (tokio::fs::read(a).await, tokio::fs::read(b).await) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Moves a saved image to where `face`'s image from the same source goes, and
/// says it's that face's.
async fn rename_to_face(file: SavedFile, card: &Card, face: &FacePlan, context: &Context<'_>) -> Result<SavedFile> {
    let settings = context.settings;
    let dir = face_dir(&context.output_dir, settings, file.source, face);
    tokio::fs::create_dir_all(&dir).await?;
    let extension = Path::new(&file.path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("jpg")
        .to_owned();
    let path = context
        .paths
        .reserve(&dir, &face_stem(settings, card, face), &extension, settings.overwrite);
    tokio::fs::rename(&file.path, &path).await?;
    Ok(SavedFile {
        path: path.to_string_lossy().into_owned(),
        face: face.name.clone(),
        face_index: face.index as u32,
        artist: face.artist.clone(),
        ..file
    })
}

/// Whether the user found a copy of `face`'s art before.
fn has_found_by_hand(face: &FacePlan, context: &Context<'_>) -> bool {
    let (Some(history), Some(art)) = (context.history, face.illustration.as_deref()) else {
        return false;
    };
    history.found_by_hand(art).ok().flatten().is_some()
}

/// Saves a face's art from where the user found it before, if they did and
/// it's still the copy they saved: checked again, since anything could be at
/// that address now. Gone from there, or changed, it's forgotten.
async fn found_by_hand(
    card: &Card,
    face: &FacePlan,
    context: &Context<'_>,
    errors: &mut Vec<String>,
) -> Option<SavedFile> {
    if context.settings.sources == SourceMode::ScryfallOnly {
        return None;
    }
    let history = context.history?;
    let art = face.illustration.as_deref()?;
    let found = history.found_by_hand(art).ok().flatten()?;
    // The size the copy had when the user saved it.
    let accepted = history
        .found_copies(art)
        .unwrap_or_default()
        .into_iter()
        .find(|(_, copy)| copy.url == found.url)
        .and_then(|(_, copy)| copy.size);

    let saved = async {
        let dir = face_dir(&context.output_dir, context.settings, ArtSource::Web, face);
        tokio::fs::create_dir_all(&dir).await?;
        let temp = TempFile::beside(&dir);
        let copy = FoundCopyAt {
            url: &found.url,
            page: found.page.as_deref(),
            accepted,
        };
        let kind = match refetch_found_copy(context.http, card, face, copy, temp.path()).await? {
            Refetched::Ready(kind) => kind,
            other => return Ok(Err(other)),
        };
        let stem = face_stem(context.settings, card, face);
        let path = context
            .paths
            .reserve(&dir, &stem, kind.extension(), context.settings.overwrite);
        place(temp, kind, &path, card, face, ArtSource::Web).await.map(Ok)
    };
    match saved.await {
        Ok(Ok(file)) => Some(SavedFile {
            page: found.page,
            ..file
        }),
        Ok(Err(not_saved)) => {
            let _ = history.forget_found_by_hand(art);
            if not_saved == Refetched::Changed {
                errors.push(format!(
                    "The image you found at {} isn't the one you saved any more",
                    found.url
                ));
            }
            None
        }
        Err(error) => {
            errors.push(format!("The image you found: {error}"));
            None
        }
    }
}

/// Where a card's MTGPics art is: remembered from an earlier run, or found by
/// searching MTGPics, which happens at most once per card.
struct MtgpicsArts<'r> {
    remembered: Option<&'r Remembered>,
    /// The search's result per face, once it has run; `Err` if it failed.
    found: Option<std::result::Result<Vec<Option<MtgpicsArt>>, ()>>,
}

enum FromMtgpics {
    Saved(SavedFile, String),
    /// MTGPics has no art for the face.
    None,
    /// MTGPics couldn't be asked, or its image couldn't be saved.
    Failed,
}

/// Saves a face's art from MTGPics: from where it was found before, if it's
/// still there, otherwise from where a search finds it now.
async fn mtgpics_art(
    card: &Card,
    (index, face): (usize, &FacePlan),
    faces: usize,
    arts: &mut MtgpicsArts<'_>,
    context: &Context<'_>,
    errors: &mut Vec<String>,
) -> FromMtgpics {
    let mut failed = false;
    let remembered = arts.remembered.and_then(|r| r.mtgpics.get(index).cloned().flatten());
    if let Some(url) = &remembered {
        match save(card, face, ArtSource::Mtgpics, url, context).await {
            Ok(Some(file)) => return FromMtgpics::Saved(file, url.clone()),
            // Gone from there: look for it again.
            Ok(None) => {}
            Err(error) => {
                errors.push(format!("MTGPics: {error}"));
                failed = true;
            }
        }
    } else if arts.remembered.is_some_and(Remembered::missing_is_recent) {
        // MTGPics had no art for this face when it was last searched.
        return FromMtgpics::None;
    }

    if arts.found.is_none() {
        let found = mtgpics::find_arts(context.http, &context.cache, card, faces).await;
        arts.found = Some(found.map_err(|error| errors.push(format!("MTGPics: {error}"))));
    }
    let art = match &arts.found {
        Some(Ok(found)) => found.get(index).cloned().flatten(),
        _ => return FromMtgpics::Failed,
    };
    let Some(art) = art.filter(|art| Some(&art.url) != remembered.as_ref()) else {
        return if failed { FromMtgpics::Failed } else { FromMtgpics::None };
    };
    match save(card, face, ArtSource::Mtgpics, &art.url, context).await {
        Ok(Some(file)) => FromMtgpics::Saved(file, art.url),
        Ok(None) => FromMtgpics::None,
        Err(error) => {
            errors.push(format!("MTGPics: {error}"));
            FromMtgpics::Failed
        }
    }
}

/// Downloads one image into place; `None` if it isn't there.
async fn save(
    card: &Card,
    face: &FacePlan,
    source: ArtSource,
    url: &str,
    context: &Context<'_>,
) -> Result<Option<SavedFile>> {
    let settings = context.settings;
    let dir = face_dir(&context.output_dir, settings, source, face);
    tokio::fs::create_dir_all(&dir).await?;

    // Downloaded beside its final place, and only moved there whole: a run
    // cancelled mid-download leaves no half image behind.
    let temp = TempFile::beside(&dir);
    let Some(kind) = context.http.download(url, temp.path()).await? else {
        return Ok(None);
    };
    let stem = face_stem(settings, card, face);
    let path = context.paths.reserve(&dir, &stem, kind.extension(), settings.overwrite);
    let file = place(temp, kind, &path, card, face, source).await?;
    // Noted, so the image's size can be shown without downloading it again.
    if let (Some(history), Some(size)) = (context.history, file.size) {
        let _ = history.remember_image_size(url, size);
    }
    Ok(Some(file))
}

/// The folder a face's art from `source` goes in, inside `output_dir`.
pub(crate) fn face_dir(output_dir: &Path, settings: &Settings, source: ArtSource, face: &FacePlan) -> PathBuf {
    let mut dir = output_dir.to_path_buf();
    if settings.source_folders {
        dir.push(source.folder());
    }
    dir.extend(&face.folders);
    dir
}

/// The name a face's art is saved under, without its extension.
pub(crate) fn face_stem(settings: &Settings, card: &Card, face: &FacePlan) -> String {
    file_stem(
        &settings.naming,
        &NameParts {
            name: &face.name,
            artist: &face.artist,
            set: &card.set.to_uppercase(),
            number: &card.collector_number,
        },
    )
}

/// Moves a downloaded image to `path`, in the same folder, and says what it is.
pub(crate) async fn place(
    temp: TempFile,
    kind: ImageKind,
    path: &Path,
    card: &Card,
    face: &FacePlan,
    source: ArtSource,
) -> Result<SavedFile> {
    let size = image_size(temp.path(), kind);
    tokio::fs::rename(temp.path(), path).await?;
    temp.keep();
    Ok(SavedFile {
        path: path.to_string_lossy().into_owned(),
        face: face.name.clone(),
        face_index: face.index as u32,
        source,
        set: card.set.to_uppercase(),
        number: card.collector_number.clone(),
        artist: face.artist.clone(),
        size,
        page: None,
    })
}

/// Numbers every temporary file this process makes, so no two share a name:
/// a run's, and a save the user asks for while one runs.
static TEMP_IDS: AtomicU64 = AtomicU64::new(0);

/// A number no other temporary file in this process has.
pub(crate) fn next_temp_id() -> u64 {
    TEMP_IDS.fetch_add(1, Ordering::Relaxed)
}

/// A partly downloaded file, deleted when dropped unless kept.
pub(crate) struct TempFile(Option<PathBuf>);

impl TempFile {
    /// A hidden file in `dir`, named so no other download in this process,
    /// or any other, takes the same name.
    pub(crate) fn beside(dir: &Path) -> Self {
        Self(Some(dir.join(format!(
            ".download-{}-{}.part",
            std::process::id(),
            next_temp_id()
        ))))
    }

    pub(crate) fn path(&self) -> &Path {
        self.0.as_deref().expect("not yet kept")
    }

    fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Hands out file paths, so two jobs in one run can't both claim a name.
#[derive(Default)]
pub(crate) struct PathReserver {
    taken: Mutex<HashSet<PathBuf>>,
}

impl PathReserver {
    /// `dir/stem.ext`, or with `overwrite` off, the first of `stem (1).ext`,
    /// `stem (2).ext`… that's neither on disk nor handed out already.
    pub(crate) fn reserve(&self, dir: &Path, stem: &str, extension: &str, overwrite: bool) -> PathBuf {
        let mut taken = self.taken.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let first = dir.join(format!("{stem}.{extension}"));
        if overwrite {
            taken.insert(first.clone());
            return first;
        }
        let mut path = first;
        let mut n = 0;
        while taken.contains(&path) || path.exists() {
            n += 1;
            path = dir.join(format!("{stem} ({n}).{extension}"));
        }
        taken.insert(path.clone());
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_paths_when_not_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Karn.png"), b"").unwrap();
        let reserver = PathReserver::default();
        let name = |p: PathBuf| p.file_name().unwrap().to_string_lossy().into_owned();
        // Any extension, unlike the original's `.jpg`-only numbering.
        assert_eq!(name(reserver.reserve(dir.path(), "Karn", "png", false)), "Karn (1).png");
        assert_eq!(name(reserver.reserve(dir.path(), "Karn", "png", false)), "Karn (2).png");
        assert_eq!(name(reserver.reserve(dir.path(), "Karn", "jpg", false)), "Karn.jpg");
        assert_eq!(name(reserver.reserve(dir.path(), "Karn", "png", true)), "Karn.png");
    }
}
