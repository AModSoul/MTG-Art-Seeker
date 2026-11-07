//! The desktop app: Tauri commands over `mtg-art-core`. The front end only
//! shows what these report; finding and saving art all happens in the core.

mod lens;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use mtg_art_core::{
    carddb::{default_card_data_dir, CardData, CardDataStatus},
    engine::{self, JobStatus, Local, RunEvent, RunInput, SavedFile},
    history::{default_history_path, History},
    http::{Http, HttpConfig},
    list::{parse_list, ListFile, ParsedList},
    manual::{self, Destination, Found, FoundEvent, Imported, Target},
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
    runs: Mutex<HashMap<u32, CancellationToken>>,
    next_run: AtomicU32,
    /// What recent runs saved, by run id.
    records: Mutex<HashMap<u32, RunRecord>>,
    /// The face the Lens window is looking for a larger copy of.
    search: Mutex<Option<Search>>,
    /// Images the user saved that didn't look like a larger copy of the art,
    /// kept until they say whether to save them anyway, by token.
    doubtful: Mutex<HashMap<u32, (Search, lens::Saved)>>,
    next_token: AtomicU32,
}

/// What's kept of a run, so an image the user finds later is saved where the
/// run saved the face's art.
struct RunRecord {
    settings: Settings,
    list_folder: Option<String>,
    /// Each finished job's saved files, by job id.
    files: HashMap<u32, Vec<SavedFile>>,
}

/// A face the Lens window is looking for a larger copy of, and where to tell
/// the front end what became of the images the user saves.
#[derive(Clone)]
struct Search {
    run: u32,
    job: u32,
    target: Target,
    events: Channel<FoundEvent>,
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
        let list_folder = match &input {
            RunInput::List { folder, .. } => folder.clone(),
            _ => None,
        };
        records.insert(
            id,
            RunRecord {
                settings: settings.clone(),
                list_folder,
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

/// Opens Google Lens, in a window of its own, on Scryfall's art crop of face
/// `face` of job `job` in run `run`. What becomes of each image the user saves
/// there arrives on `on_event`.
#[tauri::command]
async fn find_larger_copy(
    app: AppHandle,
    state: State<'_, AppState>,
    run: u32,
    job: u32,
    face: u32,
    on_event: Channel<FoundEvent>,
) -> CommandResult<()> {
    let (settings, file) = {
        let records = lock(&state.records);
        let record = records
            .get(&run)
            .ok_or("That run is too long ago. Run the card again to look for a larger copy.")?;
        let file = record
            .files
            .get(&job)
            .and_then(|files| files.iter().find(|file| file.face_index == face))
            .cloned()
            .ok_or("That card's art wasn't saved.")?;
        (record.settings.clone(), file)
    };
    let cards = state.cards.current();
    let target = manual::prepare(
        &state.http,
        cards.as_deref(),
        &settings,
        &file.set,
        &file.number,
        face as usize,
    )
    .await
    .map_err(|e| e.to_string())?;
    let url = lens::lens_url(target.art_crop());
    let title = format!("Find a larger copy: {}", target.face.name);
    *lock(&state.search) = Some(Search {
        run,
        job,
        target,
        events: on_event,
    });
    lens::open(&app, url, &title, on_saved).map_err(|e| e.to_string())?;
    Ok(())
}

/// Saves an image that didn't look like a larger copy of the art, or with
/// `save` false, throws it away.
#[tauri::command]
async fn confirm_found_art(state: State<'_, AppState>, token: u32, save: bool) -> CommandResult<()> {
    let (search, saved) = lock(&state.doubtful)
        .remove(&token)
        .ok_or("That image is gone. Save it from the Lens window again.")?;
    if save {
        let event = import_found(&state, &search, &saved, true).await;
        let _ = search.events.send(event);
    } else {
        let _ = std::fs::remove_file(&saved.path);
    }
    Ok(())
}

/// An image the user saved in the Lens window: saved as the art of the face
/// being looked for, if it's a larger copy of it.
fn on_saved(app: &AppHandle, saved: lens::Saved) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let search = lock(&state.search).clone();
        let Some(search) = search else {
            let _ = std::fs::remove_file(&saved.path);
            return;
        };
        let event = import_found(&state, &search, &saved, false).await;
        let _ = search.events.send(event);
    });
}

async fn import_found(state: &AppState, search: &Search, saved: &lens::Saved, force: bool) -> FoundEvent {
    let record = lock(&state.records).get(&search.run).map(|record| {
        let replace = record
            .files
            .get(&search.job)
            .and_then(|files| files.iter().find(|f| f.face_index as usize == search.target.face.index))
            .map(|file| PathBuf::from(&file.path));
        (record.settings.clone(), record.list_folder.clone(), replace)
    });
    let Some((settings, list_folder, replace)) = record else {
        let _ = std::fs::remove_file(&saved.path);
        return FoundEvent::Failed {
            message: "That run is too long ago. Run the card again to look for a larger copy.".into(),
        };
    };
    let destination = Destination {
        settings: &settings,
        list_folder: list_folder.as_deref(),
        replace: replace.as_deref(),
    };
    let found = Found {
        image_url: saved.image_url.as_str(),
        page: saved.page.as_ref().map(|page| page.as_str()),
    };
    let imported = manual::import(
        &state.http,
        state.history.as_deref(),
        &search.target,
        destination,
        &saved.path,
        found,
        force,
    )
    .await;
    match imported {
        Ok(Imported::Saved { file, replaced }) => {
            let _ = std::fs::remove_file(&saved.path);
            // A second copy found for the face replaces this one, not the first.
            if let Some(files) = lock(&state.records)
                .get_mut(&search.run)
                .and_then(|record| record.files.get_mut(&search.job))
            {
                files.retain(|f| f.face_index != file.face_index);
                files.push(file.clone());
            }
            FoundEvent::Saved {
                run: search.run,
                job: search.job,
                file,
                replaced,
            }
        }
        Ok(Imported::Doubtful(doubt)) => {
            let token = state.next_token.fetch_add(1, Ordering::Relaxed);
            lock(&state.doubtful).insert(token, (search.clone(), saved.clone()));
            FoundEvent::Doubtful {
                token,
                face: search.target.face.name.clone(),
                doubt,
            }
        }
        Err(error) => {
            let _ = std::fs::remove_file(&saved.path);
            FoundEvent::Failed {
                message: format!("Couldn't save that image: {error}"),
            }
        }
    }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
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
            app.manage(AppState {
                http: http.clone(),
                settings_path,
                cards: cards.clone(),
                history,
                runs: Mutex::new(HashMap::new()),
                next_run: AtomicU32::new(1),
                records: Mutex::new(HashMap::new()),
                search: Mutex::new(None),
                doubtful: Mutex::new(HashMap::new()),
                next_token: AtomicU32::new(1),
            });
            lens::clear_downloads(app.handle());
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            parse_card_list,
            list_presets,
            count_search,
            read_card_list,
            start_run,
            cancel_run,
            reveal_file,
            open_folder,
            card_data_status,
            open_discord,
            find_larger_copy,
            confirm_found_art,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
