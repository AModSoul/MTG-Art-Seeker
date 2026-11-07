//! Scryfall lookups. Every call goes through [`Http`], which keeps them within
//! Scryfall's rate limits.

use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::{
    card::{Card, CardList},
    http::Http,
    Error, Result,
};

/// The most identifiers `/cards/collection` takes in one request.
pub const COLLECTION_CHUNK: usize = 75;

/// One card to look up in `/cards/collection`.
/// See <https://scryfall.com/docs/api/cards/collection>.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Identifier {
    SetNumber { set: String, collector_number: String },
    NameSet { name: String, set: String },
    Name { name: String },
}

impl Identifier {
    /// Whether `card` is what this identifier asked for, as far as can be told.
    fn matches(&self, card: &Card) -> bool {
        match self {
            Self::SetNumber { set, collector_number } => {
                card.set.eq_ignore_ascii_case(set) && card.collector_number.eq_ignore_ascii_case(collector_number)
            }
            Self::NameSet { set, .. } => card.set.eq_ignore_ascii_case(set),
            Self::Name { .. } => true,
        }
    }
}

#[derive(Deserialize)]
struct CollectionResponse {
    #[serde(default)]
    data: Vec<Card>,
    #[serde(default)]
    not_found: Vec<serde_json::Value>,
}

/// Looks cards up in batches of [`COLLECTION_CHUNK`]: one entry per identifier,
/// `None` where Scryfall had no match. A batch that fails is `None` throughout,
/// so its cards fall back to a lookup of their own instead of failing the run.
pub async fn collection(http: &Http, identifiers: &[Identifier]) -> Vec<Option<Card>> {
    let url = format!("{}/cards/collection", http.endpoints().scryfall_api);
    let mut found = Vec::with_capacity(identifiers.len());
    for chunk in identifiers.chunks(COLLECTION_CHUNK) {
        match collection_chunk(http, &url, chunk).await {
            Ok(cards) => found.extend(cards),
            Err(_) => found.extend(std::iter::repeat_with(|| None).take(chunk.len())),
        }
    }
    found
}

async fn collection_chunk(http: &Http, url: &str, chunk: &[Identifier]) -> Result<Vec<Option<Card>>> {
    let body = serde_json::json!({ "identifiers": chunk });
    let response = http.post_json(url, &body).await?;
    if !response.status().is_success() {
        return Err(Error::Status {
            status: response.status().as_u16(),
            url: url.to_owned(),
            detail: None,
        });
    }
    let CollectionResponse { data, not_found } = response.json().await?;
    // `data` holds the found cards in the order they were asked for, and
    // `not_found` echoes back the identifiers that weren't.
    let mut data = data.into_iter();
    Ok(chunk
        .iter()
        .map(|identifier| {
            let asked = serde_json::to_value(identifier).ok()?;
            if not_found.contains(&asked) {
                return None;
            }
            // A card that doesn't match what was asked for means the two lists
            // have come apart; looking it up again alone beats the wrong art.
            data.next().filter(|card| identifier.matches(card))
        })
        .collect())
}

/// Scryfall's best guess at a name, optionally within one set; `None` when it
/// can't find one, or the name is ambiguous.
pub async fn named_fuzzy(http: &Http, name: &str, set: Option<&str>) -> Result<Option<Card>> {
    let mut url = endpoint(http, "/cards/named")?;
    url.query_pairs_mut().append_pair("fuzzy", name);
    if let Some(set) = set {
        url.query_pairs_mut().append_pair("set", set);
    }
    http.get_json(url.as_str()).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unique {
    /// One printing per distinct artwork.
    Art,
    /// Every printing.
    Prints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Newest,
    Oldest,
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub unique: Unique,
    pub include_extras: bool,
    /// Sort by release date in this direction; Scryfall's default order otherwise.
    pub by_release: Option<Direction>,
}

pub struct SearchResults {
    pub cards: Vec<Card>,
    /// How many cards matched in all, including any past `max_cards`.
    pub total: u32,
}

/// Runs a Scryfall search and follows its pages until `max_cards` are in hand.
/// A query that matches nothing is an empty result, not an error.
/// See <https://scryfall.com/docs/api/cards/search>.
pub async fn search(http: &Http, query: &str, options: &SearchOptions, max_cards: usize) -> Result<SearchResults> {
    let mut next = Some(search_url(http, query, options)?.to_string());
    let mut cards = Vec::new();
    let mut total = 0;
    while let Some(url) = next.take() {
        let Some(page) = http.get_json::<CardList>(&url).await? else {
            break;
        };
        if total == 0 {
            total = page.total_cards.unwrap_or(0);
        }
        cards.extend(page.data);
        if page.has_more && cards.len() < max_cards {
            next = page.next_page;
        }
    }
    cards.truncate(max_cards);
    let total = total.max(cards.len() as u32);
    Ok(SearchResults { cards, total })
}

/// How many cards a search matches, from its first page alone.
pub async fn count(http: &Http, query: &str, options: &SearchOptions) -> Result<u32> {
    let url = search_url(http, query, options)?;
    Ok(http
        .get_json::<CardList>(url.as_str())
        .await?
        .map_or(0, |page| page.total_cards.unwrap_or(page.data.len() as u32)))
}

/// A query for cards named exactly `name`.
pub fn exact_name_query(name: &str) -> String {
    format!("!\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

fn search_url(http: &Http, query: &str, options: &SearchOptions) -> Result<Url> {
    let mut url = endpoint(http, "/cards/search")?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("q", query);
        pairs.append_pair(
            "unique",
            match options.unique {
                Unique::Art => "art",
                Unique::Prints => "prints",
            },
        );
        pairs.append_pair("include_extras", if options.include_extras { "true" } else { "false" });
        if let Some(direction) = options.by_release {
            pairs.append_pair("order", "released");
            pairs.append_pair(
                "dir",
                match direction {
                    Direction::Newest => "desc",
                    Direction::Oldest => "asc",
                },
            );
        }
    }
    Ok(url)
}

fn endpoint(http: &Http, path: &str) -> Result<Url> {
    let base = &http.endpoints().scryfall_api;
    Url::parse(&format!("{base}{path}")).map_err(|e| Error::Invalid {
        url: base.clone(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_serialize_as_scryfall_expects() {
        let ids = [
            Identifier::SetNumber {
                set: "dom".into(),
                collector_number: "1".into(),
            },
            Identifier::NameSet {
                name: "Karn".into(),
                set: "dom".into(),
            },
            Identifier::Name { name: "Karn".into() },
        ];
        assert_eq!(
            serde_json::to_string(&ids).unwrap(),
            r#"[{"set":"dom","collector_number":"1"},{"name":"Karn","set":"dom"},{"name":"Karn"}]"#
        );
    }

    #[test]
    fn quotes_exact_names() {
        assert_eq!(exact_name_query("Fire // Ice"), r#"!"Fire // Ice""#);
        assert_eq!(
            exact_name_query(r#"Kongming, "Sleeping Dragon""#),
            r#"!"Kongming, \"Sleeping Dragon\"""#
        );
    }
}
