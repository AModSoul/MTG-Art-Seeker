//! Handing saved art to Card Anvil, the card maker, through its image inbox.
//!
//! Card Anvil keeps its cards where no other program can write, so art is
//! handed over by asking: a small JSON manifest names the images and the card
//! each is the art of ([`manifest`], [`write_manifest`]), and Card Anvil is
//! started with that file's path ([`locate`], [`launch`]). It deletes the
//! manifest once it has kept the request, which is the receipt: one still
//! there after [`TAKE_WITHIN`] hasn't been taken yet ([`wait_until_taken`]).
//! It's left for Card Anvil, which may still be starting — watched for up to
//! [`TAKE_LATE_WITHIN`] more, so a late receipt is still heard — and cleared
//! away once it's older than [`KEEP_UNTAKEN`] ([`clear_inbox_dir`]).
//!
//! The protocol is Card Anvil's, and `docs/inbox.md` in its repository
//! (<https://github.com/Card-Anvil/Card-Anvil>) is its specification.
//! `tests/fixtures/card-anvil-inbox-v1.cardanvil-inbox` is a byte-identical
//! copy of its example manifest, which the tests here build.

use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use reqwest::Url;
use serde::Serialize;
use ts_rs::TS;

use crate::{
    engine::{ArtSource, SavedFile},
    faces::UNKNOWN_ARTIST,
    manual::Site,
    settings::ScryfallImage,
    Error, Result, APP_ID,
};

/// What every manifest says it is.
pub const KIND: &str = "cardanvil.inbox";
/// The protocol's version these manifests are written in.
pub const VERSION: u32 = 1;
/// The most items, cards, one manifest may hold.
pub const MAX_ITEMS: usize = 1000;
/// How long a send waits for Card Anvil to take its manifest. Still there
/// after this, it hasn't been taken yet: Card Anvil may still be starting, or
/// it's too old to know the inbox, or turned the manifest down.
pub const TAKE_WITHIN: Duration = Duration::from_secs(15);
/// How much longer a manifest not taken within [`TAKE_WITHIN`] is watched
/// for: a slow first start is over well within it.
pub const TAKE_LATE_WITHIN: Duration = Duration::from_secs(5 * 60);
/// How long a manifest is left for Card Anvil to take: long enough for the
/// slowest start, so clearing the folder never takes one from a send still
/// waiting on it.
pub const KEEP_UNTAKEN: Duration = Duration::from_secs(60 * 60);
/// Where Card Anvil can be downloaded.
pub const DOWNLOAD_PAGE: &str = "https://cardanvil.com/download";

/// The sender, as Card Anvil shows it.
const SOURCE_APP: &str = "MTG Art Seeker";
const SOURCE_URL: &str = "https://getmtg.art";
/// A manifest's file extension.
const EXTENSION: &str = "cardanvil-inbox";
/// How often [`wait_until_taken`] looks.
const POLL: Duration = Duration::from_millis(200);

/// Card Anvil's executable, as its Windows installer names it.
#[cfg_attr(not(windows), allow(dead_code))]
const WINDOWS_EXE: &str = "Card-Anvil.exe";
/// Card Anvil's entry among the installed programs, where its installer
/// records the folder it installed to and the executable's name.
#[cfg_attr(not(windows), allow(dead_code))]
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Card Anvil";
/// The key under which Card Anvil's installer records its folder: as the
/// key's default value (NSIS), or as `InstallDir` (MSI).
#[cfg_attr(not(windows), allow(dead_code))]
const INSTALL_DIR_KEY: &str = r"Software\cardanvil\Card Anvil";
/// Card Anvil's macOS app bundle.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAC_APP: &str = "Card Anvil.app";
/// Card Anvil's macOS bundle identifier.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const BUNDLE_ID: &str = "com.cardanvil";
/// The names Card Anvil's executable may have on Linux.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
const LINUX_NAMES: [&str; 2] = ["Card-Anvil", "card-anvil"];

// --- The manifest ------------------------------------------------------------

/// A request for Card Anvil to add art: version 1 of its inbox manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub kind: &'static str,
    pub version: u32,
    /// This delivery's id, which also names its file.
    pub id: String,
    /// When it was written, RFC 3339 in UTC.
    pub created_at: String,
    pub source: Source,
    /// The card group Card Anvil suggests putting the cards in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One per card, in the order they're to be added.
    pub items: Vec<Item>,
}

impl Manifest {
    /// How many images it sends: one per item, and a split card's second.
    pub fn images(&self) -> u32 {
        let images: usize = self
            .items
            .iter()
            .map(|item| 1 + usize::from(item.second_art.is_some()))
            .sum();
        u32::try_from(images).unwrap_or(u32::MAX)
    }
}

/// Who's sending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
    pub app: &'static str,
    pub version: String,
    pub url: &'static str,
}

/// One card's art.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(flatten)]
    pub art: Art,
    pub card: CardRef,
    /// A split card's second half's painting; the first half's is `art`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second_art: Option<Art>,
}

/// An image, and what's known of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Art {
    /// Absolute.
    pub path: String,
    /// For show only: Card Anvil credits the printing's artist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    pub image: ImageKind,
    pub origin: Origin,
    /// Where the image was downloaded from. With [`Origin::Mtgpics`], Card
    /// Anvil files it as MTGPics' art at this address, not as an upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Scryfall's illustration id for the artwork, whatever the origin. With
    /// [`Origin::Scryfall`], Card Anvil files the image as Scryfall's art.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub illustration_id: Option<String>,
    /// For an image the user found, the site it's from, or else the one they
    /// found it through: `artstation`, `lens`… ([`Site::key`]); see [`found_on`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<&'static str>,
    /// The page an image the user found came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// The printing, and which of its faces, an image is the art of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRef {
    /// The face's name, or for a split card sent with both halves' paintings,
    /// the card's: `Fire // Ice`.
    pub name: String,
    /// Scryfall's set code, in lower case as Scryfall writes it.
    pub set: String,
    pub collector_number: String,
    /// Which of the printing's faces, from 0.
    pub face_index: u32,
}

/// What an image shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageKind {
    /// The art alone.
    Art,
    /// A scan of the whole card.
    Card,
}

/// Where an image came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Mtgpics,
    Scryfall,
    Web,
}

impl From<ArtSource> for Origin {
    fn from(source: ArtSource) -> Self {
        match source {
            ArtSource::Mtgpics => Self::Mtgpics,
            ArtSource::Scryfall => Self::Scryfall,
            ArtSource::Web => Self::Web,
        }
    }
}

/// The manifest that sends `jobs`' saved files, each job's in turn: one item
/// per image, except that a split card's two halves, each its own painting,
/// make one item with both. A lone half, its other one not saved, is an item
/// of its own, as is each face of a double-faced card.
///
/// `scryfall_image` is the setting the files were saved with, which says
/// whether Scryfall's are art or whole cards; `title` names the card group
/// Card Anvil suggests; `source_version` is this app's version.
pub fn manifest(
    jobs: &[Vec<SavedFile>],
    title: Option<&str>,
    scryfall_image: ScryfallImage,
    id: String,
    created_at: SystemTime,
    source_version: &str,
) -> Manifest {
    let art = |file: &SavedFile| art_of(file, scryfall_image);
    let mut items = Vec::new();
    for files in jobs {
        for faces in by_printing(files) {
            let halves: Vec<&SavedFile> = faces.iter().copied().filter(|file| file.split_half).collect();
            let pair = match *halves.as_slice() {
                [first, second] if first.face_index == 0 && second.face_index == 1 => Some((first, second)),
                _ => None,
            };
            for file in faces {
                match pair {
                    Some((first, second)) if std::ptr::eq(file, first) => items.push(Item {
                        art: art(first),
                        card: CardRef {
                            name: format!("{} // {}", first.face, second.face),
                            face_index: 0,
                            ..card_of(first)
                        },
                        second_art: Some(art(second)),
                    }),
                    Some((_, second)) if std::ptr::eq(file, second) => {}
                    _ => items.push(Item {
                        art: art(file),
                        card: card_of(file),
                        second_art: None,
                    }),
                }
            }
        }
    }
    Manifest {
        kind: KIND,
        version: VERSION,
        id,
        created_at: rfc3339(created_at),
        source: Source {
            app: SOURCE_APP,
            version: source_version.to_owned(),
            url: SOURCE_URL,
        },
        title: title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned),
        items,
    }
}

/// A job's files a printing at a time, in the order the printings were saved
/// (a job saves every printing it's asked for), each printing's in face order.
fn by_printing(files: &[SavedFile]) -> Vec<Vec<&SavedFile>> {
    let mut printings: Vec<Vec<&SavedFile>> = Vec::new();
    let mut index: HashMap<(&str, &str), usize> = HashMap::new();
    for file in files {
        let at = *index.entry((&file.set, &file.number)).or_insert_with(|| {
            printings.push(Vec::new());
            printings.len() - 1
        });
        printings[at].push(file);
    }
    for faces in &mut printings {
        faces.sort_by_key(|file| file.face_index);
    }
    printings
}

fn card_of(file: &SavedFile) -> CardRef {
    CardRef {
        name: file.face.clone(),
        set: file.set.to_lowercase(),
        collector_number: file.number.clone(),
        face_index: file.face_index,
    }
}

fn art_of(file: &SavedFile, scryfall_image: ScryfallImage) -> Art {
    let whole_card =
        file.whole_card || (file.source == ArtSource::Scryfall && scryfall_image == ScryfallImage::FullCard);
    Art {
        path: file.path.clone(),
        artist: Some(file.artist.trim())
            .filter(|artist| !artist.is_empty() && *artist != UNKNOWN_ARTIST)
            .map(str::to_owned),
        image: if whole_card { ImageKind::Card } else { ImageKind::Art },
        origin: file.source.into(),
        url: file.url.clone(),
        illustration_id: file.illustration.clone(),
        site: (file.source == ArtSource::Web)
            .then(|| found_on(file))
            .flatten()
            .map(Site::key),
        page: file.page.clone(),
        width: file.size.map(|size| size.width),
        height: file.size.map(|size| size.height),
    }
}

/// The site an image the user found is from: the one its page is on, or
/// else the image itself, however they got there. Otherwise, where they
/// started looking for it: Google Lens, say, or the artist's own website.
fn found_on(file: &SavedFile) -> Option<Site> {
    [file.page.as_deref(), file.url.as_deref()]
        .into_iter()
        .flatten()
        .find_map(site_at)
        .or(file.site)
}

/// The sites found art is on, by the domains their pages and images are at.
const SITE_DOMAINS: [(&str, Site); 6] = [
    ("artstation.com", Site::Artstation),
    ("deviantart.com", Site::Deviantart),
    ("pixiv.net", Site::Pixiv),
    // Where Pixiv keeps its images.
    ("pximg.net", Site::Pixiv),
    ("instagram.com", Site::Instagram),
    // Where Instagram keeps its images.
    ("cdninstagram.com", Site::Instagram),
];

/// The site `address` is on, if it's one of [`SITE_DOMAINS`] or a subdomain
/// of one: `cdna.artstation.com` is ArtStation, `artstation.com.example` isn't.
fn site_at(address: &str) -> Option<Site> {
    let url = Url::parse(address).ok()?;
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    SITE_DOMAINS
        .iter()
        .find(|(domain, _)| {
            host.strip_suffix(domain)
                .is_some_and(|rest| rest.is_empty() || rest.ends_with('.'))
        })
        .map(|(_, site)| *site)
}

/// `time` in RFC 3339, in UTC, to the second: `2026-11-09T14:03:12Z`.
fn rfc3339(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let (days, of_day) = (seconds / 86_400, seconds % 86_400);
    // Howard Hinnant's `civil_from_days`, for days on or after 1970-01-01.
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

// --- The manifest's file ----------------------------------------------------

/// A new delivery's id: the time in nanoseconds and this process's id, in hex.
pub fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{nanos:x}-{:x}", std::process::id())
}

/// Whether `id` is one Card Anvil accepts, and so safe in a file name.
fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// The folder manifests are written to: `<local data dir>/<APP_ID>/card-anvil`.
pub fn default_inbox_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(APP_ID).join("card-anvil"))
}

/// Writes `manifest` to `<id>.cardanvil-inbox` in `dir`, through a temporary
/// file so Card Anvil never reads half of one, and returns its path.
pub fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<PathBuf> {
    let path = manifest_path(dir, &manifest.id)
        .ok_or_else(|| Error::Other(format!("{:?} can't name a manifest", manifest.id)))?;
    std::fs::create_dir_all(dir)?;
    let temp = dir.join(format!("{}.{EXTENSION}.part", manifest.id));
    let written =
        std::fs::write(&temp, serde_json::to_vec_pretty(manifest)?).and_then(|()| std::fs::rename(&temp, &path));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(error.into());
    }
    Ok(path)
}

/// Where the manifest `id` is kept in `dir`, if `id` can name a file.
pub fn manifest_path(dir: &Path, id: &str) -> Option<PathBuf> {
    valid_id(id).then(|| dir.join(format!("{id}.{EXTENSION}")))
}

/// Deletes the files in `dir` left long enough ([`is_stale`]): manifests
/// Card Anvil never took. A newer one may belong to a send still waiting on
/// Card Anvil, in this window or another, so it's left. A folder that isn't
/// there is already clear, and a file gone already, deleted by another window
/// starting at the same time, is too. One that can't be looked at or deleted
/// is left, and the rest are still cleared; the first such error is returned.
pub fn clear_inbox_dir(dir: &Path, now: SystemTime) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut failed = None;
    for entry in entries {
        let cleared = entry.and_then(|entry| {
            let metadata = entry.metadata()?;
            if metadata.is_file() && is_stale(metadata.modified()?, now) {
                std::fs::remove_file(entry.path())?;
            }
            Ok(())
        });
        match cleared {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                failed.get_or_insert(error);
            }
        }
    }
    failed.map_or(Ok(()), |error| Err(error.into()))
}

/// Whether a manifest last written at `modified` was left long enough to be
/// cleared away: more than [`KEEP_UNTAKEN`] before `now`. One written "after"
/// `now` was written before a clock was set back: by a little, as a clock is
/// corrected, and a send may still be waiting on it; by more than
/// [`KEEP_UNTAKEN`], by a clock that was wrong, and it's as old as the rest.
fn is_stale(modified: SystemTime, now: SystemTime) -> bool {
    match now.duration_since(modified) {
        Ok(age) => age > KEEP_UNTAKEN,
        Err(ahead) => ahead.duration() > KEEP_UNTAKEN,
    }
}

/// Waits for Card Anvil to take the manifest at `path`, which it does by
/// deleting it: `true` once it's gone, `false` if it's still there after
/// `timeout`. A look that fails, as when another program has the file open,
/// says nothing either way, so it looks again.
pub async fn wait_until_taken(path: &Path, timeout: Duration) -> bool {
    let started = Instant::now();
    loop {
        if let Ok(false) = path.try_exists() {
            return true;
        }
        if started.elapsed() >= timeout {
            return false;
        }
        tokio::time::sleep(POLL).await;
    }
}

// --- Finding and starting Card Anvil ------------------------------------------

/// What the app knows of the Card Anvil it sends art to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardAnvilStatus {
    /// The Card Anvil art is sent to: the one the user chose, or else the
    /// installed one. Left out when there's neither.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    /// Whether `path` is the one the user chose.
    pub chosen: bool,
    /// Whether the user chose one that isn't there any more.
    pub chosen_missing: bool,
}

/// What came of sending art to Card Anvil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "camelCase")]
#[ts(export)]
pub enum CardAnvilSend {
    /// Card Anvil took the request, of `images` images, and will ask the user
    /// where to put them.
    Delivered { images: u32 },
    /// Card Anvil isn't installed, or isn't where the user said it is.
    NotFound,
    /// Card Anvil was started but hasn't taken the request of `images` images
    /// yet. It may still be starting, so the request is left for it; or it's
    /// too old to know the inbox. Ask after it as `request` with
    /// `card_anvil_taken`.
    NotYetTaken { images: u32, request: String },
    /// More cards than one request can hold: `cards`, of at most `max`.
    TooMany { cards: u32, max: u32 },
}

/// The Card Anvil to send art to: `chosen`, the one the user picked, if it's
/// there, otherwise the installed one, if it can be found.
pub fn locate(chosen: Option<&str>) -> Option<PathBuf> {
    chosen_app(chosen).or_else(installed)
}

/// The Card Anvil [`locate`] finds, and whether it's the one the user chose.
pub fn status(chosen: Option<&str>) -> CardAnvilStatus {
    let found = chosen_app(chosen);
    let chosen_missing = chosen.is_some_and(|chosen| !chosen.trim().is_empty()) && found.is_none();
    let chosen = found.is_some();
    CardAnvilStatus {
        path: found.or_else(installed).map(|path| path.to_string_lossy().into_owned()),
        chosen,
        chosen_missing,
    }
}

fn chosen_app(chosen: Option<&str>) -> Option<PathBuf> {
    chosen
        .map(str::trim)
        .filter(|chosen| !chosen.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

/// The installed Card Anvil, where this system's installer puts it.
fn installed() -> Option<PathBuf> {
    #[cfg(windows)]
    let mut candidates =
        windows_candidates(&read_registry, &|name: &str| std::env::var_os(name).map(PathBuf::from)).into_iter();
    #[cfg(target_os = "macos")]
    let mut candidates = mac_candidates(dirs::home_dir(), mdfind);
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut candidates = path_candidates(std::env::var_os("PATH")).into_iter();
    candidates.find(|path| is_app(path))
}

/// Whether `path` is an app to start: a file, or on macOS an app bundle too.
fn is_app(path: &Path) -> bool {
    if cfg!(target_os = "macos") {
        path.exists()
    } else {
        path.is_file()
    }
}

/// A registry hive Card Anvil's installer writes to: the user's, for an
/// install for them alone, or the machine's, for one for everyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
enum Hive {
    CurrentUser,
    LocalMachine,
}

/// Where Card Anvil may be installed on Windows, most telling first: where its
/// installer says it put it, for the user and then for everyone; the folder it
/// keeps for the next install; and the default folders. `registry` reads a
/// string value (`""` for a key's default); `var` an environment variable.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_candidates(
    registry: &dyn Fn(Hive, &str, &str) -> Option<String>,
    var: &dyn Fn(&str) -> Option<PathBuf>,
) -> Vec<PathBuf> {
    // `InstallLocation` is written in quotes.
    let read = |hive, key, name| {
        registry(hive, key, name)
            .map(|value| value.trim().trim_matches('"').to_owned())
            .filter(|value| !value.is_empty())
    };
    let mut candidates = Vec::new();
    for hive in [Hive::CurrentUser, Hive::LocalMachine] {
        if let Some(dir) = read(hive, UNINSTALL_KEY, "InstallLocation") {
            let exe = read(hive, UNINSTALL_KEY, "MainBinaryName").unwrap_or_else(|| WINDOWS_EXE.to_owned());
            candidates.push(PathBuf::from(dir).join(exe));
        }
    }
    for name in ["", "InstallDir"] {
        if let Some(dir) = read(Hive::CurrentUser, INSTALL_DIR_KEY, name) {
            candidates.push(PathBuf::from(dir).join(WINDOWS_EXE));
        }
    }
    for (variable, folder) in [("LOCALAPPDATA", "Card Anvil"), ("ProgramFiles", "Card Anvil")] {
        if let Some(dir) = var(variable) {
            candidates.push(dir.join(folder).join(WINDOWS_EXE));
        }
    }
    candidates
}

#[cfg(windows)]
fn read_registry(hive: Hive, key: &str, name: &str) -> Option<String> {
    let root = match hive {
        Hive::CurrentUser => windows_registry::CURRENT_USER,
        Hive::LocalMachine => windows_registry::LOCAL_MACHINE,
    };
    root.open(key).ok()?.get_string(name).ok()
}

/// Where Card Anvil may be on macOS: the system's and the user's
/// Applications folders, then wherever Spotlight finds its bundle id, which
/// `mdfind` is only asked for if neither has it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mac_candidates(home: Option<PathBuf>, mdfind: impl FnOnce() -> Vec<PathBuf>) -> impl Iterator<Item = PathBuf> {
    let applications = [
        Some(Path::new("/Applications").join(MAC_APP)),
        home.map(|home| home.join("Applications").join(MAC_APP)),
    ];
    applications
        .into_iter()
        .flatten()
        .chain(std::iter::once_with(mdfind).flatten())
}

#[cfg(target_os = "macos")]
fn mdfind() -> Vec<PathBuf> {
    let output = Command::new("mdfind")
        .arg(format!("kMDItemCFBundleIdentifier == '{BUNDLE_ID}'"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect(),
        _ => Vec::new(),
    }
}

/// Where Card Anvil may be on Linux: on the `PATH`, under either name.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
fn path_candidates(path: Option<OsString>) -> Vec<PathBuf> {
    let Some(path) = path else {
        return Vec::new();
    };
    std::env::split_paths(&path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .flat_map(|dir| LINUX_NAMES.map(|name| dir.join(name)))
        .collect()
}

/// Starts Card Anvil, `app`, with the manifest at `manifest`, which must be
/// an absolute path. A Card Anvil already running is handed the manifest by
/// the copy started here, which then quits. Nothing waits for it.
pub fn launch(app: &Path, manifest: &Path) -> std::io::Result<()> {
    let mut child = spawn(app, manifest)?;
    // Waited for, so it doesn't linger as a zombie once it exits.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// `app --inbox <manifest>`, started in `app`'s folder, with nothing to read or write to.
fn command(app: &Path, manifest: &Path) -> Command {
    let mut command = Command::new(app);
    command
        .arg("--inbox")
        .arg(manifest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = app.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        command.current_dir(dir);
    }
    command
}

/// Started on its own, with no console, and out of this app's job, so closing
/// this app doesn't close it. A job that won't let it leave keeps it in.
#[cfg(windows)]
fn spawn(app: &Path, manifest: &Path) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let detached = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    command(app, manifest)
        .creation_flags(detached | CREATE_BREAKAWAY_FROM_JOB)
        .spawn()
        .or_else(|_| command(app, manifest).creation_flags(detached).spawn())
}

/// An app bundle is opened with `open -n`, which starts a new copy even with
/// one running, and passes it the arguments; plain `open` would only bring the
/// running one forward. A bare executable, a development build, is started as it is.
#[cfg(target_os = "macos")]
fn spawn(app: &Path, manifest: &Path) -> std::io::Result<Child> {
    if !app.is_dir() {
        return command(app, manifest).spawn();
    }
    let mut command = Command::new("open");
    command
        .arg("-n")
        .arg("-a")
        .arg(app)
        .arg("--args")
        .arg("--inbox")
        .arg(manifest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn spawn(app: &Path, manifest: &Path) -> std::io::Result<Child> {
    command(app, manifest).spawn()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::imagesize::ImageSize;

    /// 2026-11-09T14:03:12Z, the fixture's `createdAt`.
    const CREATED_AT: u64 = 1_794_232_992;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn file(path: &str, face: &str, face_index: u32, source: ArtSource, printing: (&str, &str)) -> SavedFile {
        SavedFile {
            path: path.into(),
            face: face.into(),
            face_index,
            source,
            set: printing.0.into(),
            number: printing.1.into(),
            artist: "Dan Murayama Scott".into(),
            size: Some(ImageSize {
                width: 1000,
                height: 680,
            }),
            page: None,
            site: None,
            split_half: false,
            whole_card: false,
            url: None,
            illustration: None,
        }
    }

    /// Downloaded from `url`, of the fixture's artwork `art`: its illustration
    /// ids differ only in their last digit.
    fn from(file: SavedFile, url: &str, art: u32) -> SavedFile {
        SavedFile {
            url: Some(url.into()),
            illustration: Some(format!("6f0f6b11-7f6e-4c1b-9d4e-1a2b3c4d5e0{art}")),
            ..file
        }
    }

    fn sized(file: SavedFile, artist: &str, width: u32, height: u32) -> SavedFile {
        SavedFile {
            artist: artist.into(),
            size: Some(ImageSize { width, height }),
            ..file
        }
    }

    fn half(file: SavedFile) -> SavedFile {
        SavedFile {
            split_half: true,
            ..file
        }
    }

    const DIR: &str = r"C:\MTG Art\Karn's Workshop";

    /// What a run of the fixture's cards saved, job by job, as the engine
    /// reports it: set codes in upper case, and a split card as its halves.
    fn fixture_jobs() -> Vec<Vec<SavedFile>> {
        let path = |rest: &str| format!(r"{DIR}\{rest}");
        let mtgpics = ArtSource::Mtgpics;
        vec![
            vec![from(
                sized(
                    file(
                        &path(r"mtgpics\Basic\Karn, Scion of Urza (Chase Stone) [DOM] {1}.jpg"),
                        "Karn, Scion of Urza",
                        0,
                        mtgpics,
                        ("DOM", "1"),
                    ),
                    "Chase Stone",
                    1024,
                    768,
                ),
                "https://www.mtgpics.com/pics/art/dom/001.jpg",
                1,
            )],
            vec![
                from(
                    sized(
                        file(
                            &path(r"mtgpics\MDFC Front\Agadeem's Awakening (Dmitry Burmak) [ZNR] {90}.jpg"),
                            "Agadeem's Awakening",
                            0,
                            mtgpics,
                            ("ZNR", "90"),
                        ),
                        "Dmitry Burmak",
                        1500,
                        1126,
                    ),
                    "https://www.mtgpics.com/pics/art/znr/090.jpg",
                    2,
                ),
                from(
                    sized(
                        file(
                            &path(r"mtgpics\MDFC Back\Agadeem, the Undercrypt (Dmitry Burmak) [ZNR] {90}.jpg"),
                            "Agadeem, the Undercrypt",
                            1,
                            mtgpics,
                            ("ZNR", "90"),
                        ),
                        "Dmitry Burmak",
                        1500,
                        1126,
                    ),
                    "https://www.mtgpics.com/pics/art/znr/090b.jpg",
                    3,
                ),
            ],
            vec![from(
                sized(
                    file(
                        &path(r"scryfall\Basic\Sol Ring (Hector Ortiz) [TRC] {499}.jpg"),
                        "Sol Ring",
                        0,
                        ArtSource::Scryfall,
                        ("TRC", "499"),
                    ),
                    "Hector Ortiz",
                    626,
                    457,
                ),
                "https://cards.scryfall.io/art_crop/front/1/b/1b59533a-3e43-4c6c-8a4e-1a2b3c4d5e04.jpg?1700000000",
                4,
            )],
            vec![
                half(from(
                    file(
                        &path(r"mtgpics\Split\Fire (Dan Murayama Scott) [MH2] {290}.jpg"),
                        "Fire",
                        0,
                        mtgpics,
                        ("MH2", "290"),
                    ),
                    "https://www.mtgpics.com/pics/art/mh2/290.jpg",
                    5,
                )),
                half(from(
                    sized(
                        file(
                            &path(r"mtgpics\Split\Ice (Dan Murayama Scott) [MH2] {290}.jpg"),
                            "Ice",
                            1,
                            mtgpics,
                            ("MH2", "290"),
                        ),
                        "Dan Murayama Scott",
                        960,
                        652,
                    ),
                    "https://www.mtgpics.com/pics/art/mh2/290b.jpg",
                    5,
                )),
            ],
            // Found through Google Lens, on ArtStation.
            vec![SavedFile {
                page: Some("https://www.artstation.com/artwork/birdsofparadise".into()),
                site: Some(Site::Lens),
                ..from(
                    sized(
                        file(
                            &path(r"web\Basic\Birds of Paradise (Marcelo Vignali) [M12] {165}.webp"),
                            "Birds of Paradise",
                            0,
                            ArtSource::Web,
                            ("M12", "165"),
                        ),
                        "Marcelo Vignali",
                        2400,
                        1760,
                    ),
                    "https://cdna.artstation.com/p/assets/images/images/000/000/001/large/birds-of-paradise.webp",
                    6,
                )
            }],
        ]
    }

    fn fixture_manifest(jobs: &[Vec<SavedFile>], scryfall_image: ScryfallImage) -> Manifest {
        manifest(
            jobs,
            Some("Karn's Workshop"),
            scryfall_image,
            "18c3f7a9e2b-1f4".into(),
            at(CREATED_AT),
            "0.3.0",
        )
    }

    #[test]
    fn builds_card_anvils_example_manifest() {
        let golden = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/card-anvil-inbox-v1.cardanvil-inbox"
        ))
        .unwrap();
        let golden: serde_json::Value = serde_json::from_str(&golden).unwrap();
        let built = fixture_manifest(&fixture_jobs(), ScryfallImage::ArtCrop);
        assert_eq!(serde_json::to_value(&built).unwrap(), golden);
        // Six cards, the split one with two images.
        assert_eq!((built.items.len(), built.images()), (6, 7));
    }

    #[test]
    fn a_split_cards_halves_saved_in_any_order_make_one_card() {
        let mut jobs = fixture_jobs();
        // A half saved again from elsewhere is noted after the other.
        jobs[3].reverse();
        let items = fixture_manifest(&jobs, ScryfallImage::ArtCrop).items;
        let fire_ice = &items[4];
        assert_eq!(fire_ice.card.name, "Fire // Ice");
        assert_eq!(fire_ice.card.face_index, 0);
        assert!(fire_ice.art.path.ends_with("Fire (Dan Murayama Scott) [MH2] {290}.jpg"));
        let ice = fire_ice.second_art.as_ref().unwrap();
        assert!(ice.path.ends_with("Ice (Dan Murayama Scott) [MH2] {290}.jpg"));
    }

    #[test]
    fn a_lone_half_is_a_card_of_its_own() {
        let mut jobs = fixture_jobs();
        jobs[3].truncate(1);
        let items = fixture_manifest(&jobs[3..4], ScryfallImage::ArtCrop).items;
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].card.name.as_str(), items[0].card.face_index), ("Fire", 0));
        assert_eq!(items[0].second_art, None);

        // Ice alone keeps its own face.
        let ice = vec![fixture_jobs()[3][1].clone()];
        let items = fixture_manifest(&[ice], ScryfallImage::ArtCrop).items;
        assert_eq!((items[0].card.name.as_str(), items[0].card.face_index), ("Ice", 1));
    }

    #[test]
    fn a_double_faced_cards_faces_stay_two_cards() {
        let jobs = fixture_jobs();
        let items = fixture_manifest(&jobs[1..2], ScryfallImage::ArtCrop).items;
        let faces: Vec<_> = items
            .iter()
            .map(|item| (item.card.name.as_str(), item.card.face_index, item.second_art.is_some()))
            .collect();
        assert_eq!(
            faces,
            [("Agadeem's Awakening", 0, false), ("Agadeem, the Undercrypt", 1, false)]
        );
    }

    #[test]
    fn halves_pair_only_within_their_own_printing() {
        // A job that saved every printing of Fire // Ice: two printings' halves.
        let fire = |set| half(file("fire.jpg", "Fire", 0, ArtSource::Mtgpics, (set, "1")));
        let ice = |set| half(file("ice.jpg", "Ice", 1, ArtSource::Mtgpics, (set, "1")));
        let job = vec![fire("MH2"), ice("MH2"), fire("DMR"), ice("DMR")];
        let items = fixture_manifest(&[job], ScryfallImage::ArtCrop).items;
        let cards: Vec<_> = items
            .iter()
            .map(|item| (item.card.set.as_str(), item.second_art.is_some()))
            .collect();
        assert_eq!(cards, [("mh2", true), ("dmr", true)]);
    }

    #[test]
    fn leaves_out_an_unknown_artist() {
        let mut jobs = fixture_jobs();
        jobs[0][0].artist = UNKNOWN_ARTIST.into();
        let items = fixture_manifest(&jobs, ScryfallImage::ArtCrop).items;
        assert_eq!(items[0].art.artist, None);
        let json = serde_json::to_value(&items[0]).unwrap();
        assert!(json.get("artist").is_none());
    }

    #[test]
    fn says_where_each_image_came_from_and_which_artwork_it_is() {
        let items = fixture_manifest(&fixture_jobs(), ScryfallImage::ArtCrop).items;
        let sol_ring = &items[3].art;
        assert_eq!(sol_ring.origin, Origin::Scryfall);
        assert_eq!(
            sol_ring.illustration_id.as_deref(),
            Some("6f0f6b11-7f6e-4c1b-9d4e-1a2b3c4d5e04")
        );
        // Each half its own painting's address, and the one artwork Scryfall gives both.
        let fire_ice = &items[4];
        let ice = fire_ice.second_art.as_ref().unwrap();
        assert_eq!(
            (fire_ice.art.url.as_deref(), ice.url.as_deref()),
            (
                Some("https://www.mtgpics.com/pics/art/mh2/290.jpg"),
                Some("https://www.mtgpics.com/pics/art/mh2/290b.jpg")
            )
        );
        assert_eq!(fire_ice.art.illustration_id, ice.illustration_id);
    }

    #[test]
    fn leaves_out_an_address_and_an_artwork_not_known() {
        let mut jobs = fixture_jobs();
        // An image saved from a page's `data:` address, of a card Scryfall gives no illustration id.
        jobs[4][0].url = None;
        jobs[4][0].illustration = None;
        let items = fixture_manifest(&jobs, ScryfallImage::ArtCrop).items;
        let json = serde_json::to_value(&items[5]).unwrap();
        assert!(json.get("url").is_none() && json.get("illustrationId").is_none());
        assert_eq!(json["page"], "https://www.artstation.com/artwork/birdsofparadise");
    }

    #[test]
    fn knows_the_sites_found_art_is_on_by_their_domains() {
        let cases = [
            (
                "https://www.artstation.com/artwork/birdsofparadise",
                Some(Site::Artstation),
            ),
            (
                "https://cdna.artstation.com/p/assets/images/birds.jpg",
                Some(Site::Artstation),
            ),
            ("https://ARTSTATION.com./artwork/x", Some(Site::Artstation)),
            ("https://www.deviantart.com/someone/art/birds-1", Some(Site::Deviantart)),
            ("https://www.pixiv.net/en/artworks/1", Some(Site::Pixiv)),
            ("https://i.pximg.net/img-original/img/1_p0.png", Some(Site::Pixiv)),
            ("https://www.instagram.com/p/birds/", Some(Site::Instagram)),
            ("https://scontent.cdninstagram.com/v/birds.jpg", Some(Site::Instagram)),
            // Look-alikes.
            ("https://artstation.com.evil.example/artwork/x", None),
            ("https://evilartstation.com/artwork/x", None),
            ("https://example.com/?from=https://www.artstation.com/", None),
            ("https://example.com/artstation.com/birds.jpg", None),
            // Not a site's address at all.
            ("data:image/png;base64,AAAA", None),
            ("not an address", None),
        ];
        for (address, site) in cases {
            assert_eq!(site_at(address), site, "{address}");
        }
    }

    /// The fixture's Birds of Paradise, as a found image with `page`, `url`
    /// and `site`, and the site the manifest names it with.
    fn site_sent(page: Option<&str>, url: Option<&str>, site: Option<Site>) -> Option<&'static str> {
        let mut jobs = fixture_jobs();
        let birds = &mut jobs[4][0];
        birds.page = page.map(str::to_owned);
        birds.url = url.map(str::to_owned);
        birds.site = site;
        let items = fixture_manifest(&jobs, ScryfallImage::ArtCrop).items;
        items[5].art.site
    }

    #[test]
    fn names_the_site_a_found_image_is_from_or_was_found_through() {
        let artstation = Some("https://www.artstation.com/artwork/birdsofparadise");
        let blog = Some("https://birds.example/gallery");
        let pixiv_image = Some("https://i.pximg.net/img-original/img/1_p0.png");
        // The image's own site, however the user got there.
        assert_eq!(site_sent(artstation, None, Some(Site::Lens)), Some("artstation"));
        assert_eq!(site_sent(blog, pixiv_image, Some(Site::Lens)), Some("pixiv"));
        assert_eq!(site_sent(None, pixiv_image, None), Some("pixiv"));
        // Elsewhere: where they started looking.
        assert_eq!(
            site_sent(blog, Some("https://birds.example/1.jpg"), Some(Site::Lens)),
            Some("lens")
        );
        assert_eq!(site_sent(blog, None, Some(Site::Website)), Some("website"));
        // Neither known: left out.
        assert_eq!(site_sent(blog, None, None), None);
        let mut jobs = fixture_jobs();
        jobs[4][0].page = None;
        jobs[4][0].url = None;
        jobs[4][0].site = None;
        let items = fixture_manifest(&jobs, ScryfallImage::ArtCrop).items;
        assert!(serde_json::to_value(&items[5]).unwrap().get("site").is_none());
    }

    #[test]
    fn only_found_images_name_a_site() {
        let mut jobs = fixture_jobs();
        // Scryfall's Sol Ring, as if a site were noted for it.
        jobs[2][0].site = Some(Site::Lens);
        let items = fixture_manifest(&jobs, ScryfallImage::ArtCrop).items;
        assert!(items[..5].iter().all(|item| item.art.site.is_none()));
        assert_eq!(items[5].art.site, Some("artstation"));
    }

    #[test]
    fn scryfalls_full_card_scans_are_sent_as_cards() {
        let items = fixture_manifest(&fixture_jobs(), ScryfallImage::FullCard).items;
        let kinds: Vec<_> = items.iter().map(|item| (item.art.origin, item.art.image)).collect();
        assert_eq!(
            kinds,
            [
                (Origin::Mtgpics, ImageKind::Art),
                (Origin::Mtgpics, ImageKind::Art),
                (Origin::Mtgpics, ImageKind::Art),
                (Origin::Scryfall, ImageKind::Card),
                (Origin::Mtgpics, ImageKind::Art),
                (Origin::Web, ImageKind::Art),
            ]
        );
    }

    #[test]
    fn a_card_scan_the_user_saved_anyway_is_sent_as_a_card() {
        let mut jobs = fixture_jobs();
        // Birds of Paradise, found on the web: the whole card, frame and all.
        jobs[4][0].whole_card = true;
        let items = fixture_manifest(&jobs, ScryfallImage::ArtCrop).items;
        assert_eq!(
            (items[5].art.origin, items[5].art.image),
            (Origin::Web, ImageKind::Card)
        );
        assert!(items[..5].iter().all(|item| item.art.image == ImageKind::Art));
    }

    #[test]
    fn an_empty_title_is_left_out() {
        let built = manifest(
            &fixture_jobs(),
            Some("  "),
            ScryfallImage::ArtCrop,
            new_id(),
            at(0),
            "0.3.0",
        );
        assert_eq!(built.title, None);
        assert_eq!(built.created_at, "1970-01-01T00:00:00Z");
        assert!(valid_id(&built.id), "{}", built.id);
    }

    #[test]
    fn writes_times_in_rfc3339() {
        assert_eq!(rfc3339(at(CREATED_AT)), "2026-11-09T14:03:12Z");
        assert_eq!(rfc3339(at(951_782_400)), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(at(1_735_689_599)), "2024-12-31T23:59:59Z");
    }

    #[test]
    fn checks_ids() {
        assert!(valid_id("18c3f7a9e2b-1f4"));
        assert!(!valid_id(""));
        assert!(!valid_id("../escape"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn writes_a_manifest_whole_and_clears_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("card-anvil");
        let built = fixture_manifest(&fixture_jobs(), ScryfallImage::ArtCrop);
        let path = write_manifest(&inbox, &built).unwrap();
        assert_eq!(path, inbox.join("18c3f7a9e2b-1f4.cardanvil-inbox"));
        // No temporary file left beside it.
        let names: Vec<_> = std::fs::read_dir(&inbox)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["18c3f7a9e2b-1f4.cardanvil-inbox"]);
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written, serde_json::to_value(&built).unwrap());

        let bad = Manifest {
            id: "../escape".into(),
            ..built
        };
        assert!(write_manifest(&inbox, &bad).is_err());

        clear_inbox_dir(&dir.path().join("never made"), SystemTime::now()).unwrap();
    }

    #[test]
    fn clears_only_manifests_left_long_enough() {
        let dir = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        let written = |name: &str, age: Duration| {
            let path = dir.path().join(name);
            std::fs::write(&path, b"{}").unwrap();
            let file = std::fs::File::options().write(true).open(&path).unwrap();
            file.set_modified(now - age).unwrap();
            path
        };
        let old = written("old.cardanvil-inbox", KEEP_UNTAKEN + Duration::from_secs(1));
        let half = written("old.cardanvil-inbox.part", KEEP_UNTAKEN * 2);
        // Another window's send, still waiting on a slow start.
        let waiting = written("waiting.cardanvil-inbox", TAKE_WITHIN * 2);
        let new = written("new.cardanvil-inbox", Duration::ZERO);
        std::fs::create_dir(dir.path().join("folder")).unwrap();

        clear_inbox_dir(dir.path(), now).unwrap();
        assert!(!old.exists() && !half.exists());
        assert!(waiting.exists() && new.exists());
        assert!(dir.path().join("folder").exists());

        // Written "after" now, by a clock set back a little since: a send may
        // still be waiting on them.
        clear_inbox_dir(dir.path(), now - Duration::from_secs(60)).unwrap();
        assert!(waiting.exists() && new.exists());
        // By far more, a clock that was wrong: they're as old as any.
        clear_inbox_dir(dir.path(), now - KEEP_UNTAKEN - Duration::from_secs(60)).unwrap();
        assert!(!new.exists() && !waiting.exists());
    }

    #[test]
    fn counts_a_manifest_stale_once_its_left_long_enough() {
        let now = SystemTime::now();
        let second = Duration::from_secs(1);
        assert!(!is_stale(now, now));
        assert!(!is_stale(now - KEEP_UNTAKEN, now));
        assert!(is_stale(now - KEEP_UNTAKEN - second, now));
        // From a clock set back since.
        assert!(!is_stale(now + TAKE_WITHIN, now));
        assert!(!is_stale(now + KEEP_UNTAKEN, now));
        assert!(is_stale(now + KEEP_UNTAKEN + second, now));
    }

    #[test]
    fn names_a_manifest_only_by_an_id_that_can_name_a_file() {
        let dir = Path::new("inbox");
        assert_eq!(
            manifest_path(dir, "18c3f7a9e2b-1f4"),
            Some(dir.join("18c3f7a9e2b-1f4.cardanvil-inbox"))
        );
        for id in ["", "../x", "a/b", "a\\b", "a b", &"x".repeat(65)] {
            assert_eq!(manifest_path(dir, id), None, "{id:?}");
        }
    }

    #[tokio::test]
    async fn waits_for_card_anvil_to_take_a_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("taken.cardanvil-inbox");
        std::fs::write(&path, b"{}").unwrap();
        let taker = {
            let path = path.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                std::fs::remove_file(path).unwrap();
            })
        };
        assert!(wait_until_taken(&path, Duration::from_secs(10)).await);
        taker.join().unwrap();

        let left = dir.path().join("left.cardanvil-inbox");
        std::fs::write(&left, b"{}").unwrap();
        let started = Instant::now();
        assert!(!wait_until_taken(&left, Duration::from_millis(500)).await);
        assert!(started.elapsed() >= Duration::from_millis(500));
        assert!(left.exists());
    }

    #[test]
    fn looks_where_the_windows_installer_says_first() {
        let registry = |hive: Hive, key: &str, name: &str| -> Option<String> {
            match (hive, key, name) {
                (Hive::CurrentUser, UNINSTALL_KEY, "InstallLocation") => {
                    Some(r#""C:\Users\me\AppData\Local\Card Anvil""#.into())
                }
                (Hive::CurrentUser, UNINSTALL_KEY, "MainBinaryName") => Some("Card-Anvil.exe".into()),
                // Installed for everyone too, with no binary name noted.
                (Hive::LocalMachine, UNINSTALL_KEY, "InstallLocation") => {
                    Some(r#""C:\Program Files\Card Anvil""#.into())
                }
                (Hive::CurrentUser, INSTALL_DIR_KEY, "") => Some(r"D:\Apps\Card Anvil".into()),
                // An MSI's name for it.
                (Hive::CurrentUser, INSTALL_DIR_KEY, "InstallDir") => Some(r"E:\Card Anvil".into()),
                _ => None,
            }
        };
        let var = |name: &str| match name {
            "LOCALAPPDATA" => Some(PathBuf::from(r"C:\Users\me\AppData\Local")),
            "ProgramFiles" => Some(PathBuf::from(r"C:\Program Files")),
            _ => None,
        };
        let candidates = windows_candidates(&registry, &var);
        assert_eq!(
            candidates,
            [
                Path::new(r"C:\Users\me\AppData\Local\Card Anvil").join("Card-Anvil.exe"),
                Path::new(r"C:\Program Files\Card Anvil").join("Card-Anvil.exe"),
                Path::new(r"D:\Apps\Card Anvil").join("Card-Anvil.exe"),
                Path::new(r"E:\Card Anvil").join("Card-Anvil.exe"),
                Path::new(r"C:\Users\me\AppData\Local")
                    .join("Card Anvil")
                    .join("Card-Anvil.exe"),
                Path::new(r"C:\Program Files").join("Card Anvil").join("Card-Anvil.exe"),
            ]
        );

        // Nothing recorded: the default folders.
        let candidates = windows_candidates(&|_: Hive, _: &str, _: &str| None, &var);
        assert_eq!(candidates.len(), 2);
        assert!(windows_candidates(&|_: Hive, _: &str, _: &str| Some("  ".into()), &|_: &str| None).is_empty());
    }

    #[test]
    fn looks_in_the_applications_folders_before_asking_spotlight() {
        let asked = Cell::new(false);
        let mdfind = || {
            asked.set(true);
            vec![PathBuf::from("/Volumes/Work/Card Anvil.app")]
        };
        let mut candidates = mac_candidates(Some(PathBuf::from("/Users/me")), &mdfind);
        assert_eq!(candidates.next(), Some(Path::new("/Applications").join(MAC_APP)));
        assert_eq!(
            candidates.next(),
            Some(Path::new("/Users/me").join("Applications").join(MAC_APP))
        );
        assert!(!asked.get());
        assert_eq!(candidates.next(), Some(PathBuf::from("/Volumes/Work/Card Anvil.app")));
        assert!(asked.get());
        assert_eq!(candidates.next(), None);
    }

    #[test]
    fn looks_on_the_path_on_linux() {
        let path = std::env::join_paths([Path::new("/usr/local/bin"), Path::new("/usr/bin")]).unwrap();
        assert_eq!(
            path_candidates(Some(path)),
            [
                Path::new("/usr/local/bin").join("Card-Anvil"),
                Path::new("/usr/local/bin").join("card-anvil"),
                Path::new("/usr/bin").join("Card-Anvil"),
                Path::new("/usr/bin").join("card-anvil"),
            ]
        );
        assert!(path_candidates(None).is_empty());
    }

    #[test]
    fn a_chosen_card_anvil_comes_first_while_its_there() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("Card-Anvil.exe");
        std::fs::write(&exe, b"").unwrap();
        let chosen = exe.to_string_lossy().into_owned();
        assert_eq!(locate(Some(&chosen)), Some(exe.clone()));
        let found = status(Some(&chosen));
        assert_eq!(found.path.as_deref(), Some(chosen.as_str()));
        assert!(found.chosen && !found.chosen_missing);

        // Gone: whatever is installed, and the user is told.
        std::fs::remove_file(&exe).unwrap();
        let gone = status(Some(&chosen));
        assert!(!gone.chosen && gone.chosen_missing);
        assert_ne!(gone.path.as_deref(), Some(chosen.as_str()));
        // None chosen is nothing missing.
        assert!(!status(None).chosen_missing);
        assert!(!status(Some(" ")).chosen_missing);
    }
}
