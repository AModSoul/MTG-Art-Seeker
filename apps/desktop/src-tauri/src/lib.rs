//! The desktop app: Tauri commands over `mtg-art-core`. The front end only
//! shows what these report; finding and saving art all happens in the core.

mod finder;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime},
};

use mtg_art_core::{
    card_anvil::{self, CardAnvilSend, CardAnvilStatus},
    carddb::{default_card_data_dir, CardData, CardDataStatus},
    decks::{default_decks_dir, DeckSearch, Decks},
    engine::{self, ArtSource, JobStatus, Local, RunEvent, RunInput, SavedFile},
    history::{default_history_path, History},
    http::{Http, HttpConfig, USER_AGENT},
    known::{self, KnownSource},
    list::{parse_list, ListFile, ParsedList},
    manual::{self, Check, Destination, Doubt, Found, FoundEvent, Imported, PageCandidate, PageImage, Site, Target},
    mtgpics::Artists,
    presets::{presets, PresetInfo},
    settings::{default_settings_path, Settings},
};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio_util::sync::CancellationToken;

/// How often the app checks whether Scryfall has published newer card data.
const CARD_DATA_CHECK: Duration = Duration::from_secs(3 * 60 * 60);

/// How many runs' results are kept for finding larger copies in, newest first.
const RUNS_KEPT: u32 = 5;

struct AppState {
    http: Arc<Http>,
    settings_path: PathBuf,
    cards: Arc<CardData>,
    history: Option<Arc<History>>,
    /// MTGJSON's precon decks and their boxes' pictures.
    decks: Decks,
    runs: Mutex<HashMap<u32, CancellationToken>>,
    next_run: AtomicU32,
    /// What recent runs saved, by run id.
    records: Mutex<HashMap<u32, RunRecord>>,
    /// The face the finder window is looking for a larger copy of.
    search: Mutex<Option<Search>>,
    /// Images the user saved that didn't look like a larger copy of the art,
    /// kept until they say whether to save them anyway, by token.
    doubtful: Mutex<HashMap<u32, (Search, finder::Saved, Doubt)>>,
    /// The images of the page the user last asked to find the art on, while
    /// they choose one.
    choices: Mutex<Option<Choices>>,
    next_token: AtomicU32,
    /// MTGPics' list of artists, for their websites, read once a session.
    artists: Artists,
    /// Where manifests are written for Card Anvil to take.
    card_anvil_inbox: PathBuf,
    /// Held while art is sent to Card Anvil, so one send waits for the last
    /// to be taken.
    card_anvil_sending: tokio::sync::Mutex<()>,
}

/// What's kept of a run, so an image the user finds later is saved where the
/// run saved the face's art, and its art can be sent to Card Anvil.
struct RunRecord {
    settings: Settings,
    list_folder: Option<String>,
    /// What the run was of, for Card Anvil to name a group of cards after.
    title: Option<String>,
    /// Each finished job's saved files, by job id.
    files: HashMap<u32, Vec<SavedFile>>,
}

/// What a run was of, in a few words: a list's or deck's folder name, a
/// search's query, or a preset's name. `None` for a list given no name.
fn run_title(input: &RunInput) -> Option<String> {
    let title = match input {
        RunInput::List { .. } | RunInput::Deck { .. } => input.folder().map(str::to_owned),
        RunInput::Search { query } => Some(query.clone()),
        RunInput::Preset { id } => presets()
            .into_iter()
            .find(|preset| &preset.id == id)
            .map(|preset| preset.label),
    }?;
    let title = title.trim();
    (!title.is_empty()).then(|| title.to_owned())
}

/// A face the finder window is looking for a larger copy of, and where to tell
/// the front end what became of the images the user saves.
#[derive(Clone)]
struct Search {
    run: u32,
    job: u32,
    target: Target,
    /// Where the user started looking.
    site: Site,
    events: Channel<FoundEvent>,
}

/// A page's images, downloaded for the user to choose the art from.
struct Choices {
    token: u32,
    search: Search,
    page: Option<tauri::Url>,
    images: Vec<PageCandidate>,
}

impl Choices {
    /// Deletes the images.
    fn discard(self) {
        for image in self.images {
            let _ = std::fs::remove_file(image.downloaded);
        }
    }
}

/// Tells the front end about an image the user saved, and the user, who is
/// likely looking at the finder window, with a toast there. A question that
/// needs an answer is asked in the main window, whose taskbar icon flashes.
fn report(app: &AppHandle, search: &Search, event: FoundEvent) {
    let (kind, text) = match &event {
        FoundEvent::Checking { page: true, .. } => ("loading", "Downloading the images on this page…".to_owned()),
        FoundEvent::Checking { page: false, .. } => ("loading", "Checking the image you saved…".to_owned()),
        FoundEvent::Saved { file, .. } => (
            "success",
            match file.size {
                Some(size) => format!("Saved {}'s art: {} × {}", file.face, size.width, size.height),
                None => format!("Saved {}'s art", file.face),
            },
        ),
        FoundEvent::Doubtful { face, doubt, .. } => {
            let what = match doubt {
                Doubt::Smaller => format!("That copy of {face}'s art is no bigger than Scryfall's."),
                Doubt::CardScan => format!("That's the card, not {face}'s art on its own."),
                Doubt::Different => format!("That doesn't look like {face}'s art."),
            };
            ("warning", format!("{what} Save it anyway? Answer in MTG Art Seeker."))
        }
        FoundEvent::Choose { face, .. } => ("info", format!("Choose {face}'s art in MTG Art Seeker.")),
        FoundEvent::NoImages { face } => (
            "warning",
            format!("No image on this page is large enough to be {face}'s art."),
        ),
        FoundEvent::Failed { message } => ("error", format!("Couldn't save that image: {message}")),
    };
    finder::toast(app, kind, &text);
    if let Some(main) = app.get_webview_window("main") {
        match event {
            FoundEvent::Doubtful { .. } => {
                let _ = main.request_user_attention(Some(tauri::UserAttentionType::Informational));
            }
            // The user asked for the page's images, so they're shown at once.
            FoundEvent::Choose { .. } => {
                let _ = main.unminimize();
                let _ = main.set_focus();
            }
            _ => {}
        }
    }
    let _ = search.events.send(event);
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

type CommandResult<T> = Result<T, String>;

/// The saved settings. A file that can't be read gives the defaults, which the
/// next save replaces it with, rather than an app that can't start.
#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    Settings::load(&state.settings_path).unwrap_or_default()
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> CommandResult<()> {
    settings.save(&state.settings_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn parse_card_list(text: String) -> ParsedList {
    parse_list(&text)
}

#[tauri::command]
fn list_presets() -> Vec<PresetInfo> {
    presets()
}

/// The precon decks `query` matches, newest first, up to `limit` of them.
#[tauri::command]
async fn search_decks(state: State<'_, AppState>, query: String, limit: u32) -> CommandResult<DeckSearch> {
    state
        .decks
        .search(&state.http, &query, limit as usize)
        .await
        .map_err(|e| e.to_string())
}

/// Where the picture of deck `file_name`'s box is saved, downloading it the
/// first time; `None` when there's no picture of it.
#[tauri::command]
async fn deck_box(state: State<'_, AppState>, file_name: String) -> CommandResult<Option<String>> {
    let path = state
        .decks
        .box_image(&state.http, &file_name)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.map(|path| path.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn count_search(state: State<'_, AppState>, query: String, settings: Settings) -> CommandResult<u32> {
    engine::count_search(&state.http, &query, &settings)
        .await
        .map_err(|e| e.to_string())
}

/// Reads a card list file the user picked.
#[tauri::command]
async fn read_card_list(path: String) -> CommandResult<ListFile> {
    let read = {
        let path = PathBuf::from(&path);
        tauri::async_runtime::spawn_blocking(move || ListFile::read(&path))
    };
    read.await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Can't read {path}: {e}"))
}

/// Starts a run in the background and returns its id. Its progress arrives on
/// `on_event`, ending with `Finished`.
#[tauri::command]
fn start_run(
    app: AppHandle,
    state: State<AppState>,
    input: RunInput,
    settings: Settings,
    on_event: Channel<RunEvent>,
) -> CommandResult<u32> {
    // Saved art is shown through the asset protocol, which may read only the
    // output folder.
    std::fs::create_dir_all(&settings.output_dir).map_err(|e| format!("Can't create {}: {e}", settings.output_dir))?;
    app.asset_protocol_scope()
        .allow_directory(&settings.output_dir, true)
        .map_err(|e| e.to_string())?;

    let id = state.next_run.fetch_add(1, Ordering::Relaxed);
    {
        let mut records = lock(&state.records);
        records.retain(|run, _| *run + RUNS_KEPT > id);
        let list_folder = input.folder().map(str::to_owned);
        records.insert(
            id,
            RunRecord {
                settings: settings.clone(),
                list_folder,
                title: run_title(&input),
                files: HashMap::new(),
            },
        );
    }
    let cancel = CancellationToken::new();
    state
        .runs
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(id, cancel.clone());
    let http = state.http.clone();
    // The run keeps the card data it started with, even if an update lands meanwhile.
    let cards = state.cards.current();
    let history = state.history.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let emit = |event: RunEvent| {
            if let RunEvent::Job {
                id: job,
                status: JobStatus::Done { files, .. },
            } = &event
            {
                if let Some(record) = lock(&state.records).get_mut(&id) {
                    record.files.insert(*job, files.clone());
                }
            }
            let _ = on_event.send(event);
        };
        let local = Local {
            cards: cards.as_deref(),
            history: history.as_deref(),
        };
        engine::run(&http, input, &settings, local, &cancel, &emit).await;
        lock(&state.runs).remove(&id);
    });
    Ok(id)
}

#[tauri::command]
fn cancel_run(state: State<AppState>, id: u32) {
    if let Some(cancel) = state.runs.lock().unwrap_or_else(|p| p.into_inner()).get(&id) {
        cancel.cancel();
    }
}

/// Opens `site`, in a window of its own, to look for a larger copy of face
/// `face` of job `job` in run `run`: Google Lens on Scryfall's art crop, or a
/// search for the artist. What becomes of each image the user saves there
/// arrives on `on_event`.
#[tauri::command]
async fn find_larger_copy(
    app: AppHandle,
    state: State<'_, AppState>,
    run: u32,
    job: u32,
    face: u32,
    site: Site,
    on_event: Channel<FoundEvent>,
) -> CommandResult<()> {
    let (target, ..) = saved_face(&state, run, job, face).await?;
    let url = manual::site_url(&state.http, state.history.as_deref(), &state.artists, &target, site)
        .await
        .map_err(|e| e.to_string())?;
    let title = format!("Find a larger copy of {} on {}", target.face.name, site.name());
    *lock(&state.search) = Some(Search {
        run,
        job,
        target,
        site,
        events: on_event,
    });
    finder::open(&app, url, &title, on_saved, on_page).map_err(|e| e.to_string())?;
    Ok(())
}

/// What a run saved for face `face` of job `job`: the face, the run's
/// settings and list folder, and the file saved for it.
async fn saved_face(
    state: &AppState,
    run: u32,
    job: u32,
    face: u32,
) -> CommandResult<(Target, Settings, Option<String>, SavedFile)> {
    let (settings, list_folder, file) = {
        let records = lock(&state.records);
        let record = records
            .get(&run)
            .ok_or("That run is too long ago. Run the card again.")?;
        let file = record
            .files
            .get(&job)
            .and_then(|files| files.iter().find(|file| file.face_index == face))
            .cloned()
            .ok_or("That card's art wasn't saved.")?;
        (record.settings.clone(), record.list_folder.clone(), file)
    };
    let cards = state.cards.current();
    let target = manual::prepare(
        &state.http,
        cards.as_deref(),
        &settings,
        &file.set,
        &file.number,
        face as usize,
        Some(&file.face),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok((target, settings, list_folder, file))
}

/// Where face `face` of job `job` in run `run` is known to have art, for the
/// user to go back to one.
#[tauri::command]
async fn art_sources(state: State<'_, AppState>, run: u32, job: u32, face: u32) -> CommandResult<Vec<KnownSource>> {
    let (target, settings, _, file) = saved_face(&state, run, job, face).await?;
    Ok(known::known_sources(&state.http, state.history.as_deref(), &target, &settings, &file).await)
}

/// Saves face `face` of job `job` in run `run` again from `source` (for a copy
/// the user found, the one `copy` names), in place of the file saved for it,
/// and returns the new file.
#[tauri::command]
async fn save_art_from(
    state: State<'_, AppState>,
    run: u32,
    job: u32,
    face: u32,
    source: ArtSource,
    copy: Option<i64>,
) -> CommandResult<SavedFile> {
    let (target, settings, list_folder, current) = saved_face(&state, run, job, face).await?;
    let replace = PathBuf::from(&current.path);
    let destination = Destination {
        settings: &settings,
        list_folder: list_folder.as_deref(),
        replace: Some(&replace),
    };
    let (file, _) = known::save_from(
        &state.http,
        state.history.as_deref(),
        &target,
        destination,
        source,
        copy,
    )
    .await
    .map_err(|e| e.to_string())?;
    record_file(&state, run, job, file.clone());
    Ok(file)
}

/// Notes `file` as its face's file in job `job` of run `run`, so what's done
/// with it next starts from it.
fn record_file(state: &AppState, run: u32, job: u32, file: SavedFile) {
    if let Some(files) = lock(&state.records)
        .get_mut(&run)
        .and_then(|record| record.files.get_mut(&job))
    {
        files.retain(|f| f.face_index != file.face_index);
        files.push(file);
    }
}

/// Saves an image that didn't look like a larger copy of the art, or with
/// `save` false, throws it away.
#[tauri::command]
async fn confirm_found_art(app: AppHandle, state: State<'_, AppState>, token: u32, save: bool) -> CommandResult<()> {
    let (search, saved, doubt) = lock(&state.doubtful)
        .remove(&token)
        .ok_or("That image is gone. Save it from the search window again.")?;
    if save {
        let event = import_found(&state, &search, &saved, Check::Seen(Some(doubt))).await;
        report(&app, &search, event);
    } else {
        let _ = std::fs::remove_file(&saved.path);
    }
    Ok(())
}

/// Saves image `index` of the page images named by `token` as the art, or
/// with no `index`, saves none. Either way the images are then deleted.
#[tauri::command]
async fn choose_page_image(
    app: AppHandle,
    state: State<'_, AppState>,
    token: u32,
    index: Option<u32>,
) -> CommandResult<()> {
    let choices = {
        let mut choices = lock(&state.choices);
        match choices.as_ref() {
            Some(current) if current.token == token => choices.take(),
            _ => None,
        }
    }
    .ok_or("Those images are gone. Find the art on the page again.")?;
    let chosen = index.and_then(|index| choices.images.get(index as usize));
    if let Some(chosen) = chosen {
        let event = match tauri::Url::parse(&chosen.image_url) {
            Ok(image_url) => {
                // The user saw what the check said of it, so it's saved as it is.
                let saved = finder::Saved {
                    image_url,
                    page: choices.page.clone(),
                    path: chosen.downloaded.clone(),
                };
                import_found(&state, &choices.search, &saved, Check::Seen(chosen.doubt)).await
            }
            Err(error) => FoundEvent::Failed {
                message: error.to_string(),
            },
        };
        report(&app, &choices.search, event);
    }
    choices.discard();
    Ok(())
}

/// An image the user saved in the finder window: saved as the art of the face
/// being looked for, if it's a larger copy of it.
fn on_saved(app: &AppHandle, saved: finder::Saved) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let search = lock(&state.search).clone();
        let Some(search) = search else {
            let _ = std::fs::remove_file(&saved.path);
            return;
        };
        report(
            &app,
            &search,
            FoundEvent::Checking {
                face: search.target.face.name.clone(),
                page: false,
            },
        );
        let event = import_found(&state, &search, &saved, Check::First).await;
        report(&app, &search, event);
    });
}

/// The images of the page the user is on, when they ask to find the art on
/// it: downloaded and checked, for them to choose the face's art from.
fn on_page(app: &AppHandle, images: Vec<PageImage>, page: Option<tauri::Url>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let search = lock(&state.search).clone();
        let Some(search) = search else {
            return;
        };
        report(
            &app,
            &search,
            FoundEvent::Checking {
                face: search.target.face.name.clone(),
                page: true,
            },
        );
        let event = fetch_page(&app, &state, &search, &images, page).await;
        report(&app, &search, event);
    });
}

async fn fetch_page(
    app: &AppHandle,
    state: &AppState,
    search: &Search,
    images: &[PageImage],
    page: Option<tauri::Url>,
) -> FoundEvent {
    let face = search.target.face.name.clone();
    let dir = match finder::page_images_dir(app) {
        Ok(dir) => dir,
        Err(error) => {
            return FoundEvent::Failed {
                message: error.to_string(),
            }
        }
    };
    let referer = page.as_ref().map(tauri::Url::as_str);
    let images = match manual::fetch_page_images(&state.http, &search.target, images, referer, &dir).await {
        Ok(images) if images.is_empty() => return FoundEvent::NoImages { face },
        Ok(images) => images,
        Err(error) => {
            return FoundEvent::Failed {
                message: error.to_string(),
            }
        }
    };
    let token = state.next_token.fetch_add(1, Ordering::Relaxed);
    let choices = images.iter().map(Into::into).collect();
    let replaced = lock(&state.choices).replace(Choices {
        token,
        search: search.clone(),
        page,
        images,
    });
    // An earlier page's images, never chosen from, go.
    if let Some(replaced) = replaced {
        replaced.discard();
    }
    FoundEvent::Choose {
        token,
        face,
        images: choices,
    }
}

/// Where the run saved the face being looked for: its settings, its list
/// folder, and the file there now. `None` once the run is forgotten.
fn destination(state: &AppState, search: &Search) -> Option<(Settings, Option<String>, Option<PathBuf>)> {
    lock(&state.records).get(&search.run).map(|record| {
        let replace = record
            .files
            .get(&search.job)
            .and_then(|files| files.iter().find(|f| f.face_index as usize == search.target.face.index))
            .map(|file| PathBuf::from(&file.path));
        (record.settings.clone(), record.list_folder.clone(), replace)
    })
}

fn too_long_ago() -> FoundEvent {
    FoundEvent::Failed {
        message: "That run is too long ago. Run the card again to look for a larger copy.".into(),
    }
}

/// Notes a found image as the face's file, so a second one replaces it, and
/// says so.
fn saved(state: &AppState, search: &Search, file: SavedFile, replaced: Option<String>) -> FoundEvent {
    record_file(state, search.run, search.job, file.clone());
    FoundEvent::Saved {
        run: search.run,
        job: search.job,
        file,
        replaced,
    }
}

async fn import_found(state: &AppState, search: &Search, found: &finder::Saved, check: Check) -> FoundEvent {
    let Some((settings, list_folder, replace)) = destination(state, search) else {
        let _ = std::fs::remove_file(&found.path);
        return too_long_ago();
    };
    let destination = Destination {
        settings: &settings,
        list_folder: list_folder.as_deref(),
        replace: replace.as_deref(),
    };
    let from = Found {
        image_url: found.image_url.as_str(),
        page: found.page.as_ref().map(|page| page.as_str()),
        site: Some(search.site),
    };
    let imported = manual::import(
        &state.http,
        state.history.as_deref(),
        &search.target,
        destination,
        &found.path,
        from,
        check,
    )
    .await;
    match imported {
        Ok(Imported::Saved { file, replaced }) => {
            let _ = std::fs::remove_file(&found.path);
            saved(state, search, *file, replaced)
        }
        Ok(Imported::Doubtful(doubt)) => {
            let token = state.next_token.fetch_add(1, Ordering::Relaxed);
            lock(&state.doubtful).insert(token, (search.clone(), found.clone(), doubt));
            FoundEvent::Doubtful {
                token,
                face: search.target.face.name.clone(),
                doubt,
            }
        }
        Err(error) => {
            let _ = std::fs::remove_file(&found.path);
            FoundEvent::Failed {
                message: error.to_string(),
            }
        }
    }
}

/// Opens, in the system's browser, the page face `face` of job `job` in run
/// `run` was saved from, when it's an image the user found.
#[tauri::command]
fn open_found_page(app: AppHandle, state: State<AppState>, run: u32, job: u32, face: u32) -> CommandResult<()> {
    let page = lock(&state.records)
        .get(&run)
        .and_then(|record| record.files.get(&job))
        .and_then(|files| files.iter().find(|file| file.face_index == face))
        .and_then(|file| file.page.clone())
        .ok_or("There's no page for that image.")?;
    let url = tauri::Url::parse(&page).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("{page} isn't a web page."));
    }
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Shows a saved file in the system's file manager.
#[tauri::command]
fn reveal_file(app: AppHandle, path: String) -> CommandResult<()> {
    app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}

/// The Proxy Community Discord, which Card Anvil links to as well.
const DISCORD_INVITE: &str = "https://discord.gg/A5AkkyP8CU";

/// Opens the community Discord in the system's browser.
#[tauri::command]
fn open_discord(app: AppHandle) -> CommandResult<()> {
    app.opener()
        .open_url(DISCORD_INVITE, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Sends the art run `run` saved to Card Anvil: job `jobs`' art, or all of it.
/// `settings` say which Card Anvil, if the user chose one. Card Anvil asks the
/// user where to put the art; this returns once it has taken the request, or
/// once it's clear it won't.
#[tauri::command]
async fn send_to_card_anvil(
    state: State<'_, AppState>,
    run: u32,
    jobs: Option<Vec<u32>>,
    settings: Settings,
) -> CommandResult<CardAnvilSend> {
    // One at a time: each waits to hear whether Card Anvil took it.
    let _sending = state.card_anvil_sending.lock().await;
    let (files, scryfall_image, title) = {
        let records = lock(&state.records);
        let record = records
            .get(&run)
            .ok_or("That run is too long ago. Run the cards again.")?;
        let mut ids: Vec<u32> = jobs.unwrap_or_else(|| record.files.keys().copied().collect());
        ids.sort_unstable();
        ids.dedup();
        let files: Vec<Vec<SavedFile>> = ids
            .iter()
            .filter_map(|id| record.files.get(id))
            .filter(|files| !files.is_empty())
            .cloned()
            .collect();
        (files, record.settings.scryfall_image, record.title.clone())
    };
    if files.is_empty() {
        return Err("That run saved no art.".into());
    }
    let manifest = card_anvil::manifest(
        &files,
        title.as_deref(),
        scryfall_image,
        card_anvil::new_id(),
        SystemTime::now(),
        env!("CARGO_PKG_VERSION"),
    );
    let images = manifest.images();
    if manifest.items.len() > card_anvil::MAX_ITEMS {
        return Ok(CardAnvilSend::TooMany {
            cards: manifest.items.len() as u32,
            max: card_anvil::MAX_ITEMS as u32,
        });
    }

    let chosen = settings.card_anvil_path;
    let found = tauri::async_runtime::spawn_blocking(move || card_anvil::locate(chosen.as_deref()))
        .await
        .map_err(|e| e.to_string())?;
    let Some(app) = found else {
        return Ok(CardAnvilSend::NotFound);
    };
    let path = {
        let dir = state.card_anvil_inbox.clone();
        tauri::async_runtime::spawn_blocking(move || card_anvil::write_manifest(&dir, &manifest))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("Couldn't write the list of art for Card Anvil: {e}"))?
    };
    if let Err(error) = card_anvil::launch(&app, &path) {
        let _ = std::fs::remove_file(&path);
        return Err(format!("Couldn't start Card Anvil ({}): {error}", app.display()));
    }
    if card_anvil::wait_until_taken(&path, card_anvil::TAKE_WITHIN).await {
        Ok(CardAnvilSend::Delivered { images })
    } else {
        // Left for Card Anvil, which may still be starting; a later launch
        // clears it once it's old.
        Ok(CardAnvilSend::NotYetTaken { images })
    }
}

/// The Card Anvil art is sent to: `path`, the one the user chose, while it's
/// there, otherwise the installed one.
#[tauri::command]
async fn card_anvil_status(path: Option<String>) -> CommandResult<CardAnvilStatus> {
    tauri::async_runtime::spawn_blocking(move || card_anvil::status(path.as_deref()))
        .await
        .map_err(|e| e.to_string())
}

/// Opens Card Anvil's download page in the system's browser.
#[tauri::command]
fn open_card_anvil_site(app: AppHandle) -> CommandResult<()> {
    app.opener()
        .open_url(card_anvil::DOWNLOAD_PAGE, None::<&str>)
        .map_err(|e| e.to_string())
}

/// How current the local copy of Scryfall's card data is.
#[tauri::command]
fn card_data_status(state: State<AppState>) -> CardDataStatus {
    state.cards.status()
}

/// Opens a folder in the system's file manager.
#[tauri::command]
fn open_folder(app: AppHandle, path: String) -> CommandResult<()> {
    std::fs::create_dir_all(&path).map_err(|e| format!("Can't create {path}: {e}"))?;
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
}

/// What frames the main window, as `appWindow.ts` reads it: the app's own title bar on Windows
/// and Linux, or macOS's traffic lights over the app's header.
const WINDOW_FRAME: &str = if cfg!(target_os = "macos") { "mac" } else { "custom" };

/// Opens the main window from its entry in `tauri.conf.json`, which leaves that to this
/// (`create: false`) so one entry serves every system. On Windows and Linux the native title
/// bar is turned off, the app's header drawing its own; on macOS it stays, under the header,
/// as the entry's `titleBarStyle` and `hiddenTitle` say, which only macOS reads. The page
/// learns which before it loads, rather than guessing from the user agent.
fn open_main_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .ok_or("tauri.conf.json has no main window")?;
    tauri::WebviewWindowBuilder::from_config(app.handle(), config)?
        .decorations(cfg!(target_os = "macos"))
        .initialization_script(format!(
            "Object.defineProperty(window, \"__MTG_ART_WINDOW_FRAME__\", {{ value: \"{WINDOW_FRAME}\" }});"
        ))
        .build()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                // Its requests to GitHub name this app and its version, as the core's do,
                // rather than the plugin's own.
                app.handle().plugin(
                    tauri_plugin_updater::Builder::new()
                        .header("User-Agent", USER_AGENT)?
                        .build(),
                )?;
                app.handle().plugin(tauri_plugin_process::init())?;
            }
            let settings_path = default_settings_path()
                .or_else(|| app.path().app_config_dir().ok().map(|dir| dir.join("settings.json")))
                .ok_or("no config folder to keep settings in")?;
            let card_data_dir = default_card_data_dir()
                .or_else(|| app.path().app_local_data_dir().ok().map(|dir| dir.join("card-data")))
                .ok_or("no data folder to keep card data in")?;
            let http = Arc::new(Http::new(HttpConfig::default())?);
            let cards = Arc::new(CardData::open(card_data_dir));
            // Without the log, art is found afresh on MTGPics: slower, but no worse.
            let history = default_history_path()
                .or_else(|| {
                    app.path()
                        .app_local_data_dir()
                        .ok()
                        .map(|dir| dir.join("history.sqlite"))
                })
                .and_then(|path| match History::open(&path) {
                    Ok(history) => Some(Arc::new(history)),
                    Err(error) => {
                        eprintln!("can't open the log of saved art: {error}");
                        None
                    }
                });
            let decks_dir = default_decks_dir()
                .or_else(|| app.path().app_local_data_dir().ok().map(|dir| dir.join("decks")))
                .ok_or("no data folder to keep decks in")?;
            let decks = Decks::open(decks_dir);
            let card_anvil_inbox = card_anvil::default_inbox_dir()
                .or_else(|| app.path().app_local_data_dir().ok().map(|dir| dir.join("card-anvil")))
                .ok_or("no data folder to write Card Anvil's lists of art in")?;
            // A list of art left long enough was never taken. A newer one may be
            // another window's, still waiting on Card Anvil to start.
            if let Err(error) = card_anvil::clear_inbox_dir(&card_anvil_inbox, SystemTime::now()) {
                eprintln!("can't clear the art left for Card Anvil: {error}");
            }
            // The main window shows the pictures of the decks' boxes.
            std::fs::create_dir_all(decks.images_dir())?;
            app.asset_protocol_scope().allow_directory(decks.images_dir(), false)?;
            app.manage(AppState {
                http: http.clone(),
                settings_path,
                cards: cards.clone(),
                history,
                decks,
                runs: Mutex::new(HashMap::new()),
                next_run: AtomicU32::new(1),
                records: Mutex::new(HashMap::new()),
                search: Mutex::new(None),
                doubtful: Mutex::new(HashMap::new()),
                choices: Mutex::new(None),
                next_token: AtomicU32::new(1),
                artists: Artists::default(),
                card_anvil_inbox,
                card_anvil_sending: tokio::sync::Mutex::new(()),
            });
            finder::clear_downloads(app.handle());
            // The main window shows a page's images for the user to choose from.
            app.asset_protocol_scope()
                .allow_directory(finder::page_images_dir(app.handle())?, false)?;
            // Scryfall publishes new card data daily; until the first copy is
            // in, runs ask Scryfall about each card.
            tauri::async_runtime::spawn(async move {
                loop {
                    if let Err(error) = cards.refresh(&http).await {
                        eprintln!("couldn't update the card data: {error}");
                    }
                    tokio::time::sleep(CARD_DATA_CHECK).await;
                }
            });
            open_main_window(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            parse_card_list,
            list_presets,
            count_search,
            search_decks,
            deck_box,
            read_card_list,
            start_run,
            cancel_run,
            reveal_file,
            open_folder,
            card_data_status,
            open_discord,
            send_to_card_anvil,
            card_anvil_status,
            open_card_anvil_site,
            find_larger_copy,
            confirm_found_art,
            choose_page_image,
            open_found_page,
            art_sources,
            save_art_from,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
