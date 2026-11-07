//! Reads a pasted or exported card list: one card per line, in the forms deck
//! sites export — `Damnation`, `4 Damnation`, `4x Damnation (TSR) 106 *F*`.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use ts_rs::TS;

/// One card asked for by a line of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardLine {
    /// The line as written, trimmed.
    pub line: String,
    pub name: String,
    /// The set code, lower case, when the line gives one.
    pub set: Option<String>,
    /// The collector number, when the line gives one after the set.
    pub number: Option<String>,
}

impl CardLine {
    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            line: name.clone(),
            name,
            set: None,
            number: None,
        }
    }

    /// `Name (SET) 123`, with whichever of the set and number the line had.
    pub fn label(&self) -> String {
        match (&self.set, &self.number) {
            (Some(set), Some(number)) => format!("{} ({}) {number}", self.name, set.to_uppercase()),
            (Some(set), None) => format!("{} ({})", self.name, set.to_uppercase()),
            _ => self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SkipReason {
    /// A section heading such as `Sideboard` or `Creatures (12)`.
    Heading,
    /// A line starting `#` or `//`.
    Comment,
}

/// A line that isn't a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkippedLine {
    pub line: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParsedList {
    pub cards: Vec<CardLine>,
    pub skipped: Vec<SkippedLine>,
}

/// Section headings deck sites put in their exports. A line that is one of
/// these, with or without a trailing `:` or `(count)`, is skipped.
const HEADINGS: &[&str] = &[
    "about",
    "artifacts",
    "attractions",
    "battles",
    "commander",
    "commanders",
    "companion",
    "companions",
    "considering",
    "creatures",
    "deck",
    "enchantments",
    "instants",
    "lands",
    "main",
    "main deck",
    "mainboard",
    "maybe",
    "maybeboard",
    "other",
    "planeswalkers",
    "side",
    "sideboard",
    "sorceries",
    "spells",
    "stickers",
    "tokens",
];

/// A leading count: `4 `, `4x `, `SB: 2 `. Capped at three digits so names
/// such as `1996 World Champion` keep their number.
static QUANTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:SB:\s*)?\d{1,3}x?\s+(.+)$").unwrap());

/// `Name (SET)` with an optional collector number; anything after it — foil
/// markers, Archidekt categories and tags — is ignored. A set code has no
/// spaces, so `B.F.M. (Big Furry Monster)` stays a name.
static SET_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)\s+\(([A-Za-z0-9]{2,8})\)(?:\s+([^\s\[\^*]+))?(?:\s.*)?$").unwrap());

/// Trailing markers on a line without a set: `*F*`, `*E*`, `^tag^`, `[Category]`.
static MARKERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\s+(?:\*[A-Za-z]+\*|\^[^^]*\^|\[[^\]]*\]))+$").unwrap());

static HEADING_COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\(\d+\)$").unwrap());

pub fn parse_list(text: &str) -> ParsedList {
    let mut parsed = ParsedList::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let skip = |reason| SkippedLine {
            line: line.to_owned(),
            reason,
        };
        if line.starts_with('#') || line.starts_with("//") {
            parsed.skipped.push(skip(SkipReason::Comment));
        } else if is_heading(line) {
            parsed.skipped.push(skip(SkipReason::Heading));
        } else if let Some(card) = parse_line(line) {
            parsed.cards.push(card);
        }
    }
    parsed
}

pub fn parse_line(line: &str) -> Option<CardLine> {
    let line = line.trim();
    let mut rest = line
        .strip_prefix("SB:")
        .or_else(|| line.strip_prefix("sb:"))
        .unwrap_or(line)
        .trim();
    if let Some(captures) = QUANTITY.captures(rest) {
        rest = captures.get(1).map_or(rest, |m| m.as_str());
    }
    let (name, set, number) = match SET_NUMBER.captures(rest) {
        Some(captures) => (
            captures[1].trim().to_owned(),
            Some(captures[2].to_lowercase()),
            captures.get(3).map(|m| m.as_str().to_owned()),
        ),
        None => (MARKERS.replace(rest, "").trim().to_owned(), None, None),
    };
    (!name.is_empty()).then(|| CardLine {
        line: line.to_owned(),
        name,
        set,
        number,
    })
}

fn is_heading(line: &str) -> bool {
    let bare = line.trim_end_matches(':').trim();
    let bare = HEADING_COUNT.replace(bare, "");
    let bare = bare.trim_end_matches(':').trim().to_lowercase();
    HEADINGS.contains(&bare.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, set: Option<&str>, number: Option<&str>) -> (String, Option<String>, Option<String>) {
        (name.to_owned(), set.map(str::to_owned), number.map(str::to_owned))
    }

    fn parse(line: &str) -> (String, Option<String>, Option<String>) {
        let c = parse_line(line).expect("parses");
        (c.name, c.set, c.number)
    }

    #[test]
    fn reads_names() {
        assert_eq!(parse("Damnation"), card("Damnation", None, None));
        assert_eq!(parse("  Fire // Ice  "), card("Fire // Ice", None, None));
        assert_eq!(
            parse("B.F.M. (Big Furry Monster)"),
            card("B.F.M. (Big Furry Monster)", None, None)
        );
        assert_eq!(parse("1996 World Champion"), card("1996 World Champion", None, None));
    }

    #[test]
    fn strips_counts() {
        assert_eq!(parse("4 Damnation"), card("Damnation", None, None));
        assert_eq!(parse("4x Damnation"), card("Damnation", None, None));
        assert_eq!(parse("SB: 2 Damnation"), card("Damnation", None, None));
        assert_eq!(parse("100 Swamp"), card("Swamp", None, None));
    }

    #[test]
    fn reads_sets_and_numbers() {
        assert_eq!(
            parse("Damnation (TSR) 106"),
            card("Damnation", Some("tsr"), Some("106"))
        );
        assert_eq!(parse("1 Damnation (tsr)"), card("Damnation", Some("tsr"), None));
        assert_eq!(
            parse("Bala Ged Recovery // Bala Ged Sanctuary (znr) 180"),
            card("Bala Ged Recovery // Bala Ged Sanctuary", Some("znr"), Some("180"))
        );
        assert_eq!(
            parse("Sol Ring (PLST) C21-263"),
            card("Sol Ring", Some("plst"), Some("C21-263"))
        );
        assert_eq!(parse("Plains (UNF) 235★"), card("Plains", Some("unf"), Some("235★")));
    }

    #[test]
    fn ignores_markers() {
        assert_eq!(
            parse("1 Damnation (TSR) 106 *F*"),
            card("Damnation", Some("tsr"), Some("106"))
        );
        assert_eq!(parse("1 Damnation (TSR) *F*"), card("Damnation", Some("tsr"), None));
        assert_eq!(
            parse("1x Damnation (tsr) 106 [Removal] ^Have,#37d67a^"),
            card("Damnation", Some("tsr"), Some("106"))
        );
        assert_eq!(parse("1 Damnation *E*"), card("Damnation", None, None));
        assert_eq!(parse("1x Damnation [Removal]"), card("Damnation", None, None));
    }

    #[test]
    fn skips_headings_comments_and_blank_lines() {
        let parsed = parse_list("Commander\nAtraxa, Grand Unifier\n\n// tokens below\nSideboard (15):\nCreatures (2)\n2 Birds of Paradise\n");
        let names: Vec<_> = parsed.cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Atraxa, Grand Unifier", "Birds of Paradise"]);
        let skipped: Vec<_> = parsed.skipped.iter().map(|s| (s.line.as_str(), s.reason)).collect();
        assert_eq!(
            skipped,
            [
                ("Commander", SkipReason::Heading),
                ("// tokens below", SkipReason::Comment),
                ("Sideboard (15):", SkipReason::Heading),
                ("Creatures (2)", SkipReason::Heading),
            ]
        );
    }

    #[test]
    fn labels() {
        assert_eq!(
            parse_line("4 Damnation (tsr) 106 *F*").unwrap().label(),
            "Damnation (TSR) 106"
        );
        assert_eq!(parse_line("4 Damnation").unwrap().label(), "Damnation");
    }
}
