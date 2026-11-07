//! `mtg-art`: the downloader without the window. Reads the same settings file as
//! the desktop app.

use std::{path::PathBuf, process::ExitCode};

use clap::{ArgGroup, Parser};
use mtg_art_core::{
    carddb::{default_card_data_dir, CardData},
    engine::{self, ArtSource, JobInfo, JobStatus, Local, RunEvent, RunInput},
    history::{default_history_path, History},
    http::{Http, HttpConfig},
    list::ListFile,
    presets::presets,
    settings::{default_settings_path, Settings},
};
use tokio_util::sync::CancellationToken;

/// Download Magic: The Gathering card art from MTGPics, with Scryfall as a backup.
#[derive(Parser)]
#[command(version, group(ArgGroup::new("input").required(true).args(["list", "search", "preset", "presets"])))]
struct Args {
    /// A card list file, one card per line (`4 Damnation (TSR) 106`).
    list: Option<PathBuf>,

    /// Download every card a Scryfall search matches, e.g. "set:mh2 t:creature".
    #[arg(long, short)]
    search: Option<String>,

    /// Download a ready-made list; see --presets.
    #[arg(long, short)]
    preset: Option<String>,

    /// List the ready-made lists.
    #[arg(long)]
    presets: bool,

    /// Save art here instead of the folder in the settings.
    #[arg(long, short)]
    out: Option<PathBuf>,

    /// Save the list's art in a folder of this name inside the output folder.
    /// With "A folder for each list" on in the settings, the list file's name
    /// is used when this isn't given.
    #[arg(long, short, requires = "list")]
    folder: Option<String>,

    /// Read settings from this file instead of the app's.
    #[arg(long)]
    settings: Option<PathBuf>,

    /// Don't check for newer Scryfall card data before a list or preset; use
    /// the copy already downloaded, if there is one.
    #[arg(long)]
    no_update: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    match run(args).await {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<ExitCode, String> {
    if args.presets {
        for preset in presets() {
            println!("{:<22} {}", preset.id, preset.label);
        }
        return Ok(ExitCode::SUCCESS);
    }

    let settings_path = args.settings.or_else(default_settings_path);
    let mut settings = match &settings_path {
        Some(path) => Settings::load(path).map_err(|e| format!("can't read {}: {e}", path.display()))?,
        None => Settings::default(),
    };
    if let Some(out) = args.out {
        settings.output_dir = out.to_string_lossy().into_owned();
    }

    if args.folder.is_some() {
        settings.list_folders = true;
    }

    let input = if let Some(path) = args.list {
        let file = ListFile::read(&path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
        RunInput::List {
            text: file.text,
            folder: Some(args.folder.unwrap_or(file.name)),
        }
    } else if let Some(query) = args.search {
        RunInput::Search { query }
    } else if let Some(id) = args.preset {
        RunInput::Preset { id }
    } else {
        unreachable!("clap requires an input");
    };

    let http = Http::new(HttpConfig::default()).map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    let on_ctrl_c = cancel.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("\nCancelling…");
            on_ctrl_c.cancel();
        }
    });

    // A search is Scryfall's to answer; lists and presets are looked up in its
    // card data, kept locally and updated when Scryfall has published newer.
    let card_data = match input {
        RunInput::Search { .. } => None,
        _ => default_card_data_dir().map(CardData::open),
    };
    if let Some(data) = card_data.as_ref().filter(|_| !args.no_update) {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(ExitCode::FAILURE),
            () = update_card_data(&http, data) => {}
        }
    }
    let cards = card_data.as_ref().and_then(CardData::current);
    // Without the log, art is found afresh on MTGPics: slower, but no worse.
    let history = default_history_path().and_then(|path| match History::open(&path) {
        Ok(history) => Some(history),
        Err(error) => {
            eprintln!("warning: can't open the log of saved art: {error}");
            None
        }
    });
    let local = Local {
        cards: cards.as_deref(),
        history: history.as_ref(),
    };

    let labels = std::sync::Mutex::new(std::collections::HashMap::<u32, String>::new());
    let on_event = |event: RunEvent| report(event, &labels);
    let summary = engine::run(&http, input, &settings, local, &cancel, &on_event).await;

    Ok(if summary.failed + summary.partial + summary.cancelled == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Downloads Scryfall's card data if it has published newer than the copy here.
/// A failure only means Scryfall is asked about each card instead.
async fn update_card_data(http: &Http, data: &CardData) {
    let file = match CardData::latest(http).await {
        Ok(file) => file,
        Err(error) => {
            eprintln!("warning: couldn't check for newer card data: {error}");
            return;
        }
    };
    if !data.is_older_than(&file) {
        return;
    }
    println!("Downloading Scryfall's card data…");
    let started = std::time::Instant::now();
    match data.update_to(http, &file).await {
        Ok(_) => println!(
            "{} cards ready in {:.0}s",
            data.status().cards,
            started.elapsed().as_secs_f64()
        ),
        Err(error) => eprintln!("warning: couldn't update the card data: {error}"),
    }
}

fn report(event: RunEvent, labels: &std::sync::Mutex<std::collections::HashMap<u32, String>>) {
    let mut labels = labels.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match event {
        RunEvent::Planned { jobs, skipped, folder } => {
            println!("Saving to {folder}");
            println!("{} cards to download", jobs.len());
            for line in skipped {
                println!("  skipped: {}", line.line);
            }
            labels.extend(jobs.into_iter().map(|JobInfo { id, label, .. }| (id, label)));
        }
        RunEvent::Job { id, status } => {
            let label = labels.get(&id).map_or("?", String::as_str);
            match status {
                JobStatus::Done { files, missing } => {
                    for file in files {
                        let source = match file.source {
                            ArtSource::Mtgpics => "MTGPICS ",
                            ArtSource::Scryfall => "SCRYFALL",
                        };
                        let size = file
                            .size
                            .map_or_else(|| "?".to_owned(), |s| format!("{}×{}", s.width, s.height));
                        println!("{source} {size:>9}  {label}  →  {}", file.path);
                    }
                    for face in missing {
                        println!("MISSING  {face} ({label})");
                    }
                }
                JobStatus::Failed { error } => println!("FAILED   {label}: {error}"),
                JobStatus::Queued | JobStatus::Resolving | JobStatus::Downloading | JobStatus::Cancelled => {}
            }
        }
        RunEvent::Error { message } => eprintln!("error: {message}"),
        RunEvent::Finished { summary } => {
            println!(
                "\n{} cards in {:.1}s: {} done, {} partly done, {} failed, {} cancelled \
                 ({} from MTGPics, {} from Scryfall)",
                summary.jobs,
                summary.seconds,
                summary.done,
                summary.partial,
                summary.failed,
                summary.cancelled,
                summary.mtgpics_files,
                summary.scryfall_files,
            );
        }
    }
}
