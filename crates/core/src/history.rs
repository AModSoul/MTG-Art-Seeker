//! A log of every printing whose art has been saved, with where on MTGPics
//! each of its faces' art was found. The next time the printing is asked for,
//! its art is downloaded from there rather than found again; MTGPics is only
//! searched again when that address no longer has the image.
//!
//! Kept in SQLite in the local data folder, shared by the app and the command
//! line tool.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{card::Card, text::fold, Result, APP_ID};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS fetched (
  card_id TEXT PRIMARY KEY,   -- Scryfall's id for the printing
  name TEXT NOT NULL,
  set_code TEXT NOT NULL,
  number TEXT NOT NULL,
  mtgpics TEXT NOT NULL,      -- JSON: per face, the MTGPics image URL, or null
  checked_at INTEGER NOT NULL, -- when MTGPics was last searched for it
  fetched_at INTEGER NOT NULL  -- when its art was last saved
);
CREATE TABLE IF NOT EXISTS artist_websites (
  artist TEXT PRIMARY KEY,     -- the artist's name, folded
  url TEXT,                    -- the website MTGPics links, or null for none
  checked_at INTEGER NOT NULL  -- when MTGPics was asked
);
CREATE TABLE IF NOT EXISTS found_by_hand (
  illustration_id TEXT PRIMARY KEY, -- Scryfall's id for the artwork
  url TEXT NOT NULL,           -- where the user saved a larger copy from
  page TEXT,                   -- the page it was on
  saved_at INTEGER NOT NULL
);
";

/// The log's version, kept in SQLite's `user_version`. Opening an older log
/// brings it up to date; see [`migrate`].
const VERSION: i64 = 1;

/// A face MTGPics had no art for is looked for again after this long, in case
/// MTGPics has added it since.
pub const RECHECK_MISSING_AFTER: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Where the log is kept: `<local data folder>/<APP_ID>/history.sqlite`.
pub fn default_history_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(APP_ID).join("history.sqlite"))
}

pub struct History {
    conn: Mutex<Connection>,
}

/// A larger copy of an artwork the user found and saved themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundByHand {
    pub url: String,
    pub page: Option<String>,
}

/// What MTGPics said of an artist's website, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownWebsite {
    /// The website, or `None` when MTGPics links none.
    pub url: Option<String>,
    pub checked_at: SystemTime,
}

impl KnownWebsite {
    /// Whether MTGPics was asked recently enough not to ask again.
    pub fn is_recent(&self) -> bool {
        self.checked_at.elapsed().is_ok_and(|age| age < RECHECK_MISSING_AFTER)
    }
}

/// What the log knows of a printing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remembered {
    /// Per face, where its MTGPics art was found; `None` where MTGPics had none.
    pub mtgpics: Vec<Option<String>>,
    /// When MTGPics was last searched for the printing.
    pub checked_at: SystemTime,
}

impl Remembered {
    /// Whether MTGPics was searched recently enough that a face it had no art
    /// for needn't be looked for again yet.
    pub fn missing_is_recent(&self) -> bool {
        self.checked_at.elapsed().is_ok_and(|age| age < RECHECK_MISSING_AFTER)
    }
}

impl History {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        // The app and the command line tool may write at once.
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// What the log knows of `card`, if its art has been saved before with
    /// `faces` faces.
    pub fn remembered(&self, card: &Card, faces: usize) -> Result<Option<Remembered>> {
        let row = self
            .lock()
            .prepare_cached("SELECT mtgpics, checked_at FROM fetched WHERE card_id = ?1")?
            .query_row([&card.id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .optional()?;
        let Some((mtgpics, checked_at)) = row else {
            return Ok(None);
        };
        let mtgpics: Vec<Option<String>> = serde_json::from_str(&mtgpics)?;
        Ok((mtgpics.len() == faces).then(|| Remembered {
            mtgpics,
            checked_at: UNIX_EPOCH + Duration::from_secs(checked_at.max(0).unsigned_abs()),
        }))
    }

    /// Logs that `card`'s art was saved, with where on MTGPics each face's art
    /// was found, and when MTGPics was searched.
    pub fn remember(&self, card: &Card, mtgpics: &[Option<String>], checked_at: SystemTime) -> Result<()> {
        if card.id.is_empty() {
            return Ok(());
        }
        let secs = |time: SystemTime| {
            time.duration_since(UNIX_EPOCH)
                .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
        };
        self.lock()
            .prepare_cached(
                "INSERT INTO fetched (card_id, name, set_code, number, mtgpics, checked_at, fetched_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT (card_id) DO UPDATE SET name = ?2, set_code = ?3, number = ?4, \
                 mtgpics = ?5, checked_at = ?6, fetched_at = ?7",
            )?
            .execute(params![
                card.id,
                card.name,
                card.set,
                card.collector_number,
                serde_json::to_string(mtgpics)?,
                secs(checked_at),
                secs(SystemTime::now()),
            ])?;
        Ok(())
    }

    /// What MTGPics said of `artist`'s website, if it has been asked.
    pub fn artist_website(&self, artist: &str) -> Result<Option<KnownWebsite>> {
        Ok(self
            .lock()
            .prepare_cached("SELECT url, checked_at FROM artist_websites WHERE artist = ?1")?
            .query_row([fold(artist)], |row| {
                Ok(KnownWebsite {
                    url: row.get(0)?,
                    checked_at: UNIX_EPOCH + Duration::from_secs(row.get::<_, i64>(1)?.max(0).unsigned_abs()),
                })
            })
            .optional()?)
    }

    /// Logs what MTGPics said of `artist`'s website: `url`, or none.
    pub fn remember_artist_website(&self, artist: &str, url: Option<&str>) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX));
        self.lock()
            .prepare_cached(
                "INSERT INTO artist_websites (artist, url, checked_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (artist) DO UPDATE SET url = ?2, checked_at = ?3",
            )?
            .execute(params![fold(artist), url, now])?;
        Ok(())
    }

    /// Where the user found a larger copy of the artwork `illustration`, if
    /// they have. Every printing of the art is saved from there.
    pub fn found_by_hand(&self, illustration: &str) -> Result<Option<FoundByHand>> {
        Ok(self
            .lock()
            .prepare_cached("SELECT url, page FROM found_by_hand WHERE illustration_id = ?1")?
            .query_row([illustration], |row| {
                Ok(FoundByHand {
                    url: row.get(0)?,
                    page: row.get(1)?,
                })
            })
            .optional()?)
    }

    pub fn remember_found_by_hand(&self, illustration: &str, found: &FoundByHand) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX));
        self.lock()
            .prepare_cached(
                "INSERT INTO found_by_hand (illustration_id, url, page, saved_at) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (illustration_id) DO UPDATE SET url = ?2, page = ?3, saved_at = ?4",
            )?
            .execute(params![illustration, found.url, found.page, now])?;
        Ok(())
    }

    pub fn forget_found_by_hand(&self, illustration: &str) -> Result<()> {
        self.lock()
            .prepare_cached("DELETE FROM found_by_hand WHERE illustration_id = ?1")?
            .execute([illustration])?;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Brings a log written by an older version up to date.
fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        // Before version 1, a card with two faces could have its faces' MTGPics
        // art logged the wrong way round, when its page showed the back's art
        // first. Forget every card with more than one face, so each is found
        // again, once, the right way round.
        conn.execute("DELETE FROM fetched WHERE json_array_length(mtgpics) > 1", [])?;
    }
    if version < VERSION {
        conn.pragma_update(None, "user_version", VERSION)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str) -> Card {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": "Karn, Scion of Urza", "set": "dom", "collector_number": "1"
        }))
        .unwrap()
    }

    #[test]
    fn forgets_two_faced_cards_logged_before_version_1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.sqlite");
        let url = |n: &str| Some(format!("https://www.mtgpics.com/pics/art/stx/{n}.jpg"));
        {
            let history = History::open(&path).unwrap();
            history
                .remember(&card("one-face"), &[url("001")], SystemTime::now())
                .unwrap();
            history
                .remember(&card("two-faces"), &[url("715"), url("006")], SystemTime::now())
                .unwrap();
            // As a log from before version 1 would be.
            history.lock().pragma_update(None, "user_version", 0).unwrap();
        }
        let history = History::open(&path).unwrap();
        assert!(history.remembered(&card("one-face"), 1).unwrap().is_some());
        assert_eq!(history.remembered(&card("two-faces"), 2).unwrap(), None);

        // Logged again after the update, it stays.
        history
            .remember(&card("two-faces"), &[url("006"), url("715")], SystemTime::now())
            .unwrap();
        drop(history);
        let history = History::open(&path).unwrap();
        assert!(history.remembered(&card("two-faces"), 2).unwrap().is_some());
    }

    #[test]
    fn remembers_where_a_printings_art_was_found() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("history.sqlite");
        let history = History::open(&path).unwrap();
        let karn = card("karn-id");
        assert_eq!(history.remembered(&karn, 1).unwrap(), None);

        let url = Some("https://www.mtgpics.com/pics/art/dom/001_1.jpg".to_owned());
        let now = SystemTime::now();
        history.remember(&karn, std::slice::from_ref(&url), now).unwrap();
        // Read back by the next process, too.
        drop(history);
        let history = History::open(&path).unwrap();
        let remembered = history.remembered(&karn, 1).unwrap().unwrap();
        assert_eq!(remembered.mtgpics, [url]);
        assert!(remembered.missing_is_recent());
        // Asked for with another number of faces, it's a different thing.
        assert_eq!(history.remembered(&karn, 2).unwrap(), None);

        // Saving it again replaces the entry.
        let long_ago = now - RECHECK_MISSING_AFTER - Duration::from_secs(60);
        history.remember(&karn, &[None], long_ago).unwrap();
        let remembered = history.remembered(&karn, 1).unwrap().unwrap();
        assert_eq!(remembered.mtgpics, [None]);
        assert!(!remembered.missing_is_recent());
    }
}
