//! Scryfall's card data, kept on disk: its `default_cards` bulk file — every
//! printing, in English where there is one — imported into SQLite, so a list's
//! cards are looked up here rather than with a Scryfall request each. Scryfall
//! publishes a new file daily, and [`CardData::refresh`] fetches it once it has.
//! See <https://scryfall.com/docs/api/bulk-data>.
//!
//! Each new file is built into a database of its own, named for when Scryfall
//! published it, and swapped in whole; the app and the command line tool share
//! the folder.

use std::{
    collections::HashSet,
    fs,
    io::BufRead,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, SystemTime},
};

use flate2::read::GzDecoder;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{card::Card, http::Http, text::fold, Error, Result, APP_ID};

const SCHEMA: &str = "
CREATE TABLE cards (
  id INTEGER PRIMARY KEY,
  set_code TEXT NOT NULL,
  number TEXT NOT NULL,
  released_at TEXT NOT NULL,
  extra INTEGER NOT NULL,
  json TEXT NOT NULL
);
-- Every name a card is found by, folded: its own and its front face's, at
-- rank 0; its other faces', at rank 1; and its flavor names, at rank 2. A
-- name is found by the lowest rank that has it.
CREATE TABLE names (name TEXT NOT NULL, card INTEGER NOT NULL, rank INTEGER NOT NULL);
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

/// Built after the cards are in, which is much faster than keeping them up to
/// date row by row.
const INDEXES: &str = "
CREATE INDEX cards_by_number ON cards(set_code, number);
CREATE INDEX names_by_name ON names(name);
";

/// Databases are named `cards-v<SCHEMA_VERSION>-<published>.sqlite`. Changing
/// the schema means a new version, so a database built the old way is built
/// again rather than read.
const SCHEMA_VERSION: u32 = 2;

/// Leftovers of an update that didn't finish — a download, a half-built
/// database — are removed once they're this old.
const STALE_AFTER: Duration = Duration::from_secs(60 * 60);

/// Where the card data is kept: `<local data folder>/<APP_ID>/card-data`,
/// shared by the app and the command line tool.
pub fn default_card_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(APP_ID).join("card-data"))
}

/// One database built from one of Scryfall's files, opened read-only.
pub struct CardDb {
    conn: Mutex<Connection>,
    updated_at: String,
    cards: u32,
}

/// Which printings of a name [`CardDb::printings`] returns, and in what order.
#[derive(Debug, Clone, Copy)]
pub struct NameQuery {
    /// One printing per artwork: the first by release date in `newest_first` order.
    pub unique_art: bool,
    pub include_extras: bool,
    pub newest_first: bool,
}

impl CardDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        let (updated_at, cards) = {
            let meta = |key: &str| {
                conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                    row.get::<_, String>(0)
                })
            };
            (meta("updated_at")?, meta("cards")?.parse().unwrap_or(0))
        };
        Ok(Self {
            conn: Mutex::new(conn),
            updated_at,
            cards,
        })
    }

    /// When Scryfall published the data this was built from.
    pub fn updated_at(&self) -> &str {
        &self.updated_at
    }

    pub fn card_count(&self) -> u32 {
        self.cards
    }

    /// The printing with this set code and collector number.
    pub fn by_number(&self, set: &str, number: &str) -> Result<Option<Card>> {
        self.one(
            "SELECT json FROM cards WHERE set_code = ?1 AND number = ?2 ORDER BY id LIMIT 1",
            params![set.to_lowercase(), number.to_lowercase()],
        )
    }

    /// The newest printing of a name in a set, preferring a card of that name,
    /// or with that front face, to one with only a back face of that name.
    pub fn by_name_in_set(&self, name: &str, set: &str) -> Result<Option<Card>> {
        self.one(
            "SELECT c.json FROM names n JOIN cards c ON c.id = n.card \
             WHERE n.name = ?1 AND c.set_code = ?2 ORDER BY n.rank, c.released_at DESC, c.id LIMIT 1",
            params![fold(name), set.to_lowercase()],
        )
    }

    /// Every printing named `name`, or with a front face of that name, by
    /// release date — as Scryfall's `!"name"` search finds them, except that
    /// a card with only another face of that name, such as the prepared spell
    /// on a creature, is only returned when there's nothing better. Failing
    /// those, the printings with that flavor name, which Scryfall's search
    /// doesn't find.
    pub fn printings(&self, name: &str, query: NameQuery) -> Result<Vec<Card>> {
        let order = if query.newest_first { "DESC" } else { "ASC" };
        let sql = format!(
            "SELECT c.json FROM names n JOIN cards c ON c.id = n.card \
             WHERE n.name = ?1 AND (?2 OR c.extra = 0) AND n.rank = ( \
               SELECT MIN(m.rank) FROM names m JOIN cards d ON d.id = m.card \
               WHERE m.name = ?1 AND (?2 OR d.extra = 0) \
             ) ORDER BY c.released_at {order}, c.id"
        );
        let conn = self.lock();
        let mut statement = conn.prepare_cached(&sql)?;
        let rows = statement.query_map(params![fold(name), query.include_extras], |row| row.get::<_, String>(0))?;
        let mut cards = Vec::new();
        let mut arts = HashSet::new();
        for json in rows {
            let card: Card = serde_json::from_str(&json?)?;
            if query.unique_art {
                if let Some(art) = card.front_illustration() {
                    if !arts.insert(art.to_owned()) {
                        continue;
                    }
                }
            }
            cards.push(card);
        }
        Ok(cards)
    }

    fn one(&self, sql: &str, params: impl Params) -> Result<Option<Card>> {
        let json = self
            .lock()
            .prepare_cached(sql)?
            .query_row(params, |row| row.get::<_, String>(0))
            .optional()?;
        Ok(json.map(|json| serde_json::from_str(&json)).transpose()?)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Builds a database at `db` from a gzipped JSON-lines bulk file, one card per
/// line, decompressing and reading it a line at a time. A line that isn't a
/// card is skipped rather than failing the file. Blocking, and seconds of
/// work: keep it off the async runtime.
pub fn import(jsonl_gz: &Path, db: &Path, updated_at: &str) -> Result<u32> {
    let reader = std::io::BufReader::with_capacity(1 << 20, GzDecoder::new(fs::File::open(jsonl_gz)?));
    let mut conn = Connection::open(db)?;
    // Before any table exists, or it takes a VACUUM to apply.
    conn.pragma_update(None, "page_size", 32768)?;
    conn.pragma_update(None, "journal_mode", "OFF")?;
    conn.pragma_update(None, "synchronous", "OFF")?;
    conn.execute_batch(SCHEMA)?;

    let tx = conn.transaction()?;
    let mut count: u32 = 0;
    {
        let mut add_card = tx.prepare(
            "INSERT INTO cards (id, set_code, number, released_at, extra, json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let mut add_name = tx.prepare("INSERT INTO names (name, card, rank) VALUES (?1, ?2, ?3)")?;
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(card) = serde_json::from_str::<Card>(line) else {
                continue;
            };
            // An art series card is a print of another card's art, with that
            // card's name on both faces; the art is had from the card itself.
            if card.layout == "art_series" {
                continue;
            }
            count += 1;
            add_card.execute(params![
                count,
                card.set.to_lowercase(),
                card.collector_number.to_lowercase(),
                card.released_at.as_deref().unwrap_or_default(),
                card.is_extra(),
                serde_json::to_string(&card)?,
            ])?;
            for (name, rank) in names_of(&card) {
                add_name.execute(params![name, count, rank])?;
            }
        }
    }
    if count == 0 {
        return Err(Error::Other("Scryfall's card data file had no cards in it".into()));
    }
    tx.execute(
        "INSERT INTO meta (key, value) VALUES ('updated_at', ?1), ('cards', ?2)",
        params![updated_at, count.to_string()],
    )?;
    tx.commit()?;
    conn.execute_batch(INDEXES)?;
    Ok(count)
}

/// The names a card is found by, folded, each once at its best rank: see `SCHEMA`.
fn names_of(card: &Card) -> Vec<(String, u8)> {
    let mut candidates = vec![(card.name.as_str(), 0)];
    for (index, face) in card.card_faces.iter().enumerate() {
        candidates.push((face.name.as_str(), u8::from(index > 0)));
    }
    candidates.extend(card.flavor_name.as_deref().map(|name| (name, 2)));
    for face in &card.card_faces {
        candidates.extend(face.flavor_name.as_deref().map(|name| (name, 2)));
    }
    let mut names: Vec<(String, u8)> = Vec::new();
    for (name, rank) in candidates {
        let name = fold(name);
        if !name.is_empty() && !names.iter().any(|(seen, _)| *seen == name) {
            names.push((name, rank));
        }
    }
    names
}

/// One of Scryfall's bulk data files, from its list of them.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkFile {
    pub updated_at: String,
    pub jsonl_download_uri: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardDataStatus {
    /// When Scryfall published the data in use; `None` before any is downloaded.
    pub updated_at: Option<String>,
    pub cards: u32,
    /// A newer file is being downloaded and read.
    pub updating: bool,
    /// Why the last update failed, if it did.
    pub error: Option<String>,
}

/// The card data in use, and the updates to it.
pub struct CardData {
    dir: PathBuf,
    current: RwLock<Option<Arc<CardDb>>>,
    /// One update at a time.
    update: tokio::sync::Mutex<()>,
    updating: AtomicBool,
    error: Mutex<Option<String>>,
}

impl CardData {
    /// The card data in `dir`: the newest database there, if there is one.
    pub fn open(dir: PathBuf) -> Self {
        let current = newest_db(&dir).and_then(|path| CardDb::open(&path).ok()).map(Arc::new);
        Self {
            dir,
            current: RwLock::new(current),
            update: tokio::sync::Mutex::new(()),
            updating: AtomicBool::new(false),
            error: Mutex::new(None),
        }
    }

    /// The database to look cards up in, if one has been built.
    pub fn current(&self) -> Option<Arc<CardDb>> {
        self.current.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn status(&self) -> CardDataStatus {
        let current = self.current();
        CardDataStatus {
            updated_at: current.as_ref().map(|db| db.updated_at.clone()),
            cards: current.map_or(0, |db| db.cards),
            updating: self.updating.load(Ordering::Relaxed),
            error: self.error.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        }
    }

    /// Scryfall's newest `default_cards` file.
    pub async fn latest(http: &Http) -> Result<BulkFile> {
        let url = format!("{}/bulk-data/default_cards", http.endpoints().scryfall_api);
        http.get_json(&url).await?.ok_or_else(|| Error::Invalid {
            url,
            reason: "Scryfall has no default_cards bulk data".into(),
        })
    }

    /// Whether `file` is newer than the data in use.
    pub fn is_older_than(&self, file: &BulkFile) -> bool {
        self.current().map_or(true, |db| db.updated_at != file.updated_at)
    }

    /// Fetches and reads Scryfall's newest card data, if it's newer than the
    /// data in use. `true` when the data in use changed.
    pub async fn refresh(&self, http: &Http) -> Result<bool> {
        match Self::latest(http).await {
            Ok(file) => self.update_to(http, &file).await,
            Err(error) => {
                let failed = Err(error);
                self.set_error(&failed);
                failed
            }
        }
    }

    /// Downloads and reads `file`, unless the data in use is already from it.
    pub async fn update_to(&self, http: &Http, file: &BulkFile) -> Result<bool> {
        let _one_at_a_time = self.update.lock().await;
        if !self.is_older_than(file) {
            return Ok(false);
        }
        self.updating.store(true, Ordering::Relaxed);
        let result = self.build(http, file).await.map(|()| true);
        self.updating.store(false, Ordering::Relaxed);
        self.set_error(&result);
        result
    }

    async fn build(&self, http: &Http, file: &BulkFile) -> Result<()> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let name = db_name(&file.updated_at);
        let path = self.dir.join(&name);
        // The app and the command line tool share the folder, so the other may
        // have built this one already.
        if !path.exists() {
            let pid = std::process::id();
            let download = self.dir.join(format!("download-{pid}.jsonl.gz"));
            let partial = self.dir.join(format!("{name}.{pid}.tmp"));
            let _ = fs::remove_file(&partial);
            let built = async {
                if !http.download_file(&file.jsonl_download_uri, &download).await? {
                    return Err(Error::Invalid {
                        url: file.jsonl_download_uri.clone(),
                        reason: "Scryfall's card data file is missing".into(),
                    });
                }
                let (from, to, updated_at) = (download.clone(), partial.clone(), file.updated_at.clone());
                tokio::task::spawn_blocking(move || import(&from, &to, &updated_at))
                    .await
                    .map_err(|e| Error::Other(e.to_string()))??;
                if let Err(error) = fs::rename(&partial, &path) {
                    if !path.exists() {
                        return Err(error.into());
                    }
                }
                Ok(())
            }
            .await;
            let _ = fs::remove_file(&download);
            if built.is_err() {
                let _ = fs::remove_file(&partial);
            }
            built?;
        }
        let db = CardDb::open(&path)?;
        *self.current.write().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(db));
        self.remove_old(&path);
        Ok(())
    }

    /// Removes older databases and stale leftovers. One still open — by a run
    /// using it, or the other program — can't be removed on Windows, and is
    /// tried again next time.
    fn remove_old(&self, keep: &Path) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let old_db = is_db_name(&name) && path != keep;
            let leftover = (name.ends_with(".tmp") || name.ends_with(".jsonl.gz"))
                && entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .is_ok_and(|modified| SystemTime::now().duration_since(modified).unwrap_or_default() > STALE_AFTER);
            if old_db || leftover {
                let _ = fs::remove_file(path);
            }
        }
    }

    fn set_error<T>(&self, result: &Result<T>) {
        *self.error.lock().unwrap_or_else(|p| p.into_inner()) = result.as_ref().err().map(ToString::to_string);
    }
}

/// A database of any schema version.
fn is_db_name(name: &str) -> bool {
    name.starts_with("cards-") && name.ends_with(".sqlite")
}

/// The file name of a database of this schema version, from Scryfall's
/// `updated_at`.
fn db_name(updated_at: &str) -> String {
    let stamp: String = updated_at.chars().filter(char::is_ascii_digit).collect();
    format!("cards-v{SCHEMA_VERSION}-{stamp}.sqlite")
}

/// The newest database of this schema version in `dir`: their names hold the
/// time Scryfall published their data, so the last by name.
fn newest_db(dir: &Path) -> Option<PathBuf> {
    let prefix = format!("cards-v{SCHEMA_VERSION}-");
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_db_name(name) && name.starts_with(&prefix))
        .max()
        .map(|name| dir.join(name))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    /// Real cards from Scryfall: printings of Blightsteel Colossus (one of them
    /// a reversible card), Agadeem's Awakening, Karn, Lightning Bolt, Demonic\n    /// Tutor, and Emeritus of Woe, whose prepared spell is a Demonic Tutor.
    fn fixture_db(dir: &Path) -> CardDb {
        let jsonl = fs::read(format!(
            "{}/tests/fixtures/default_cards.jsonl",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let gz = dir.join("cards.jsonl.gz");
        let mut encoder = flate2::write::GzEncoder::new(fs::File::create(&gz).unwrap(), flate2::Compression::fast());
        encoder.write_all(&jsonl).unwrap();
        encoder.write_all(b"\nnot a card\n\n").unwrap();
        encoder.finish().unwrap();
        let db = dir.join("cards.sqlite");
        assert_eq!(import(&gz, &db, "2026-11-07T10:06:55.230+00:00").unwrap(), 27);
        CardDb::open(&db).unwrap()
    }

    fn labels(cards: &[Card]) -> Vec<String> {
        cards.iter().map(Card::label).collect()
    }

    #[test]
    fn finds_printings_by_set_and_number() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        assert_eq!(db.updated_at(), "2026-11-07T10:06:55.230+00:00");
        let card = db.by_number("SLD", "1079").unwrap().unwrap();
        assert_eq!(card.layout, "reversible_card");
        assert_eq!(card.card_faces.len(), 2);
        assert_eq!(
            db.by_number("plst", "2xm-235").unwrap().unwrap().label(),
            "Blightsteel Colossus (PLST) 2XM-235"
        );
        assert!(db.by_number("sld", "9999").unwrap().is_none());
    }

    #[test]
    fn finds_a_name_in_a_set_by_its_front_face() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        let card = db.by_name_in_set("agadeems awakening", "znr").unwrap().unwrap();
        assert_eq!(card.name, "Agadeem's Awakening // Agadeem, the Undercrypt");
    }

    #[test]
    fn lists_printings_by_release_date() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        let every = NameQuery {
            unique_art: false,
            include_extras: true,
            newest_first: true,
        };
        let newest = db.printings("Blightsteel Colossus", every).unwrap();
        assert_eq!(
            labels(&newest),
            [
                "Blightsteel Colossus (SLD) 2223",
                "Blightsteel Colossus (PLST) 2XM-235",
                "Blightsteel Colossus // Blightsteel Colossus (SLD) 1079",
                "Blightsteel Colossus (PRM) 82844",
                "Blightsteel Colossus (2XM) 235",
                "Blightsteel Colossus (2XM) 357",
                "Blightsteel Colossus (MBS) 99",
            ]
        );
        let oldest = db
            .printings(
                "Blightsteel Colossus",
                NameQuery {
                    newest_first: false,
                    ..every
                },
            )
            .unwrap();
        assert_eq!(oldest[0].label(), "Blightsteel Colossus (MBS) 99");
    }

    #[test]
    fn lists_one_printing_per_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        let query = NameQuery {
            unique_art: true,
            include_extras: true,
            newest_first: false,
        };
        let arts = db.printings("Blightsteel Colossus", query).unwrap();
        let labels = labels(&arts);
        // MBS 99, 2XM 235 and PLST 2XM-235 share Chris Rahn's art: the oldest stays.
        assert!(
            labels.contains(&"Blightsteel Colossus (MBS) 99".to_owned()),
            "{labels:?}"
        );
        assert!(
            !labels.contains(&"Blightsteel Colossus (2XM) 235".to_owned()),
            "{labels:?}"
        );
        assert!(
            !labels.contains(&"Blightsteel Colossus (PLST) 2XM-235".to_owned()),
            "{labels:?}"
        );
        assert!(
            labels.contains(&"Blightsteel Colossus (SLD) 2223".to_owned()),
            "{labels:?}"
        );
    }

    #[test]
    fn finds_a_double_faced_card_by_either_face_or_its_whole_name() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        let query = NameQuery {
            unique_art: false,
            include_extras: true,
            newest_first: true,
        };
        for name in [
            "Agadeem's Awakening",
            "Agadeem, the Undercrypt",
            "Agadeem's Awakening // Agadeem, the Undercrypt",
        ] {
            assert_eq!(db.printings(name, query).unwrap().len(), 4, "{name}");
        }
        assert!(db.printings("Agadeem", query).unwrap().is_empty());
    }

    #[test]
    fn prefers_a_card_of_the_name_to_one_with_a_back_face_of_it() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        let query = NameQuery {
            unique_art: false,
            include_extras: true,
            newest_first: true,
        };
        // Emeritus of Woe is newer, but it's a creature whose prepared spell
        // is a Demonic Tutor.
        let tutors = db.printings("Demonic Tutor", query).unwrap();
        assert_eq!(
            labels(&tutors),
            [
                "Demonic Tutor (CMM) 150",
                "Demonic Tutor (CMM) 509",
                "Demonic Tutor (CMM) 696"
            ]
        );
        assert_eq!(
            db.by_name_in_set("Demonic Tutor", "sos")
                .unwrap()
                .unwrap()
                .collector_number,
            "80"
        );
        // Asked for by its own name, it's found.
        assert_eq!(db.printings("Emeritus of Woe", query).unwrap().len(), 2);
    }

    #[test]
    fn finds_a_printing_by_its_flavor_name() {
        let dir = tempfile::tempdir().unwrap();
        let db = fixture_db(dir.path());
        let query = NameQuery {
            unique_art: true,
            include_extras: true,
            newest_first: true,
        };
        // Moxfield exports the Secret Lair Blightsteel Colossus by the name on
        // its faces.
        let megatron = db.printings("Megatron", query).unwrap();
        assert_eq!(
            labels(&megatron),
            ["Blightsteel Colossus // Blightsteel Colossus (SLD) 1079"]
        );
        assert_eq!(
            db.by_name_in_set("Megatron", "sld").unwrap().unwrap().collector_number,
            "1079"
        );
        // A card's own name still finds every art of it, the Megatron one included.
        let arts = labels(&db.printings("Blightsteel Colossus", query).unwrap());
        assert_eq!(arts.len(), 4, "{arts:?}");
        assert!(arts.contains(&megatron[0].label()), "{arts:?}");
    }

    #[test]
    fn ranks_each_name_a_card_has_once() {
        let card: Card = serde_json::from_str(
            r#"{"name": "Front // Back", "flavor_name": "Front", "card_faces": [
                {"name": "Front", "flavor_name": "Crossover"}, {"name": "Back", "flavor_name": "Crossover"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(
            names_of(&card),
            [
                ("front back".to_owned(), 0),
                ("front".to_owned(), 0),
                ("back".to_owned(), 1),
                ("crossover".to_owned(), 2),
            ]
        );
    }

    #[test]
    fn opens_the_newest_database_of_this_schema_and_reports_it() {
        let dir = tempfile::tempdir().unwrap();
        fixture_db(dir.path());
        fs::rename(
            dir.path().join("cards.sqlite"),
            dir.path().join(db_name("2026-11-07T10:06:55.230+00:00")),
        )
        .unwrap();
        // An older file, and a newer one built the old way, are passed over.
        fs::write(dir.path().join(db_name("2026-11-06T10:00:00.000+00:00")), b"").unwrap();
        fs::write(dir.path().join("cards-20261108000000000000.sqlite"), b"").unwrap();
        let data = CardData::open(dir.path().to_owned());
        let status = data.status();
        assert_eq!(status.updated_at.as_deref(), Some("2026-11-07T10:06:55.230+00:00"));
        assert_eq!((status.cards, status.updating, status.error), (27, false, None));
        assert!(CardData::open(dir.path().join("missing")).current().is_none());
    }
}
