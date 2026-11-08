//! Does a real run and records what the desktop app would be told about it,
//! for the app's screenshots (`apps/desktop/screenshots`).
//!
//! ```text
//! cargo run --release -p mtg-art-core --example record -- list <card list> <out dir> <recording.json>
//! cargo run --release -p mtg-art-core --example record -- deck <search> <deck file name> <folder> <out dir> <recording.json>
//! ```
//!
//! The art is saved in `<out dir>/art`, with a log of where it came from beside
//! it, so every run starts afresh. Keep `<out dir>` outside the repo: the
//! images are artists' work. The recording holds the settings, the run's
//! events, and, for a list, every saved face's other sources; for a deck, the
//! search that found it and its boxes' pictures.

use std::{collections::BTreeMap, path::PathBuf, process::ExitCode, sync::Mutex};

use mtg_art_core::{
    carddb::{default_card_data_dir, CardData},
    decks::{default_decks_dir, Decks},
    engine::{self, JobStatus, Local, RunEvent, RunInput},
    history::History,
    http::{Http, HttpConfig},
    known,
    list::parse_list,
    manual,
    settings::Settings,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

/// As many decks as the app's picker asks for at first (`DECK_PAGE`).
const DECK_PAGE: usize = 40;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["list", list, out, recording] => record_list(list, out, recording).await,
        ["deck", query, file_name, folder, out, recording] => {
            record_deck(query, file_name, folder, out, recording).await
        }
        _ => Err("usage: record list <card list> <out dir> <recording.json>\n       \
                  record deck <search> <deck file name> <folder> <out dir> <recording.json>"
            .to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// What every run needs: the shared `Http`, the card data if there's a copy
/// (it isn't updated here; the app or the CLI keeps it current), and a fresh
/// log of saved art, so MTGPics is searched as it would be the first time.
struct Setup {
    http: Http,
    card_data: Option<CardData>,
    history: History,
    settings: Settings,
}

fn setup(out: &str) -> Result<Setup, String> {
    let out = PathBuf::from(out);
    let art = out.join("art");
    let history_path = out.join("history.db");
    // Start afresh: no art saved, nothing logged.
    let _ = std::fs::remove_dir_all(&art);
    let _ = std::fs::remove_file(&history_path);
    std::fs::create_dir_all(&art).map_err(|e| format!("can't create {}: {e}", art.display()))?;
    // The app is told where each file is, and reads it from there.
    let art = std::fs::canonicalize(&art).map_err(|e| e.to_string())?;

    let settings = Settings {
        output_dir: art.to_string_lossy().into_owned(),
        ..Settings::default()
    };
    Ok(Setup {
        http: Http::new(HttpConfig::default()).map_err(|e| e.to_string())?,
        card_data: default_card_data_dir().map(CardData::open),
        history: History::open(&history_path).map_err(|e| e.to_string())?,
        settings,
    })
}

/// Runs `input`, printing each card as it finishes, and returns its events.
async fn run(setup: &Setup, input: RunInput) -> Vec<RunEvent> {
    let cards = setup.card_data.as_ref().and_then(CardData::current);
    if cards.is_none() {
        eprintln!("no copy of Scryfall's card data: every card is asked of Scryfall");
    }
    let local = Local {
        cards: cards.as_deref(),
        history: Some(&setup.history),
    };
    let events = Mutex::new(Vec::new());
    let on_event = |event: RunEvent| {
        match &event {
            RunEvent::Job {
                id,
                status: JobStatus::Done { .. },
            } => eprintln!("done: job {id}"),
            RunEvent::Job {
                id,
                status: JobStatus::Failed { error },
            } => eprintln!("failed: job {id}: {error}"),
            RunEvent::Error { message } => eprintln!("error: {message}"),
            _ => {}
        }
        events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event);
    };
    let cancel = CancellationToken::new();
    let summary = engine::run(&setup.http, input, &setup.settings, local, &cancel, &on_event).await;
    eprintln!(
        "{} cards in {:.1}s: {} done, {} failed",
        summary.jobs, summary.seconds, summary.done, summary.failed
    );
    events.into_inner().unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn record_list(list: &str, out: &str, recording: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(list).map_err(|e| format!("can't read {list}: {e}"))?;
    let setup = setup(out)?;
    let events = run(
        &setup,
        RunInput::List {
            text: text.clone(),
            folder: None,
        },
    )
    .await;

    // What the right-click menu offers for each saved face, keyed `job:face`.
    let cards = setup.card_data.as_ref().and_then(CardData::current);
    let mut sources = BTreeMap::new();
    for event in &events {
        let RunEvent::Job {
            id,
            status: JobStatus::Done { files, .. },
        } = event
        else {
            continue;
        };
        for file in files {
            let target = manual::prepare(
                &setup.http,
                cards.as_deref(),
                &setup.settings,
                &file.set,
                &file.number,
                file.face_index as usize,
                Some(&file.face),
            )
            .await
            .map_err(|e| format!("{}: {e}", file.face))?;
            let known = known::known_sources(&setup.http, Some(&setup.history), &target, &setup.settings, file).await;
            sources.insert(format!("{id}:{}", file.face_index), known);
        }
    }

    write(
        recording,
        &json!({
            "settings": setup.settings,
            "cardData": setup.card_data.as_ref().map(CardData::status),
            "input": { "kind": "list", "text": text.trim_end() },
            "parsed": parse_list(&text),
            "events": events,
            "sources": sources,
        }),
    )
}

async fn record_deck(query: &str, file_name: &str, folder: &str, out: &str, recording: &str) -> Result<(), String> {
    let mut setup = setup(out)?;
    // The deck's folder is only asked for with "A folder for each card list" on.
    setup.settings.list_folders = true;

    let decks = default_decks_dir()
        .map(Decks::open)
        .ok_or("no data folder to keep decks in")?;
    let found = decks
        .search(&setup.http, query, DECK_PAGE)
        .await
        .map_err(|e| e.to_string())?;
    let deck = found
        .decks
        .iter()
        .find(|deck| deck.file_name == file_name)
        .ok_or_else(|| {
            let names: Vec<&str> = found.decks.iter().map(|deck| deck.file_name.as_str()).collect();
            format!("searching {query} doesn't find {file_name}, only: {}", names.join(", "))
        })?
        .clone();
    let mut boxes = BTreeMap::new();
    for summary in &found.decks {
        let path = decks
            .box_image(&setup.http, &summary.file_name)
            .await
            .map_err(|e| e.to_string())?;
        boxes.insert(
            summary.file_name.clone(),
            path.map(|path| path.to_string_lossy().into_owned()),
        );
    }

    let input = RunInput::Deck {
        file_name: file_name.to_owned(),
        folder: Some(folder.to_owned()),
    };
    let events = run(&setup, input).await;

    write(
        recording,
        &json!({
            "settings": setup.settings,
            "cardData": setup.card_data.as_ref().map(CardData::status),
            "input": { "kind": "deck", "query": query, "deck": deck, "folder": folder },
            "decks": found,
            "boxes": boxes,
            "events": events,
        }),
    )
}

fn write(path: &str, recording: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(recording).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("can't write {path}: {e}"))?;
    eprintln!("recorded {path}");
    Ok(())
}
