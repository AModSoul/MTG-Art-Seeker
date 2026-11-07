//! Reading an MTGPics set checklist (`/set_checklist?set=288`): every card in a
//! set, with the ref its page is under. Used when a card isn't at the ref its
//! set code and collector number give.

use std::sync::LazyLock;

use regex::Regex;

use crate::text::{fold, same_artist, strip_tags};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The collector number as MTGPics shows it, unpadded: `1`.
    pub number: String,
    /// The card page's ref: `dom001`.
    pub card_ref: String,
    pub name: String,
    pub artist: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checklist {
    pub rows: Vec<Row>,
}

/// Each card is a `<div id=card_N>` holding one table row of seven cells:
/// number, rarity, name (linked to its page), type, mana value, cost, artist.
static ROW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)<div\s+id=["']?card_\d+.*?</table>"#).unwrap());
static CELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<td[^>]*>(.*?)</td>").unwrap());
static CARD_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)href=["']?card\?ref=([a-z0-9_]+)["']?[^>]*>"#).unwrap());
/// The link from a card page to its set: `href=set?set=288`.
static SET_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)href=["']?set\?set=(\d+)"#).unwrap());

/// The id of the set a card page belongs to.
pub fn set_id_on_card_page(html: &str) -> Option<String> {
    SET_LINK.captures(html).map(|c| c[1].to_owned())
}

pub fn parse_checklist(html: &str) -> Checklist {
    let rows = ROW
        .find_iter(html)
        .filter_map(|row| {
            let cells: Vec<&str> = CELL
                .captures_iter(row.as_str())
                .map(|c| c.get(1).unwrap().as_str())
                .collect();
            let name_cell = cells.get(2)?;
            Some(Row {
                number: strip_tags(cells.first()?),
                card_ref: CARD_LINK.captures(name_cell)?[1].to_lowercase(),
                name: strip_tags(name_cell),
                artist: cells.get(6).map(|c| strip_tags(c)).unwrap_or_default(),
            })
        })
        .collect();
    Checklist { rows }
}

impl Checklist {
    /// The row for a card: the one with its collector number and name, or
    /// failing that, the first with its name and artist. A set can print a
    /// name many times — Secret Lair has dozens of Sol Rings — so a name alone
    /// is never enough.
    pub fn find(&self, number: &str, name: &str, artist: Option<&str>) -> Option<&Row> {
        let name = fold(name);
        let number = number.trim_start_matches('0');
        let named = |row: &&Row| fold(&row.name).contains(&name);
        self.rows
            .iter()
            .filter(named)
            .find(|row| row.number.trim_start_matches('0').eq_ignore_ascii_case(number))
            .or_else(|| {
                let artist = artist?;
                self.rows
                    .iter()
                    .filter(named)
                    .find(|row| same_artist(&row.artist, artist))
            })
    }

    /// The row for a promo: among those with the card's name and artist, the one
    /// whose name, the card's own name aside, reads most like the set's name —
    /// `Sol Ring (Judge Gift)` for a card from `Judge Gift Cards 2004`.
    pub fn find_promo(&self, name: &str, artist: &str, set_name: &str) -> Option<&Row> {
        let name = fold(name);
        let set_name = fold(set_name);
        self.rows
            .iter()
            .filter(|row| same_artist(&row.artist, artist))
            .filter_map(|row| {
                let row_name = fold(&row.name);
                let rest = row_name.contains(&name).then(|| row_name.replace(&name, ""))?;
                Some((strsim::sorensen_dice(rest.trim(), &set_name), row))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, row)| row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).unwrap().iter().map(|&b| char::from(b)).collect()
    }

    #[test]
    fn finds_the_set_on_a_card_page() {
        assert_eq!(
            set_id_on_card_page(&fixture("card_dom001.html")).as_deref(),
            Some("288")
        );
    }

    #[test]
    fn reads_a_checklist() {
        let checklist = parse_checklist(&fixture("checklist_288.html"));
        assert!(checklist.rows.len() > 250, "{} rows", checklist.rows.len());
        assert_eq!(
            checklist.rows[0],
            Row {
                number: "1".into(),
                card_ref: "dom001".into(),
                name: "Karn, Scion of Urza".into(),
                artist: "Chase Stone".into(),
            }
        );
    }

    #[test]
    fn finds_cards_by_number_and_name() {
        let checklist = parse_checklist(&fixture("checklist_288.html"));
        let find = |number, name, artist| checklist.find(number, name, artist).map(|r| r.card_ref.as_str());
        assert_eq!(find("1", "Karn, Scion of Urza", None), Some("dom001"));
        assert_eq!(find("2", "Adamant Will", Some("Alex Konstad")), Some("dom002"));
        // A number that doesn't match falls back to the name and artist…
        assert_eq!(find("999", "Adamant Will", Some("Alex Konstad")), Some("dom002"));
        // …never the name alone.
        assert_eq!(find("999", "Adamant Will", Some("Someone Else")), None);
        assert_eq!(find("999", "Adamant Will", None), None);
        assert_eq!(find("1", "Nonexistent Card", None), None);
    }

    #[test]
    fn finds_promos_by_artist_and_set_name() {
        let row = |card_ref: &str, name: &str, artist: &str| {
            format!(
                "<div id=card_1><table><tr><td>1</td><td></td><td><a href=card?ref={card_ref}>{name}</a></td>\
                 <td></td><td></td><td></td><td><a href=x>{artist}</a></td></tr></table></div>"
            )
        };
        let html = [
            row("pmo001", "Sol Ring (FNM)", "Mark Tedin"),
            row("pmo002", "Sol Ring (Judge Gift)", "Mark Tedin"),
            row("pmo003", "Sol Ring (Judge Gift)", "Someone Else"),
        ]
        .concat();
        let checklist = parse_checklist(&html);
        assert_eq!(checklist.rows.len(), 3);
        let found = checklist.find_promo("Sol Ring", "Mark Tedin", "Judge Gift Cards 2004");
        assert_eq!(found.map(|r| r.card_ref.as_str()), Some("pmo002"));
        assert_eq!(
            checklist.find_promo("Sol Ring", "Nobody", "Judge Gift Cards 2004"),
            None
        );
    }
}
