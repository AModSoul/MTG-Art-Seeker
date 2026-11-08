//! Finding a card's art on MTGPics, which has no API: its pages are read.
//!
//! A card is first looked for at the ref its set code and collector number give
//! (`dom` + `001`), as Card Anvil does — one page per card. That page has to
//! name the card and credit its artist, since MTGPics numbers some sets its own
//! way, and a set can print one name many times. When it doesn't, the set's checklist is read to find the card's real ref, as the
//! original downloader did; each checklist is read once per run, however many
//! of its cards are asked for.

mod artists;
mod checklist;
mod codes;
mod page;

use std::{collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, OnceCell};

pub use artists::{artist_website, parse_artist_list, website_on_page, Artists};
pub use checklist::{parse_checklist, set_id_on_card_page, Checklist, Row};
pub use codes::{mtgpics_set_code, pad_collector_number};
pub use page::{extract_arts, is_missing_page, page_artists, page_title, pick_arts, MtgpicsArt};

use crate::{
    card::Card,
    http::Http,
    text::{fold, same_artist},
    Result,
};

/// Set codes MTGPics files promos under, whose cards are found in a promo list
/// by name and artist rather than by number.
const PROMO_SETS: [&str; 7] = ["pre", "pmo", "dci", "fnm", "pst", "30a", "slc"];

/// MTGPics' promo checklists, by set id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromoList {
    Promos = 72,
    JudgeGifts = 18,
    Alchemy = 375,
    Championships = 201,
}

enum Lookup {
    Set(String),
    Promo(PromoList),
}

/// A checklist once it has been read: `None` when there was none to read.
type ChecklistCell = Arc<OnceCell<Option<Arc<Checklist>>>>;

/// Checklists already read this run, so each is fetched at most once.
#[derive(Default)]
pub struct Cache {
    checklists: Mutex<HashMap<String, ChecklistCell>>,
}

/// The art MTGPics has for each of a card's `faces`; `None` where it has none.
pub async fn find_arts(http: &Http, cache: &Cache, card: &Card, faces: usize) -> Result<Vec<Option<MtgpicsArt>>> {
    let arts = match lookup(card) {
        Lookup::Set(set) => {
            let direct = format!("{set}{}", pad_collector_number(&card.collector_number));
            match arts_on_page(http, &direct, card).await? {
                Some(arts) => arts,
                None => match find_in_set(http, cache, &set, card).await? {
                    Some(card_ref) if card_ref != direct => {
                        arts_on_page(http, &card_ref, card).await?.unwrap_or_default()
                    }
                    // Promo and Un-sets MTGPics has no set for are in its promo list.
                    _ if matches!(card.set_type.as_str(), "promo" | "funny") => {
                        promo_arts(http, cache, PromoList::Promos, card).await?
                    }
                    _ => Vec::new(),
                },
            }
        }
        Lookup::Promo(list) => promo_arts(http, cache, list, card).await?,
    };
    Ok(pick_arts(&arts, faces))
}

fn lookup(card: &Card) -> Lookup {
    let set = mtgpics_set_code(&card.set);
    if set == "dci" || card.set_name.contains("Judge Gift") {
        return Lookup::Promo(PromoList::JudgeGifts);
    }
    if PROMO_SETS.contains(&set.as_str()) {
        return Lookup::Promo(PromoList::Promos);
    }
    if card.set_name.contains("Alchemy") {
        return Lookup::Promo(PromoList::Alchemy);
    }
    if matches!(card.set_name.as_str(), "Legacy Championship" | "Vintage Championship") {
        return Lookup::Promo(PromoList::Championships);
    }
    Lookup::Set(set)
}

/// The art on a card page, or `None` if there's no such page or it's another
/// card's: another name, or another artist's printing of the name.
async fn arts_on_page(http: &Http, card_ref: &str, card: &Card) -> Result<Option<Vec<MtgpicsArt>>> {
    let origin = &http.endpoints().mtgpics;
    let Some(html) = http.get_text(&format!("{origin}/card?ref={card_ref}")).await? else {
        return Ok(None);
    };
    if is_missing_page(&html) {
        return Ok(None);
    }
    if let Some(title) = page_title(&html) {
        if !fold(&title).contains(&fold(card.front_name())) {
            return Ok(None);
        }
    }
    if let Some(artist) = card_artist(card) {
        let credited = page_artists(&html);
        if !credited.is_empty() && !credited.iter().any(|credit| same_artist(credit, artist)) {
            return Ok(None);
        }
    }
    Ok(Some(extract_arts(&html, origin)))
}

/// The card's ref according to its set's checklist.
async fn find_in_set(http: &Http, cache: &Cache, set: &str, card: &Card) -> Result<Option<String>> {
    let checklist = cache
        .get(&format!("set:{set}"), || async {
            let origin = &http.endpoints().mtgpics;
            // The set's id is only linked from its cards' pages.
            let Some(first) = http.get_text(&format!("{origin}/card?ref={set}001")).await? else {
                return Ok(None);
            };
            match set_id_on_card_page(&first) {
                Some(id) => fetch_checklist(http, &id).await,
                None => Ok(None),
            }
        })
        .await?;
    Ok(checklist.and_then(|list| {
        list.find(&card.collector_number, card.front_name(), card_artist(card))
            .map(|row| row.card_ref.clone())
    }))
}

fn card_artist(card: &Card) -> Option<&str> {
    card.artist
        .as_deref()
        .or_else(|| card.card_faces.first()?.artist.as_deref())
}

async fn promo_arts(http: &Http, cache: &Cache, list: PromoList, card: &Card) -> Result<Vec<MtgpicsArt>> {
    let id = (list as u32).to_string();
    let checklist = cache.get(&format!("id:{id}"), || fetch_checklist(http, &id)).await?;
    let artist = card_artist(card).unwrap_or_default();
    let card_ref = checklist.and_then(|list| {
        list.find_promo(card.front_name(), artist, &card.set_name)
            .map(|row| row.card_ref.clone())
    });
    match card_ref {
        Some(card_ref) => Ok(arts_on_page(http, &card_ref, card).await?.unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

async fn fetch_checklist(http: &Http, set_id: &str) -> Result<Option<Checklist>> {
    let url = format!("{}/set_checklist?set={set_id}", http.endpoints().mtgpics);
    Ok(http.get_text(&url).await?.map(|html| parse_checklist(&html)))
}

impl Cache {
    /// The checklist under `key`, loading it if no one has. Callers that arrive
    /// while it loads wait for that load rather than starting their own. A
    /// failed load isn't kept, so the next caller tries again.
    async fn get<F, Fut>(&self, key: &str, load: F) -> Result<Option<Arc<Checklist>>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Option<Checklist>>>,
    {
        let cell = self.checklists.lock().await.entry(key.to_owned()).or_default().clone();
        cell.get_or_try_init(|| async { Ok(load().await?.map(Arc::new)) })
            .await
            .cloned()
    }
}
