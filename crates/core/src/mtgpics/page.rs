//! Reading an MTGPics card page (`/card?ref=dom001`).

use std::sync::LazyLock;

use regex::Regex;

use crate::text::decode_entities;

/// One full-size artwork on MTGPics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtgpicsArt {
    pub url: String,
    /// MTGPics' set code, e.g. `dom`.
    pub set: String,
    /// The image's number on MTGPics: the collector number, plus any variant
    /// suffix (`001_2`), or another number entirely for a back face (`549`).
    pub number: String,
}

/// The page shows its art as `art_th` thumbnails; the full-size image is at the
/// same path under `art`. `big` and `reg` are whole-card scans, not art, so
/// they don't match. From Card Anvil's `extractArtUrls`.
static ART_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)pics/(?:art|art_th)/([a-z0-9]+)/([a-z0-9]+(?:_\d+)?)\.(jpg|png)").unwrap());

/// A whole-card scan. A page shows its card's faces' scans front first, which
/// its art thumbnails don't always follow.
static CARD_SCAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)pics/(?:big|reg)/([a-z0-9]+)/([a-z0-9]+)\.(?:jpg|png)").unwrap());

static TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<title>\s*(.*?)\s*-\s*mtgpics\.com\s*</title>").unwrap());

/// `Illus. <a href=illus?art=724>Chase Stone</a>`, or for a card with two faces
/// `Illus. (Agadeem's Awakening) <a …>Dmitry Burmak</a>`, once per face.
static ILLUSTRATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)Illus\.\s*(?:\([^)]*\)\s*)?<a[^>]*>([^<]+)</a>").unwrap());

/// The artists a card page credits.
pub fn page_artists(html: &str) -> Vec<String> {
    ILLUSTRATOR
        .captures_iter(html)
        .map(|c| decode_entities(c[1].trim()))
        .collect()
}

/// Every distinct artwork on a card page, front face first.
///
/// Thumbnails mostly come in face order, but not always: `stx/006` shows its
/// back's art (`715`) before its front's. The page's card scans do come front
/// first, so art that has a scan of the same number is put in their order.
/// Art without one keeps its place after them.
pub fn extract_arts(html: &str, origin: &str) -> Vec<MtgpicsArt> {
    let mut arts: Vec<MtgpicsArt> = Vec::new();
    for captures in ART_PATH.captures_iter(html) {
        let (set, number, ext) = (&captures[1], &captures[2], &captures[3]);
        let url = format!("{origin}/pics/art/{set}/{number}.{ext}");
        if !arts.iter().any(|art| art.url == url) {
            arts.push(MtgpicsArt {
                url,
                set: set.to_owned(),
                number: number.to_owned(),
            });
        }
    }
    let scans: Vec<(String, String)> = CARD_SCAN
        .captures_iter(html)
        .map(|c| (c[1].to_lowercase(), c[2].to_lowercase()))
        .collect();
    // A stable sort: variants of one image, and art with no scan, keep their order.
    arts.sort_by_key(|art| {
        let key = (art.set.to_lowercase(), base_number(&art.number).to_lowercase());
        scans.iter().position(|scan| *scan == key).unwrap_or(usize::MAX)
    });
    arts
}

/// The card's name as the page's title gives it: `Karn, Scion of Urza`, or
/// `Front/Back` for a card with two faces.
pub fn page_title(html: &str) -> Option<String> {
    TITLE.captures(html).map(|c| decode_entities(&c[1]))
}

/// Whether MTGPics answered "no such card" with a page instead of a 404.
pub fn is_missing_page(html: &str) -> bool {
    html.contains("No card found.") || html.contains("Wrong ref or number.")
}

/// Which art goes with which face: one entry per face, `None` where MTGPics has
/// none for it.
///
/// Variants of one image (`001_1`, `001_2`) count as one artwork, and its first
/// variant is used. A double-faced card's back is the second artwork — MTGPics
/// may number it anything (`znr/090` has its back at `znr/549`). With only one
/// artwork, the back has none, and falls back to Scryfall.
pub fn pick_arts(arts: &[MtgpicsArt], faces: usize) -> Vec<Option<MtgpicsArt>> {
    let mut groups: Vec<(&str, &MtgpicsArt)> = Vec::new();
    for art in arts {
        let base = base_number(&art.number);
        if !groups.iter().any(|(seen, _)| *seen == base) {
            groups.push((base, art));
        }
    }
    (0..faces)
        .map(|face| groups.get(face).map(|(_, art)| (*art).clone()))
        .collect()
}

/// `001_2` → `001`.
fn base_number(number: &str) -> &str {
    match number.rsplit_once('_') {
        Some((base, variant)) if !variant.is_empty() && variant.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => number,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "https://www.mtgpics.com";

    /// MTGPics serves ISO-8859-1, where each byte is the char of that code point.
    pub(crate) fn latin1(bytes: &[u8]) -> String {
        bytes.iter().map(|&b| char::from(b)).collect()
    }

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        latin1(&std::fs::read(path).unwrap())
    }

    fn numbers(arts: &[MtgpicsArt]) -> Vec<&str> {
        arts.iter().map(|a| a.number.as_str()).collect()
    }

    #[test]
    fn reads_a_single_faced_card_with_variants() {
        let html = fixture("card_dom001.html");
        assert_eq!(page_title(&html).as_deref(), Some("Karn, Scion of Urza"));
        assert_eq!(page_artists(&html), ["Chase Stone"]);
        let arts = extract_arts(&html, ORIGIN);
        assert_eq!(numbers(&arts), ["001_1", "001_2"]);
        assert_eq!(arts[0].url, "https://www.mtgpics.com/pics/art/dom/001_1.jpg");
        let picked = pick_arts(&arts, 1);
        assert_eq!(picked[0].as_ref().map(|a| a.number.as_str()), Some("001_1"));
    }

    #[test]
    fn reads_a_double_faced_card() {
        let html = fixture("card_znr090.html");
        assert_eq!(
            page_title(&html).as_deref(),
            Some("Agadeem's Awakening/Agadeem, the Undercrypt")
        );
        assert_eq!(page_artists(&html), ["Dmitry Burmak", "Dmitry Burmak"]);
        let arts = extract_arts(&html, ORIGIN);
        assert_eq!(numbers(&arts), ["090", "549"]);
        let picked: Vec<_> = pick_arts(&arts, 2).into_iter().map(|a| a.map(|a| a.number)).collect();
        assert_eq!(picked, [Some("090".to_owned()), Some("549".to_owned())]);
    }

    #[test]
    fn puts_art_in_face_order_when_the_thumbnails_are_not() {
        // The page's thumbnails show the back's art first; its scans don't.
        let html = fixture("card_stx006.html");
        assert_eq!(
            page_title(&html).as_deref(),
            Some("Wandering Archaic/Explore the Vastlands")
        );
        let arts = extract_arts(&html, ORIGIN);
        assert_eq!(numbers(&arts), ["006", "715"]);
        let picked: Vec<_> = pick_arts(&arts, 2).into_iter().map(|a| a.map(|a| a.number)).collect();
        assert_eq!(picked, [Some("006".to_owned()), Some("715".to_owned())]);
    }

    #[test]
    fn a_back_face_without_art_is_none() {
        let arts = extract_arts(&fixture("card_khm252.html"), ORIGIN);
        assert_eq!(numbers(&arts), ["252"]);
        let picked = pick_arts(&arts, 2);
        assert!(picked[0].is_some() && picked[1].is_none());
    }

    #[test]
    fn variants_of_the_front_are_not_the_back() {
        let art = |n: &str| MtgpicsArt {
            url: format!("{ORIGIN}/pics/art/x/{n}.jpg"),
            set: "x".into(),
            number: n.into(),
        };
        let arts = [art("030_1"), art("030_2"), art("031")];
        let picked: Vec<_> = pick_arts(&arts, 2).into_iter().map(|a| a.unwrap().number).collect();
        assert_eq!(picked, ["030_1", "031"]);
        // Suffixed back faces, as the original downloader's tests had them.
        let picked: Vec<_> = pick_arts(&[art("030"), art("030b")], 2)
            .into_iter()
            .map(|a| a.unwrap().number)
            .collect();
        assert_eq!(picked, ["030", "030b"]);
    }

    #[test]
    fn recognises_a_missing_card() {
        assert!(is_missing_page(&fixture("card_zzz999.html")));
        assert!(!is_missing_page(&fixture("card_dom001.html")));
    }
}
