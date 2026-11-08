//! MTGPics' artist pages (`/illus?art=724`), which link some artists' own
//! websites under their name.
//!
//! An artist's page is found through MTGPics' list of artists
//! (`/illustrators`), read once and kept: a card that needs this got
//! Scryfall's art, so MTGPics had no page for it to link the artist from.

use std::{collections::HashMap, sync::LazyLock};

use regex::Regex;
use tokio::sync::OnceCell;

use crate::{
    http::Http,
    text::{decode_entities, fold, same_artist},
    Result,
};

/// `<a href=illus?art=1454>A. M. Sartor</a>` in the list of artists.
static LIST_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)<a href=['"]?illus\?art=(\d+)['"]?>([^<]+)</a>"#).unwrap());

/// The cell with the artist's name at the top of their page, which holds the
/// link to their website when MTGPics has one.
static NAME_CELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)class=titleB16>(.*?)</td>").unwrap());

static HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)href=['"]?([^'" >]+)"#).unwrap());

/// MTGPics' artists, by name folded with [`fold`], to the id of their page.
type ArtistIds = HashMap<String, String>;

/// MTGPics' list of artists, read the first time it's needed and kept.
#[derive(Default)]
pub struct Artists {
    ids: OnceCell<ArtistIds>,
}

/// The website MTGPics links for `artist`: `None` when it has no page for
/// them, or their page links none. Fails, and is asked again next time, when
/// the list of artists can't be had: a list with no one on it isn't MTGPics'.
pub async fn artist_website(http: &Http, artists: &Artists, artist: &str) -> Result<Option<String>> {
    let origin = &http.endpoints().mtgpics;
    let ids = artists
        .ids
        .get_or_try_init(|| async {
            let url = format!("{origin}/illustrators");
            let html = http.get_text(&url).await?;
            let ids = parse_artist_list(html.as_deref().unwrap_or_default());
            if ids.is_empty() {
                return Err(crate::Error::Invalid {
                    url,
                    reason: "MTGPics' list of artists is empty; try again later".into(),
                });
            }
            Ok(ids)
        })
        .await?;
    let Some(id) = find_artist(ids, artist) else {
        return Ok(None);
    };
    let Some(html) = http.get_text(&format!("{origin}/illus?art={id}")).await? else {
        return Ok(None);
    };
    Ok(website_on_page(&html))
}

/// MTGPics' list of artists, by folded name.
pub fn parse_artist_list(html: &str) -> HashMap<String, String> {
    LIST_ENTRY
        .captures_iter(html)
        .map(|c| (fold(&repair_cp1252(&decode_entities(c[2].trim()))), c[1].to_owned()))
        .collect()
}

/// The website an artist's page links under their name, if it's a web address.
pub fn website_on_page(html: &str) -> Option<String> {
    let cell = NAME_CELL.captures(html)?;
    HREF.captures_iter(&cell[1])
        .map(|c| decode_entities(&c[1]))
        .find(|href| href.starts_with("http://") || href.starts_with("https://"))
}

/// The id of `artist`'s page: the artist of that name, or else the one artist
/// whose name is like it. A co-credit (`A & B`) is looked for by each name.
fn find_artist<'a>(ids: &'a ArtistIds, artist: &str) -> Option<&'a str> {
    let names = std::iter::once(artist).chain(artist.split(['&', ',']).map(str::trim));
    for name in names.filter(|name| !name.is_empty()) {
        if let Some(id) = ids.get(&fold(name)) {
            return Some(id);
        }
        let mut like = ids.iter().filter(|(listed, _)| same_artist(listed, name));
        if let (Some((_, id)), None) = (like.next(), like.next()) {
            return Some(id);
        }
    }
    None
}

/// MTGPics declares ISO-8859-1 but writes Windows-1252, where 0x80–0x9F are
/// letters (`š` is 0x9A) rather than control characters; puts them back.
fn repair_cp1252(text: &str) -> String {
    const LETTERS: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘',
        '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
    ];
    text.chars()
        .map(|c| match u32::from(c) {
            code @ 0x80..=0x9F => LETTERS[(code - 0x80) as usize],
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).unwrap().iter().map(|&b| char::from(b)).collect()
    }

    #[test]
    fn reads_the_list_of_artists() {
        let ids = parse_artist_list(&fixture("illustrators.html"));
        assert!(ids.len() > 2400, "{}", ids.len());
        assert_eq!(ids.get("seb mckinnon").map(String::as_str), Some("729"));
        assert_eq!(ids.get("dmitry burmak").map(String::as_str), Some("841"));
        // Named with entities (Eglė Mosakaitė), and with a Windows-1252 letter
        // (the š of Paulius Daščioras is the byte 0x9A).
        assert_eq!(ids.get("egle mosakaite").map(String::as_str), Some("2221"));
        assert_eq!(ids.get("paulius dascioras").map(String::as_str), Some("1970"));
    }

    #[test]
    fn finds_an_artist_by_name_or_like_it() {
        let ids = parse_artist_list(&fixture("illustrators.html"));
        assert_eq!(find_artist(&ids, "Seb McKinnon"), Some("729"));
        assert_eq!(find_artist(&ids, "seb mckinnon"), Some("729"));
        assert_eq!(find_artist(&ids, "Dmitry Burmak & Seb McKinnon"), Some("841"));
        assert_eq!(find_artist(&ids, "Nobody Of That Name"), None);
    }

    #[test]
    fn reads_the_website_under_an_artists_name() {
        assert_eq!(
            website_on_page(&fixture("illus_729.html")).as_deref(),
            Some("http://www.sebmckinnon.com")
        );
        // Dmitry Burmak's page links none; the banner's link isn't his.
        assert_eq!(website_on_page(&fixture("illus_841.html")), None);
    }
}
