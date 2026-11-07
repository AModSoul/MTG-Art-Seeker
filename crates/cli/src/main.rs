//! `mtg-art`: the downloader without the window. Reads the same settings file as
//! the desktop app.

use std::{path::PathBuf, process::ExitCode};

use clap::{ArgGroup, Parser};
use mtg_art_core::{
    engine::{self, ArtSource, JobInfo, JobStatus, RunEvent, RunInput},
    http::{Http, HttpConfig},
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

    /// Read settings from this file instead of the app's.
    #[arg(long)]
    settings: Option<PathBuf>,
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

    let input = if let Some(path) = args.list {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
        RunInput::List { text }
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

    println!("Saving to {}", settings.output_dir);
    let labels = std::sync::Mutex::new(std::collections::HashMap::<u32, String>::new());
    let on_event = |event: RunEvent| report(event, &labels);
    let summary = engine::run(&http, input, &settings, &cancel, &on_event).await;

    Ok(if summary.failed + summary.partial + summary.cancelled == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn report(event: RunEvent, labels: &std::sync::Mutex<std::collections::HashMap<u32, String>>) {
    let mut labels = labels.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match event {
        RunEvent::Planned { jobs, skipped } => {
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
                        println!("{source} {label}  →  {}", file.path);
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
