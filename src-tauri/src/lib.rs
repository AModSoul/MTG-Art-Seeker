//! The desktop app: Tauri commands over `mtg-art-core`. The front end only
//! shows what these report; finding and saving art all happens in the core.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
};

use mtg_art_core::{
    engine::{self, RunEvent, RunInput},
    http::{Http, HttpConfig},
    list::{parse_list, ParsedList},
    presets::{presets, PresetInfo},
    settings::{default_settings_path, Settings},
};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio_util::sync::CancellationToken;

struct AppState {
    http: Arc<Http>,
    settings_path: PathBuf,
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
async fn read_card_list(path: String) -> CommandResult<String> {
    tokio::fs::read_to_string(&path)
        .await
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
    tauri::async_runtime::spawn(async move {
        let emit = |event: RunEvent| {
            let _ = on_event.send(event);
        };
        engine::run(&http, input, &settings, &cancel, &emit).await;
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
            let http = Http::new(HttpConfig::default())?;
            app.manage(AppState {
                http: Arc::new(http),
                settings_path,
                runs: Mutex::new(HashMap::new()),
                next_run: AtomicU32::new(1),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
