//! Finds Magic: The Gathering card art on MTGPics and Scryfall and saves it to disk.
//!
//! The desktop app and the `mtg-art` command line tool are both thin shells over
//! this crate: [`engine::run`] takes an input (a card list, a Scryfall search, or a
//! preset) and the user's [`settings::Settings`], and reports each card's progress
//! through a callback.

pub mod card;
pub mod carddb;
pub mod engine;
pub mod error;
pub mod faces;
pub mod history;
pub mod http;
pub mod imagesize;
pub mod known;
pub mod list;
pub mod manual;
pub mod mtgpics;
pub mod naming;
pub mod presets;
pub mod scryfall;
pub mod settings;
pub mod text;
pub mod verify;

pub use error::{Error, Result};

/// The app's identifier. The desktop app's bundle identifier, and the name of the
/// folder its settings live in, so the app and the command line tool share them.
pub const APP_ID: &str = "io.github.amodsoul.mtg-art-downloader";
