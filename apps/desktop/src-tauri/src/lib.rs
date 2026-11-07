//! The desktop app: Tauri commands over `mtg-art-core`. The front end only
//! shows what these report; finding and saving art all happens in the core.

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
    engine::{self, Local, RunEvent, RunInput},
    history::{default_history_path, History},
    http::{Http, HttpConfig},
    list::{parse_list, ListFile, ParsedList},
    presets::{presets, PresetInfo},
    settings::{default_settings_path, Settings},
};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio_util::sync::CancellationToken;

/// How often the app checks whether Scryfall has published newer card data.
const CARD_DATA_CHECK: Duration = Duration::from_secs(3 * 60 * 60);

struct AppState {
    http: Arc<Http>,
    settings_path: PathBuf,
    cards: Arc<CardData>,
    history: Option<Arc<History>>,
    runs: Mutex<HashMap<u32, CancellationToken>>,
    next_run: AtomicU32,
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
        let emit = |event: RunEvent| {
            let _ = on_event.send(event);
        };
        let local = Local {
            cards: cards.as_deref(),
            history: history.as_deref(),
        };
        engine::run(&http, input, &settings, local, &cancel, &emit).await;
        let state = app.state::<AppState>();
        state.runs.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
    });
    Ok(id)
}

#[tauri::command]
fn cancel_run(state: State<AppState>, id: u32) {
    if let Some(cancel) = state.runs.lock().unwrap_or_else(|p| p.into_inner()).get(&id) {
        cancel.cancel();
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
            });
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
