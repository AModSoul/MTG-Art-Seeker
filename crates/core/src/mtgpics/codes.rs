use std::{collections::HashMap, sync::LazyLock};

/// Scryfall set codes that MTGPics files under another code. A code not listed
/// is the same on both sites. Taken from Card Anvil, which took it from the
/// original MTG Art Downloader.
static SCRYFALL_TO_MTGPICS: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(include_str!("codes.json")).expect("codes.json is valid JSON"));

/// The MTGPics set code for a Scryfall set code.
pub fn mtgpics_set_code(scryfall_set: &str) -> String {
    let lower = scryfall_set.to_lowercase();
    SCRYFALL_TO_MTGPICS.get(&lower).cloned().unwrap_or(lower)
}

/// Pads a collector number's digits to three, as MTGPics refs do:
/// `1` → `001`, `1a` → `001a`, `290` → `290`.
pub fn pad_collector_number(number: &str) -> String {
    let digits = number.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return number.to_owned();
    }
    format!("{:0>3}{}", &number[..digits], &number[digits..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_set_codes() {
        assert_eq!(mtgpics_set_code("10E"), "xth");
        assert_eq!(mtgpics_set_code("c13"), "13c");
        assert_eq!(mtgpics_set_code("dom"), "dom");
    }

    #[test]
    fn pads_numbers() {
        assert_eq!(pad_collector_number("1"), "001");
        assert_eq!(pad_collector_number("12"), "012");
        assert_eq!(pad_collector_number("290"), "290");
        assert_eq!(pad_collector_number("1a"), "001a");
        assert_eq!(pad_collector_number("1★"), "001★");
        assert_eq!(pad_collector_number("A-12"), "A-12");
    }
}
