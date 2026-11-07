//! A download run: turns an input into jobs, one per card asked for, and works
//! through them a few at a time, reporting each job's progress as it goes.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Instant,
};

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{
    card::Card,
    faces::{plan_faces, FacePlan},
    http::Http,
    list::{parse_list, CardLine, SkippedLine},
    mtgpics,
    naming::{file_stem, NameParts},
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

/// What to download.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RunInput {
    /// A card list, one card per line.
    List { text: String },
    /// Every card a Scryfall search matches.
    Search { query: String },
    /// A ready-made list, by [`crate::presets::PresetInfo::id`].
    Preset { id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "event", rename_all = "camelCase")]
#[ts(export)]
pub enum RunEvent {
    /// The jobs a run will work through, all queued.
    Planned {
        jobs: Vec<JobInfo>,
        skipped: Vec<SkippedLine>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum JobStatus {
    Queued,
    /// Looking the card up on Scryfall.
    Resolving,
    /// Finding and saving its art.
    Downloading,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ArtSource {
    Mtgpics,
    Scryfall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SavedFile {
    pub path: String,
    /// The name of the face this is the art of.
    pub face: String,
    pub source: ArtSource,
    pub set: String,
    pub number: String,
    pub artist: String,
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
    cache: mtgpics::Cache,
    paths: PathReserver,
    temp_ids: AtomicU64,
    emit: &'a (dyn Fn(RunEvent) + Sync),
}

/// Runs `input` to the end, or until `cancel` is cancelled, sending each change
/// to `emit`. The last event is always [`RunEvent::Finished`].
pub async fn run(
    http: &Http,
    input: RunInput,
    settings: &Settings,
    cancel: &CancellationToken,
    emit: &(dyn Fn(RunEvent) + Sync),
) -> RunSummary {
    let started = Instant::now();
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
    emit(RunEvent::Planned {
        jobs: jobs.iter().map(Job::info).collect(),
        skipped,
    });

    // Cards a list names a set for are looked up together, 75 to a request.
    let batched: Vec<(usize, Identifier)> = jobs
        .iter()
        .enumerate()
        .filter_map(|(index, job)| match &job.request {
            Request::Line { line, .. } => identifier(line).map(|id| (index, id)),
            Request::Card(_) => None,
        })
        .collect();
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
        cache: mtgpics::Cache::default(),
        paths: PathReserver::default(),
        temp_ids: AtomicU64::new(0),
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
                    }
                }
            }
            JobStatus::Failed { .. } => summary.failed += 1,
            JobStatus::Cancelled => summary.cancelled += 1,
            JobStatus::Queued | JobStatus::Resolving | JobStatus::Downloading => {}
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
        RunInput::List { text } => {
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
    fn info(&self) -> JobInfo {
        let (label, line) = match &self.request {
            Request::Line { line, .. } => (line.label(), line.line.clone()),
            Request::Card(card) => (card.label(), card.label()),
        };
        JobInfo {
            id: self.id,
            label,
            line,
        }
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

    emit(JobStatus::Downloading);
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
    let query = scryfall::exact_name_query(&line.name);
    let mut cards = scryfall::search(http, &query, &search_options(settings, Some(direction)), MAX_PRINTINGS)
        .await?
        .cards;
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
    let arts = if settings.sources == SourceMode::ScryfallOnly {
        vec![None; faces.len()]
    } else {
        match mtgpics::find_arts(context.http, &context.cache, card, faces.len()).await {
            Ok(arts) => arts,
            Err(error) => {
                errors.push(format!("MTGPics: {error}"));
                vec![None; faces.len()]
            }
        }
    };

    for (face, art) in faces.iter().zip(arts) {
        if let Some(art) = art {
            match save(card, face, ArtSource::Mtgpics, &art.url, context).await {
                Ok(Some(file)) => {
                    files.push(file);
                    continue;
                }
                Ok(None) => {}
                Err(error) => errors.push(format!("MTGPics: {error}")),
            }
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
    let mut dir = PathBuf::from(&settings.output_dir);
    if settings.source_folders {
        dir.push(match source {
            ArtSource::Mtgpics => "mtgpics",
            ArtSource::Scryfall => "scryfall",
        });
    }
    dir.extend(&face.folders);
    tokio::fs::create_dir_all(&dir).await?;

    // Downloaded beside its final place, and only moved there whole: a run
    // cancelled mid-download leaves no half image behind.
    let temp = TempFile(Some(dir.join(format!(
        ".download-{}-{}.part",
        std::process::id(),
        context.temp_ids.fetch_add(1, Ordering::Relaxed)
    ))));
    let Some(kind) = context.http.download(url, temp.path()).await? else {
        return Ok(None);
    };
    let set = card.set.to_uppercase();
    let stem = file_stem(
        &settings.naming,
        &NameParts {
            name: &face.name,
            artist: &face.artist,
            set: &set,
            number: &card.collector_number,
        },
    );
    let path = context.paths.reserve(&dir, &stem, kind.extension(), settings.overwrite);
    tokio::fs::rename(temp.path(), &path).await?;
    temp.keep();
    Ok(Some(SavedFile {
        path: path.to_string_lossy().into_owned(),
        face: face.name.clone(),
        source,
        set,
        number: card.collector_number.clone(),
        artist: face.artist.clone(),
    }))
}

/// A partly downloaded file, deleted when dropped unless kept.
struct TempFile(Option<PathBuf>);

impl TempFile {
    fn path(&self) -> &Path {
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
struct PathReserver {
    taken: Mutex<HashSet<PathBuf>>,
}

impl PathReserver {
    /// `dir/stem.ext`, or with `overwrite` off, the first of `stem (1).ext`,
    /// `stem (2).ext`… that's neither on disk nor handed out already.
    fn reserve(&self, dir: &Path, stem: &str, extension: &str, overwrite: bool) -> PathBuf {
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
