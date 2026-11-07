//! The user's settings, kept as JSON in the app's config folder. The desktop app
//! and the command line tool read the same file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Result, APP_ID};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    /// The folder art is saved under.
    pub output_dir: String,
    /// The file name template: `NAME`, `ARTIST`, `SET` and `NUMBER` are filled in.
    pub naming: String,
    pub sources: SourceMode,
    pub scryfall_image: ScryfallImage,
    /// Which printing a line with only a name gets.
    pub printing: Printing,
    /// For a line with only a name, every printing's art instead of one.
    pub download_all: bool,
    /// When searching, one printing per distinct artwork rather than every printing.
    pub unique_art: bool,
    pub exclude_full_art: bool,
    /// Include extras in searches: Un-cards, oversized and championship cards, and the like.
    pub include_extras: bool,
    /// Save MTGPics and Scryfall art in folders of their own.
    pub source_folders: bool,
    /// Save each layout (MDFC Front, Adventure, Saga…) in a folder of its own.
    pub layout_folders: bool,
    /// Save each card type (Creature, Land…) in a folder of its own.
    pub type_folders: bool,
    /// Replace a file of the same name; otherwise the new one is numbered.
    pub overwrite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceMode {
    MtgpicsThenScryfall,
    MtgpicsOnly,
    ScryfallOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScryfallImage {
    /// Scryfall's `art_crop`: the art alone, as a JPEG.
    ArtCrop,
    /// Scryfall's `png`: the whole card scan.
    FullCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Printing {
    Newest,
    Oldest,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir().to_string_lossy().into_owned(),
            naming: "NAME (ARTIST) [SET] {NUMBER}".into(),
            sources: SourceMode::MtgpicsThenScryfall,
            scryfall_image: ScryfallImage::ArtCrop,
            printing: Printing::Newest,
            download_all: false,
            unique_art: true,
            exclude_full_art: false,
            include_extras: true,
            source_folders: true,
            layout_folders: true,
            type_folders: false,
            overwrite: true,
        }
    }
}

impl Settings {
    /// Reads settings from `path`, or the defaults if there is no file yet.
    /// Settings missing from the file take their defaults.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(json) => Ok(serde_json::from_str(&json)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Writes settings to `path`, through a temporary file so a crash can't
    /// leave half a file behind.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }
}

/// Where the settings file lives: `<config dir>/<APP_ID>/settings.json`, which
/// is also the desktop app's config folder.
pub fn default_settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(APP_ID).join("settings.json"))
}

/// `Pictures/MTG Art`, or the downloads or home folder when there's no
/// pictures folder.
pub fn default_output_dir() -> PathBuf {
    dirs::picture_dir()
        .or_else(dirs::download_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MTG Art")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_fills_in_missing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());

        let settings = Settings {
            naming: "NAME".into(),
            sources: SourceMode::ScryfallOnly,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);

        std::fs::write(&path, r#"{ "overwrite": false }"#).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert!(!loaded.overwrite);
        assert_eq!(loaded.naming, Settings::default().naming);
    }
}
