//! Preconstructed decks: every precon MTGJSON lists — Commander decks, Secret
//! Lair drops, theme decks, Jumpstart packs… — to pick one and download its
//! cards' art. See <https://mtgjson.com/data-models/deck/>.
//!
//! The catalog is built from two of MTGJSON's files: `DeckList`, which names
//! each deck and its file, and `SetList`, for its set's name and the sealed
//! product it's sold as, whose TCGplayer id gives a picture of its box. It's
//! kept on disk and built again once a day. A deck's cards are read from its
//! own file when it's downloaded: each names its printing by set and
//! collector number, so a run asks for exactly the cards in the box.

use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use flate2::read::GzDecoder;
use image::{imageops, DynamicImage, Rgba, RgbaImage};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use ts_rs::TS;

use crate::{http::Http, list::CardLine, text::fold, Error, Result, APP_ID};

/// How long a catalog is used before it's built again. MTGJSON publishes
/// daily, and precons come out a few times a month.
const CATALOG_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Bumped when what's kept in `catalog.json` changes, so an old one is built again.
const CATALOG_VERSION: u32 = 1;

/// How many box pictures are downloaded at once.
const IMAGE_DOWNLOADS: usize = 4;

/// The width box pictures are kept at: about twice what the app shows.
const BOX_WIDTH: u32 = 360;

/// Where the deck catalog and box pictures are kept: `<local data folder>/<APP_ID>/decks`.
pub fn default_decks_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join(APP_ID).join("decks"))
}

/// A deck, as the app lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeckSummary {
    /// MTGJSON's name for the deck's file, by which it's asked for.
    pub file_name: String,
    pub name: String,
    /// The code of the set it was released in, upper case.
    pub code: String,
    pub set_name: String,
    /// MTGJSON's kind of deck: `Commander Deck`, `Secret Lair Drop`, `Theme Deck`…
    #[serde(rename = "type")]
    pub deck_type: String,
    /// `YYYY-MM-DD`; `None` for a deck that wasn't sold as a product.
    pub release_date: Option<String>,
    /// Whether a picture of its box is known of.
    pub has_box: bool,
}

/// The decks a search matched, best first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeckSearch {
    /// The first of them, up to the limit asked for.
    pub decks: Vec<DeckSummary>,
    /// How many matched in all.
    pub total: u32,
}

/// A deck in the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Deck {
    file_name: String,
    name: String,
    code: String,
    set_name: String,
    deck_type: String,
    release_date: Option<String>,
    /// TCGplayer's id for the product the deck is sold as.
    box_id: Option<String>,
}

impl Deck {
    fn summary(&self) -> DeckSummary {
        DeckSummary {
            file_name: self.file_name.clone(),
            name: self.name.clone(),
            code: self.code.clone(),
            set_name: self.set_name.clone(),
            deck_type: self.deck_type.clone(),
            release_date: self.release_date.clone(),
            has_box: self.box_id.is_some(),
        }
    }
}

/// `catalog.json`.
#[derive(Serialize, Deserialize)]
struct CatalogFile {
    version: u32,
    /// When it was built, in seconds since 1970.
    built: u64,
    decks: Vec<Deck>,
}

/// The catalog in use, newest deck first, with each deck's names folded for searching.
struct Catalog {
    built: u64,
    decks: Vec<Deck>,
    folded: Vec<Folded>,
}

struct Folded {
    name: String,
    set_name: String,
    deck_type: String,
}

impl Catalog {
    fn new(built: u64, mut decks: Vec<Deck>) -> Self {
        decks.sort_by(|a, b| {
            // Newest first; undated decks last.
            b.release_date
                .is_some()
                .cmp(&a.release_date.is_some())
                .then_with(|| b.release_date.cmp(&a.release_date))
                .then_with(|| a.set_name.cmp(&b.set_name))
                .then_with(|| a.name.cmp(&b.name))
        });
        let folded = decks
            .iter()
            .map(|deck| Folded {
                name: fold(&deck.name),
                set_name: fold(&deck.set_name),
                deck_type: fold(&deck.deck_type),
            })
            .collect();
        Self { built, decks, folded }
    }

    fn is_fresh(&self) -> bool {
        now().saturating_sub(self.built) < CATALOG_MAX_AGE.as_secs()
    }

    fn search(&self, filter: &DeckFilter, limit: usize) -> DeckSearch {
        let mut total = 0;
        let mut decks = Vec::new();
        for (deck, folded) in self.decks.iter().zip(&self.folded) {
            if filter.matches(deck, folded) {
                total += 1;
                if decks.len() < limit {
                    decks.push(deck.summary());
                }
            }
        }
        DeckSearch { decks, total }
    }

    fn deck(&self, file_name: &str) -> Option<&Deck> {
        self.decks.iter().find(|deck| deck.file_name == file_name)
    }
}

/// The deck catalog and the pictures of the boxes, kept in one folder.
pub struct Decks {
    dir: PathBuf,
    catalog: RwLock<Option<Arc<Catalog>>>,
    /// One build at a time.
    update: tokio::sync::Mutex<()>,
    downloads: Semaphore,
    /// Boxes whose picture couldn't be had this session, so they aren't asked for again.
    failed: Mutex<HashSet<String>>,
    temp_ids: AtomicU64,
}

impl Decks {
    /// The decks kept in `dir`. Nothing is read until they're asked for.
    pub fn open(dir: PathBuf) -> Self {
        Self {
            dir,
            catalog: RwLock::new(None),
            update: tokio::sync::Mutex::new(()),
            downloads: Semaphore::new(IMAGE_DOWNLOADS),
            failed: Mutex::new(HashSet::new()),
            temp_ids: AtomicU64::new(0),
        }
    }

    /// Where box pictures are kept, for the app to show them from.
    pub fn images_dir(&self) -> PathBuf {
        self.dir.join("boxes")
    }

    /// The decks `query` matches, newest first: up to `limit` of them, and how
    /// many there are. Builds the catalog first if there's none, or it's a day old.
    pub async fn search(&self, http: &Http, query: &str, limit: usize) -> Result<DeckSearch> {
        let filter = DeckFilter::parse(query).map_err(Error::Other)?;
        Ok(self.catalog(http).await?.search(&filter, limit))
    }

    /// A picture of the deck's box with its background cut away, downloaded
    /// the first time it's asked for; `None` when there's none to be had.
    pub async fn box_image(&self, http: &Http, file_name: &str) -> Result<Option<PathBuf>> {
        let catalog = self.catalog(http).await?;
        let Some(id) = catalog.deck(file_name).and_then(|deck| deck.box_id.clone()) else {
            return Ok(None);
        };
        let path = self.images_dir().join(format!("{id}.png"));
        if path.exists() {
            return Ok(Some(path));
        }
        if lock(&self.failed).contains(&id) {
            return Ok(None);
        }
        let _turn = self
            .downloads
            .acquire()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        // Another request may have saved it while this one waited.
        if path.exists() {
            return Ok(Some(path));
        }
        match self.download_box(http, &id, &path).await {
            Ok(true) => Ok(Some(path)),
            Ok(false) => {
                lock(&self.failed).insert(id);
                Ok(None)
            }
            Err(error) => {
                lock(&self.failed).insert(id);
                Err(error)
            }
        }
    }

    /// Saves TCGplayer's picture of product `id` to `path`; `false` if TCGplayer has none.
    async fn download_box(&self, http: &Http, id: &str, path: &Path) -> Result<bool> {
        let url = format!("{}/product/{id}_in_1000x1000.jpg", http.endpoints().box_images);
        let Some(bytes) = http.get_bytes(&url).await? else {
            return Ok(false);
        };
        let image = image::load_from_memory(&bytes).map_err(|e| Error::Invalid {
            url: url.clone(),
            reason: e.to_string(),
        })?;
        let dir = self.images_dir();
        tokio::fs::create_dir_all(&dir).await?;
        let partial = dir.join(format!(
            "{id}.{}.{}.tmp",
            std::process::id(),
            self.temp_ids.fetch_add(1, Ordering::Relaxed)
        ));
        let (to, done) = (partial.clone(), path.to_owned());
        let saved = tokio::task::spawn_blocking(move || -> Result<()> {
            let cut = cut_out_box(&image);
            let cut = if cut.width() > BOX_WIDTH {
                let height = (u64::from(cut.height()) * u64::from(BOX_WIDTH) / u64::from(cut.width())) as u32;
                imageops::resize(&cut, BOX_WIDTH, height.max(1), imageops::FilterType::Lanczos3)
            } else {
                cut
            };
            cut.save_with_format(&to, image::ImageFormat::Png)
                .map_err(|e| Error::Other(e.to_string()))?;
            std::fs::rename(&to, &done)?;
            Ok(())
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
        if saved.is_err() {
            let _ = tokio::fs::remove_file(&partial).await;
        }
        saved.map(|()| true)
    }

    /// The catalog: the one in use, the one on disk, or a new one. One that's
    /// a day old is built again, and used still if that fails.
    async fn catalog(&self, http: &Http) -> Result<Arc<Catalog>> {
        if let Some(catalog) = self.current().filter(|catalog| catalog.is_fresh()) {
            return Ok(catalog);
        }
        let _one_at_a_time = self.update.lock().await;
        let mut current = self.current();
        if current.is_none() {
            current = self.read().map(Arc::new);
            self.set(current.clone());
        }
        if let Some(catalog) = current.as_ref().filter(|catalog| catalog.is_fresh()) {
            return Ok(catalog.clone());
        }
        match self.build(http).await {
            Ok(catalog) => {
                let catalog = Arc::new(catalog);
                self.set(Some(catalog.clone()));
                Ok(catalog)
            }
            Err(error) => current.ok_or(error),
        }
    }

    fn current(&self) -> Option<Arc<Catalog>> {
        self.catalog.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn set(&self, catalog: Option<Arc<Catalog>>) {
        *self.catalog.write().unwrap_or_else(|p| p.into_inner()) = catalog;
    }

    fn catalog_path(&self) -> PathBuf {
        self.dir.join("catalog.json")
    }

    fn read(&self) -> Option<Catalog> {
        let file: CatalogFile = serde_json::from_slice(&std::fs::read(self.catalog_path()).ok()?).ok()?;
        (file.version == CATALOG_VERSION).then(|| Catalog::new(file.built, file.decks))
    }

    async fn build(&self, http: &Http) -> Result<Catalog> {
        let base = &http.endpoints().mtgjson;
        let fetch = |name: &'static str| async move {
            let url = format!("{base}/{name}.json.gz");
            http.get_bytes(&url).await?.ok_or_else(|| Error::Invalid {
                url,
                reason: "MTGJSON's file is missing".into(),
            })
        };
        let set_list = fetch("SetList").await?;
        let deck_list = fetch("DeckList").await?;
        let decks = tokio::task::spawn_blocking(move || -> Result<Vec<Deck>> {
            let set_list: MtgjsonFile<Vec<SetEntry>> = serde_json::from_slice(&gunzip(&set_list)?)?;
            let deck_list: MtgjsonFile<Vec<ListedDeck>> = serde_json::from_slice(&gunzip(&deck_list)?)?;
            Ok(catalog_decks(set_list.data, deck_list.data))
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
        if decks.is_empty() {
            return Err(Error::Other("MTGJSON's list of decks was empty".into()));
        }
        let file = CatalogFile {
            version: CATALOG_VERSION,
            built: now(),
            decks,
        };
        // Kept for next time; a catalog that can't be written is still used.
        if tokio::fs::create_dir_all(&self.dir).await.is_ok() {
            let partial = self.dir.join(format!("catalog.{}.tmp", std::process::id()));
            if tokio::fs::write(&partial, serde_json::to_vec(&file)?).await.is_ok() {
                let _ = tokio::fs::rename(&partial, self.catalog_path()).await;
            }
        }
        Ok(Catalog::new(file.built, file.decks))
    }
}

fn gunzip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut out)?;
    Ok(out)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

// --- MTGJSON's files, as much of them as is read -----------------------------

#[derive(Deserialize)]
struct MtgjsonFile<T> {
    data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetEntry {
    code: String,
    name: String,
    #[serde(default)]
    decks: Vec<SetDeck>,
    #[serde(default)]
    sealed_product: Vec<SealedProduct>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetDeck {
    name: String,
    #[serde(default)]
    sealed_product_uuids: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SealedProduct {
    uuid: String,
    #[serde(default)]
    identifiers: SealedIdentifiers,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SealedIdentifiers {
    tcgplayer_product_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedDeck {
    code: String,
    file_name: String,
    name: String,
    release_date: Option<String>,
    #[serde(rename = "type")]
    deck_type: String,
}

/// Each deck in `DeckList`, with its set's name and its box from `SetList`.
fn catalog_decks(sets: Vec<SetEntry>, listed: Vec<ListedDeck>) -> Vec<Deck> {
    let mut set_names = HashMap::new();
    let mut products = HashMap::new();
    let mut boxes: HashMap<(String, String), Vec<String>> = HashMap::new();
    for set in sets {
        for product in set.sealed_product {
            // An id is a number; anything else isn't one to build a URL from.
            if let Some(id) = product
                .identifiers
                .tcgplayer_product_id
                .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
            {
                products.insert(product.uuid, id);
            }
        }
        for deck in set.decks {
            boxes.insert(
                (set.code.clone(), deck.name),
                deck.sealed_product_uuids.unwrap_or_default(),
            );
        }
        set_names.insert(set.code, set.name);
    }
    listed
        .into_iter()
        .map(|deck| {
            let key = (deck.code, deck.name);
            let box_id = boxes
                .get(&key)
                .and_then(|uuids| uuids.iter().find_map(|uuid| products.get(uuid)))
                .cloned();
            let (code, name) = key;
            Deck {
                file_name: deck.file_name,
                set_name: set_names.get(&code).cloned().unwrap_or_else(|| code.clone()),
                code,
                name,
                deck_type: deck.deck_type,
                release_date: deck.release_date.filter(|date| !date.is_empty()),
                box_id,
            }
        })
        .collect()
}

// --- A deck's cards ----------------------------------------------------------

/// The cards in a deck, each once, as list lines that name their printing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckCards {
    pub name: String,
    pub lines: Vec<CardLine>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeckFile {
    name: String,
    #[serde(default)]
    commander: Vec<DeckCard>,
    #[serde(default)]
    main_board: Vec<DeckCard>,
    #[serde(default)]
    side_board: Vec<DeckCard>,
    #[serde(default)]
    planes: Vec<DeckCard>,
    #[serde(default)]
    schemes: Vec<DeckCard>,
    #[serde(default)]
    tokens: Vec<DeckCard>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeckCard {
    name: String,
    set_code: String,
    number: String,
}

/// Reads deck `file_name`'s cards from MTGJSON: its commanders first, then the
/// rest of the deck, its sideboard, planes, schemes and tokens. The oversized
/// display copies of a commander some older decks came with are left out.
pub async fn deck_cards(http: &Http, file_name: &str) -> Result<DeckCards> {
    if file_name.is_empty() || file_name.len() > 200 || !file_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(Error::Other(format!(
            "\u{201c}{file_name}\u{201d} isn't one of MTGJSON's decks"
        )));
    }
    let mut url = Url::parse(&http.endpoints().mtgjson).map_err(|e| Error::Other(e.to_string()))?;
    url.path_segments_mut()
        .map_err(|()| Error::Other("MTGJSON's address can't take a path".into()))?
        .pop_if_empty()
        .push("decks")
        .push(&format!("{file_name}.json.gz"));
    let bytes = http
        .get_bytes(url.as_str())
        .await?
        .ok_or_else(|| Error::Other(format!("MTGJSON no longer has the deck \u{201c}{file_name}\u{201d}")))?;
    let deck: MtgjsonFile<DeckFile> = serde_json::from_slice(&gunzip(&bytes)?)?;
    Ok(deck_lines(deck.data))
}

fn deck_lines(deck: DeckFile) -> DeckCards {
    let mut seen = HashSet::new();
    let lines = [
        deck.commander,
        deck.main_board,
        deck.side_board,
        deck.planes,
        deck.schemes,
        deck.tokens,
    ]
    .into_iter()
    .flatten()
    .filter_map(|card| {
        // MTGJSON writes a few set codes that are Windows device names with an
        // underscore after them (`CON_`); Scryfall's are without.
        let set = card.set_code.trim_end_matches('_').to_lowercase();
        if set.is_empty() || card.number.is_empty() || !seen.insert((set.clone(), card.number.clone())) {
            return None;
        }
        let line = format!("{} ({}) {}", card.name, set.to_uppercase(), card.number);
        Some(CardLine {
            line,
            name: card.name,
            set: Some(set),
            number: Some(card.number),
        })
    })
    .collect();
    DeckCards { name: deck.name, lines }
}

// --- Searching ---------------------------------------------------------------

/// A search of the decks, in a few of Scryfall's words: `set:tdc`,
/// `type:commander`, `name:abzan`, `year>=2024`, `date<2020-06`, a bare word
/// for the deck's or its set's name, `"quoted words"`, and `-` before any of
/// them to leave out what it matches. Every part has to match.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeckFilter(Vec<Clause>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Clause {
    negated: bool,
    term: Term,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Term {
    /// The deck's or its set's name has these words.
    Word(String),
    Name(String),
    Set(String),
    Type(String),
    Year(Compare, i32),
    Date(Compare, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compare {
    Equal,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    NotEqual,
}

impl Compare {
    fn holds(self, ordering: std::cmp::Ordering) -> bool {
        use std::cmp::Ordering::{Equal, Greater, Less};
        match self {
            Self::Equal => ordering == Equal,
            Self::Less => ordering == Less,
            Self::LessOrEqual => ordering != Greater,
            Self::Greater => ordering == Greater,
            Self::GreaterOrEqual => ordering != Less,
            Self::NotEqual => ordering != Equal,
        }
    }
}

/// The operators a filter takes, longest first so `>=` isn't read as `>`.
const OPERATORS: [(&str, Compare); 7] = [
    (">=", Compare::GreaterOrEqual),
    ("<=", Compare::LessOrEqual),
    ("!=", Compare::NotEqual),
    (":", Compare::Equal),
    ("=", Compare::Equal),
    (">", Compare::Greater),
    ("<", Compare::Less),
];

impl DeckFilter {
    /// Reads a search; an error says what's wrong with it, for the user.
    pub fn parse(query: &str) -> std::result::Result<Self, String> {
        let mut clauses = Vec::new();
        for token in tokens(query) {
            let (negated, text) = match token.text.strip_prefix('-') {
                Some(rest) if !rest.is_empty() && !token.quoted => (true, rest.to_owned()),
                _ => (false, token.text),
            };
            let term = if token.quoted {
                Term::Word(fold(&text))
            } else {
                term(&text)?
            };
            // A word that folds to nothing, such as `&`, matches everything.
            if matches!(&term, Term::Word(word) if word.is_empty()) {
                continue;
            }
            clauses.push(Clause { negated, term });
        }
        Ok(Self(clauses))
    }

    fn matches(&self, deck: &Deck, folded: &Folded) -> bool {
        self.0
            .iter()
            .all(|clause| clause.term.matches(deck, folded) != clause.negated)
    }
}

impl Term {
    fn matches(&self, deck: &Deck, folded: &Folded) -> bool {
        match self {
            Self::Word(word) => has_words(&folded.name, word) || has_words(&folded.set_name, word),
            Self::Name(words) => has_words(&folded.name, words),
            Self::Set(code) => deck.code.eq_ignore_ascii_case(code),
            Self::Type(words) => has_words(&folded.deck_type, words),
            Self::Year(compare, year) => deck
                .release_date
                .as_deref()
                .and_then(|date| date.get(..4)?.parse::<i32>().ok())
                .is_some_and(|released| compare.holds(released.cmp(year))),
            Self::Date(compare, date) => deck.release_date.as_deref().is_some_and(|released| {
                // A date given in part, such as `2024-06`, is compared with as
                // much of the release date.
                let released = released.get(..date.len()).unwrap_or(released);
                compare.holds(released.cmp(date.as_str()))
            }),
        }
    }
}

/// Whether folded `text` has folded `words` in it, at the start of a word.
fn has_words(text: &str, words: &str) -> bool {
    text.match_indices(words)
        .any(|(at, _)| at == 0 || text.as_bytes()[at - 1] == b' ')
}

struct Token {
    text: String,
    /// It started with a quote: it's words to find, whatever's in it.
    quoted: bool,
}

/// Splits a search at spaces outside quotes, dropping the quotes.
fn tokens(query: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut text = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut started = false;
    for c in query.chars() {
        match c {
            '"' | '\u{201c}' | '\u{201d}' => {
                if !started {
                    quoted = true;
                }
                started = true;
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                if started {
                    tokens.push(Token {
                        text: std::mem::take(&mut text),
                        quoted,
                    });
                }
                started = false;
                quoted = false;
            }
            c => {
                started = true;
                text.push(c);
            }
        }
    }
    if started {
        tokens.push(Token { text, quoted });
    }
    tokens
}

/// One part of a search: `key<operator>value`, or a word.
fn term(text: &str) -> std::result::Result<Term, String> {
    let key_end = text.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(text.len());
    let (key, rest) = text.split_at(key_end);
    let Some((operator, compare)) = OPERATORS.iter().find(|(operator, _)| rest.starts_with(operator)) else {
        return Ok(Term::Word(fold(text)));
    };
    if key.is_empty() {
        return Ok(Term::Word(fold(text)));
    }
    let value = &rest[operator.len()..];
    let key = key.to_ascii_lowercase();
    let equal_only = |term: Term| {
        if *compare == Compare::Equal {
            Ok(term)
        } else {
            Err(format!("{key} takes \u{201c}:\u{201d}, not \u{201c}{operator}\u{201d}"))
        }
    };
    match key.as_str() {
        "set" | "s" | "e" | "edition" => equal_only(Term::Set(value.trim().to_owned())),
        "type" | "t" => equal_only(Term::Type(fold(value))),
        "name" | "n" => equal_only(Term::Name(fold(value))),
        "year" | "y" => value
            .trim()
            .parse()
            .map(|year| Term::Year(*compare, year))
            .map_err(|_| format!("year needs a year, such as year>=2020, not \u{201c}{value}\u{201d}")),
        "date" | "released" => {
            let date = value.trim();
            let shape = date.len() >= 4
                && date.len() <= 10
                && date
                    .chars()
                    .enumerate()
                    .all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
            if shape {
                Ok(Term::Date(*compare, date.to_owned()))
            } else {
                Err(format!(
                    "date needs a date, such as date>=2024-06-01, not \u{201c}{value}\u{201d}"
                ))
            }
        }
        _ => Err(format!(
            "Unknown filter \u{201c}{key}{operator}\u{201d}. Try set:, type:, name:, year: or date:"
        )),
    }
}

// --- A box's picture -----------------------------------------------------------

/// How near to white a pixel is to be taken for the background: its darkest
/// channel at least this.
const BACKGROUND: u8 = 235;
/// Where a pixel on the box's edge starts to show through: its darkest channel
/// at or below this is the box's own colour.
const EDGE: u8 = 180;
/// More of the picture than this taken for background, and it's likely a
/// white box, not a white backdrop: the picture is kept as it was.
const MOST_REMOVED: f64 = 0.6;

/// A product photo with its white backdrop cut away, and cropped to what's
/// left. Only white that reaches the picture's edge is backdrop, so white on
/// the box itself stays; the box's outline fades by how white it is, so it
/// isn't jagged.
pub fn cut_out_box(image: &DynamicImage) -> RgbaImage {
    let mut out = image.to_rgba8();
    let (width, height) = out.dimensions();
    if width == 0 || height == 0 {
        return out;
    }
    let darkest = |pixel: &Rgba<u8>| pixel[0].min(pixel[1]).min(pixel[2]);
    let index = |x: u32, y: u32| (y * width + x) as usize;

    let mut background = vec![false; (width * height) as usize];
    let mut queue: Vec<(u32, u32)> = Vec::new();
    for x in 0..width {
        queue.push((x, 0));
        queue.push((x, height - 1));
    }
    for y in 0..height {
        queue.push((0, y));
        queue.push((width - 1, y));
    }
    let mut seen = vec![false; (width * height) as usize];
    while let Some((x, y)) = queue.pop() {
        let i = index(x, y);
        if seen[i] {
            continue;
        }
        seen[i] = true;
        if darkest(out.get_pixel(x, y)) < BACKGROUND {
            continue;
        }
        background[i] = true;
        if x > 0 {
            queue.push((x - 1, y));
        }
        if x + 1 < width {
            queue.push((x + 1, y));
        }
        if y > 0 {
            queue.push((x, y - 1));
        }
        if y + 1 < height {
            queue.push((x, y + 1));
        }
    }

    let removed = background.iter().filter(|b| **b).count();
    if removed == 0 || removed as f64 / background.len() as f64 > MOST_REMOVED {
        return out;
    }

    for y in 0..height {
        for x in 0..width {
            if background[index(x, y)] {
                out.get_pixel_mut(x, y)[3] = 0;
                continue;
            }
            let touches = (x > 0 && background[index(x - 1, y)])
                || (x + 1 < width && background[index(x + 1, y)])
                || (y > 0 && background[index(x, y - 1)])
                || (y + 1 < height && background[index(x, y + 1)]);
            if touches {
                let pixel = out.get_pixel_mut(x, y);
                let light = darkest(pixel);
                if light > EDGE {
                    pixel[3] = ((255 - u32::from(light)) * 255 / u32::from(255 - EDGE)) as u8;
                }
            }
        }
    }

    // Cropped to the box.
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (x, y, pixel) in out.enumerate_pixels() {
        if pixel[3] > 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }
    if left > right || top > bottom {
        return out;
    }
    imageops::crop_imm(&out, left, top, right - left + 1, bottom - top + 1).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn catalog() -> Catalog {
        let sets: MtgjsonFile<Vec<SetEntry>> = serde_json::from_str(&fixture("mtgjson_set_list.json")).unwrap();
        let listed: MtgjsonFile<Vec<ListedDeck>> = serde_json::from_str(&fixture("mtgjson_deck_list.json")).unwrap();
        Catalog::new(now(), catalog_decks(sets.data, listed.data))
    }

    fn names(catalog: &Catalog, query: &str) -> Vec<String> {
        let filter = DeckFilter::parse(query).unwrap();
        catalog
            .search(&filter, usize::MAX)
            .decks
            .into_iter()
            .map(|deck| deck.name)
            .collect()
    }

    #[test]
    fn lists_each_deck_with_its_set_and_box() {
        let catalog = catalog();
        let abzan = catalog.deck("AbzanArmor_TDC").unwrap();
        assert_eq!(
            abzan.summary(),
            DeckSummary {
                file_name: "AbzanArmor_TDC".into(),
                name: "Abzan Armor".into(),
                code: "TDC".into(),
                set_name: "Tarkir: Dragonstorm Commander".into(),
                deck_type: "Commander Deck".into(),
                release_date: Some("2025-04-11".into()),
                has_box: true,
            }
        );
        assert_eq!(abzan.box_id.as_deref(), Some("619664"));
        // Newest first.
        let first = &catalog.decks[0];
        assert_eq!(first.code, "EOC");
    }

    #[test]
    fn searches_like_scryfall() {
        let catalog = catalog();
        assert_eq!(names(&catalog, "set:eoc"), ["Counter Intelligence", "World Shaper"]);
        assert_eq!(names(&catalog, "SET:EOC -world"), ["Counter Intelligence"]);
        assert_eq!(names(&catalog, "abzan"), ["Abzan Armor"]);
        // A bare word finds the set's name too.
        assert_eq!(names(&catalog, "tarkir").len(), 5);
        assert_eq!(names(&catalog, "\"heavenly inferno\""), ["Heavenly Inferno"]);
        assert_eq!(names(&catalog, "year<2012").len(), 5);
        assert_eq!(names(&catalog, "year>=2025 t:commander").len(), 7);
        assert_eq!(names(&catalog, "date:2025-04"), names(&catalog, "s:tdc"));
        assert!(names(&catalog, "t:\"secret lair\"").is_empty());
        // Words start words: `arm` is in "Abzan Armor", `rmor` isn't.
        assert_eq!(names(&catalog, "arm"), ["Abzan Armor"]);
        assert!(names(&catalog, "rmor").is_empty());
        assert_eq!(catalog.search(&DeckFilter::default(), 3).total, 12);
        assert_eq!(catalog.search(&DeckFilter::default(), 3).decks.len(), 3);
    }

    #[test]
    fn says_what_is_wrong_with_a_search() {
        assert!(DeckFilter::parse("color:g").unwrap_err().contains("color:"));
        assert!(DeckFilter::parse("year>=soon").unwrap_err().contains("year"));
        assert!(DeckFilter::parse("date<june").unwrap_err().contains("date"));
        assert!(DeckFilter::parse("set>tdc").is_err());
        // A colon in quotes is part of the words.
        assert!(DeckFilter::parse("\"foo: bar\"").is_ok());
    }

    #[test]
    fn reads_a_decks_printings_commander_first() {
        let deck: MtgjsonFile<DeckFile> = serde_json::from_str(&fixture("mtgjson_deck_AbzanArmor_TDC.json")).unwrap();
        let cards = deck_lines(deck.data);
        assert_eq!(cards.name, "Abzan Armor");
        let lines: Vec<_> = cards.lines.iter().map(|line| line.line.as_str()).collect();
        assert_eq!(
            lines,
            [
                "Felothar the Steadfast (TDC) 4",
                "Betor, Ancestor's Voice (TDC) 1",
                "Protector of the Wastes (TDC) 14",
                "Reunion of the House (TDC) 15",
            ]
        );
        assert_eq!(cards.lines[0].set.as_deref(), Some("tdc"));
        assert_eq!(cards.lines[0].number.as_deref(), Some("4"));
    }

    #[test]
    fn writes_mtgjsons_device_name_sets_as_scryfall_does() {
        let deck = DeckFile {
            name: "Conflux".into(),
            commander: Vec::new(),
            main_board: vec![
                DeckCard {
                    name: "Path to Exile".into(),
                    set_code: "CON_".into(),
                    number: "15".into(),
                },
                DeckCard {
                    name: "Path to Exile".into(),
                    set_code: "CON_".into(),
                    number: "15".into(),
                },
            ],
            side_board: Vec::new(),
            planes: Vec::new(),
            schemes: Vec::new(),
            tokens: Vec::new(),
        };
        let cards = deck_lines(deck);
        assert_eq!(cards.lines.len(), 1);
        assert_eq!(cards.lines[0].set.as_deref(), Some("con"));
    }

    /// A box on a white backdrop, with white on the box itself.
    fn product_photo() -> DynamicImage {
        let mut photo = RgbaImage::from_pixel(100, 140, Rgba([255, 255, 255, 255]));
        for y in 20..120 {
            for x in 20..80 {
                photo.put_pixel(x, y, Rgba([30, 30, 40, 255]));
            }
        }
        // A white badge inside the box.
        for y in 40..50 {
            for x in 40..50 {
                photo.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        DynamicImage::ImageRgba8(photo)
    }

    #[test]
    fn cuts_the_backdrop_from_a_box_and_keeps_white_on_it() {
        let cut = cut_out_box(&product_photo());
        assert_eq!(cut.dimensions(), (60, 100));
        // The badge is still white and solid.
        assert_eq!(cut.get_pixel(25, 25), &Rgba([255, 255, 255, 255]));
        assert_eq!(cut.get_pixel(0, 0)[3], 255);
    }

    #[test]
    fn keeps_a_white_box_whole() {
        // Mostly white reaching the edges: a white box, not a backdrop.
        let mut photo = RgbaImage::from_pixel(100, 100, Rgba([250, 250, 250, 255]));
        photo.put_pixel(50, 50, Rgba([0, 0, 0, 255]));
        let cut = cut_out_box(&DynamicImage::ImageRgba8(photo));
        assert_eq!(cut.dimensions(), (100, 100));
        assert!(cut.pixels().all(|pixel| pixel[3] == 255));
    }
}
