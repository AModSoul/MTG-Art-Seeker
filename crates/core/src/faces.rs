//! Which images a card has, and where each is saved.
//!
//! A card whose faces each carry an image (transform, modal double-faced,
//! reversible…) is downloaded once per face. A split card's halves (aftermath
//! cards and Rooms are split cards too) have a painting each, which MTGPics has
//! apart and Scryfall only together: with MTGPics in use, each half is its own
//! image, and when MTGPics hasn't both, the run saves Scryfall's image of the
//! whole card instead ([`whole_card`]); and when MTGPics' halves are one
//! picture, as Duskmourn's Rooms' are, that picture, once. Everything else,
//! adventure and flip cards among them, is one download.

use crate::{
    card::Card,
    settings::{ScryfallImage, Settings, SourceMode},
};

/// One image to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacePlan {
    /// Which face, from 0. Always 0 for a card with one image.
    pub index: usize,
    /// The name the file is saved under.
    pub name: String,
    pub artist: String,
    /// Folders between the source folder and the file, outermost first.
    pub folders: Vec<String>,
    /// Scryfall's image of this face, in the format the settings ask for.
    /// `None` for a split card's half: Scryfall has only the whole card.
    pub scryfall_url: Option<String>,
    /// The artwork this image is, by which a copy the user found is kept:
    /// Scryfall's illustration id, and for a split card's half, the half's
    /// number after it, since Scryfall gives the halves one id between them.
    pub illustration: Option<String>,
}

impl FacePlan {
    /// Whether this is one half of a split card.
    pub fn is_split_half(&self) -> bool {
        self.illustration.as_deref().is_some_and(|art| art.contains('#'))
    }
}

/// Card types a type folder can be, in the order a type line is checked:
/// an `Artifact Creature` goes in `Creature`.
const CARD_TYPES: [&str; 17] = [
    "Creature",
    "Land",
    "Artifact",
    "Enchantment",
    "Planeswalker",
    "Instant",
    "Sorcery",
    "Battle",
    "Conspiracy",
    "Dungeon",
    "Emblem",
    "Hero",
    "Kindred",
    "Phenomenon",
    "Plane",
    "Scheme",
    "Vanguard",
];

pub fn plan_faces(card: &Card, settings: &Settings) -> Vec<FacePlan> {
    let image_key = |uris: &crate::card::ImageUris| image_url(uris, settings);
    let type_folder = settings.type_folders.then(|| type_folder(card));
    let layout_folder = if settings.layout_folders {
        layout_folder(card)
    } else {
        None
    };

    if has_split_halves(card, settings) {
        let art = card.front_illustration();
        return card
            .card_faces
            .iter()
            .enumerate()
            .map(|(index, face)| FacePlan {
                index,
                name: face.name.clone(),
                artist: artist(face.artist.as_deref().or(card.artist.as_deref())),
                folders: folders(&type_folder, layout_folder.clone()),
                scryfall_url: None,
                illustration: art.map(|art| format!("{art}#{index}")),
            })
            .collect();
    }

    if card.has_face_images() {
        return card
            .card_faces
            .iter()
            .enumerate()
            .map(|(index, face)| {
                let side = if index == 0 { "Front" } else { "Back" };
                FacePlan {
                    index,
                    name: face.name.clone(),
                    artist: artist(face.artist.as_deref().or(card.artist.as_deref())),
                    folders: folders(
                        &type_folder,
                        layout_folder.as_ref().map(|layout| format!("{layout} {side}")),
                    ),
                    scryfall_url: face.image_uris.as_ref().and_then(image_key),
                    illustration: face.illustration_id.clone(),
                }
            })
            .collect();
    }
    vec![whole_card(card, settings)]
}

/// The card as one image, as Scryfall has it: a split card's halves together,
/// under the card's full name. What [`plan_faces`] plans for any card whose
/// faces don't each have an image of their own, except a split card's halves.
pub fn whole_card(card: &Card, settings: &Settings) -> FacePlan {
    let type_folder = settings.type_folders.then(|| type_folder(card));
    let layout_folder = if settings.layout_folders {
        layout_folder(card)
    } else {
        None
    };
    // Adventure and flip cards are saved under their main half's name.
    let name = match card.layout.as_str() {
        "adventure" | "flip" => card.front_name().to_owned(),
        _ => card.name.clone(),
    };
    let artist = artist(
        card.artist
            .as_deref()
            .or_else(|| card.card_faces.first()?.artist.as_deref()),
    );
    FacePlan {
        index: 0,
        name,
        artist,
        folders: folders(&type_folder, layout_folder),
        scryfall_url: card.image_uris.as_ref().and_then(|uris| image_url(uris, settings)),
        illustration: card.front_illustration().map(str::to_owned),
    }
}

/// Whether a split card is planned half by half: a painting each, from
/// MTGPics, so only when MTGPics is asked.
fn has_split_halves(card: &Card, settings: &Settings) -> bool {
    card.layout == "split" && card.card_faces.len() == 2 && settings.sources != SourceMode::ScryfallOnly
}

fn image_url(uris: &crate::card::ImageUris, settings: &Settings) -> Option<String> {
    match settings.scryfall_image {
        ScryfallImage::ArtCrop => uris.art_crop.clone(),
        ScryfallImage::FullCard => uris.png.clone(),
    }
}

/// The artist a face is saved under when Scryfall doesn't name one.
pub const UNKNOWN_ARTIST: &str = "Unknown Artist";

fn artist(name: Option<&str>) -> String {
    name.map_or_else(|| UNKNOWN_ARTIST.into(), deunicode::deunicode)
}

fn folders(type_folder: &Option<String>, layout_folder: Option<String>) -> Vec<String> {
    type_folder.iter().cloned().chain(layout_folder).collect()
}

/// The front's type, so a double-faced card's faces stay together: Agadeem's
/// Awakening (`Sorcery // Land`) goes in `Sorcery`.
fn type_folder(card: &Card) -> String {
    let type_line = card
        .card_faces
        .first()
        .and_then(|face| face.type_line.as_deref())
        .or(card.type_line.as_deref())
        .unwrap_or_default();
    CARD_TYPES
        .iter()
        .find(|t| type_line.contains(*t))
        .map_or_else(|| "Other".into(), |t| (*t).to_owned())
}

/// The folder a layout's art goes in; `None` for an ordinary card.
fn layout_folder(card: &Card) -> Option<String> {
    if card.type_line.as_deref().is_some_and(|t| t.contains("Basic Land")) {
        return Some("Basic".into());
    }
    let name = match card.layout.as_str() {
        "" | "normal" => return None,
        "modal_dfc" => "MDFC".into(),
        "reversible_card" => "Reversible".into(),
        "double_faced_token" => "Double-Faced Token".into(),
        other => title_case(other),
    };
    Some(name)
}

/// `art_series` → `Art Series`.
fn title_case(layout: &str) -> String {
    layout
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_cards() -> Vec<Card> {
        let json = include_str!("../tests/fixtures/collection.json");
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        serde_json::from_value(value["data"].clone()).unwrap()
    }

    fn card(name_prefix: &str) -> Card {
        fixture_cards()
            .into_iter()
            .find(|c| c.name.starts_with(name_prefix))
            .unwrap()
    }

    #[test]
    fn a_normal_card_is_one_image_at_the_top() {
        let faces = plan_faces(&card("Karn"), &Settings::default());
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].name, "Karn, Scion of Urza");
        assert_eq!(faces[0].artist, "Chase Stone");
        assert!(faces[0].folders.is_empty());
        assert!(faces[0].scryfall_url.as_deref().unwrap().contains("/art_crop/"));
    }

    #[test]
    fn a_double_faced_card_is_one_image_per_face() {
        let faces = plan_faces(&card("Agadeem"), &Settings::default());
        let names: Vec<_> = faces.iter().map(|f| (f.name.as_str(), f.folders.clone())).collect();
        assert_eq!(
            names,
            [
                ("Agadeem's Awakening", vec!["MDFC Front".to_owned()]),
                ("Agadeem, the Undercrypt", vec!["MDFC Back".to_owned()]),
            ]
        );
        assert!(faces.iter().all(|f| f.scryfall_url.is_some()));

        let delver = plan_faces(&card("Delver"), &Settings::default());
        assert_eq!(delver[1].folders, ["Transform Back"]);
    }

    #[test]
    fn adventure_and_flip_cards_take_their_main_name() {
        let giant = plan_faces(&card("Bonecrusher"), &Settings::default());
        assert_eq!((giant.len(), giant[0].name.as_str()), (1, "Bonecrusher Giant"));
        assert_eq!(giant[0].folders, ["Adventure"]);

        let akki = plan_faces(&card("Akki"), &Settings::default());
        assert_eq!((akki.len(), akki[0].name.as_str()), (1, "Akki Lavarunner"));
        assert_eq!(akki[0].folders, ["Flip"]);
    }

    #[test]
    fn folders_follow_the_settings() {
        let settings = Settings {
            type_folders: true,
            scryfall_image: ScryfallImage::FullCard,
            ..Settings::default()
        };
        assert_eq!(plan_faces(&card("Swamp"), &settings)[0].folders, ["Land", "Basic"]);
        let agadeem = plan_faces(&card("Agadeem"), &settings);
        assert_eq!(agadeem[0].folders, ["Sorcery", "MDFC Front"]);
        assert_eq!(agadeem[1].folders, ["Sorcery", "MDFC Back"]);
        assert!(agadeem[0].scryfall_url.as_deref().unwrap().contains("/png/"));

        let flat = Settings {
            layout_folders: false,
            ..Settings::default()
        };
        assert!(plan_faces(&card("Agadeem"), &flat).iter().all(|f| f.folders.is_empty()));
    }

    fn fire_ice() -> Card {
        serde_json::from_value(serde_json::json!({
            "name": "Fire // Ice", "layout": "split", "set": "mh2", "collector_number": "290",
            "artist": "Dan Murayama Scott", "type_line": "Instant // Instant",
            "illustration_id": "fire-ice-art",
            "image_uris": { "art_crop": "https://cards.scryfall.io/art_crop/x.jpg" },
            "card_faces": [
                { "name": "Fire", "artist": "Dan Murayama Scott", "illustration_id": "fire-ice-art" },
                { "name": "Ice", "artist": "Dan Murayama Scott" }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_split_card_is_an_image_per_half_from_mtgpics() {
        let faces = plan_faces(&fire_ice(), &Settings::default());
        let halves: Vec<_> = faces
            .iter()
            .map(|f| {
                (
                    f.index,
                    f.name.as_str(),
                    f.illustration.as_deref(),
                    f.scryfall_url.is_some(),
                )
            })
            .collect();
        assert_eq!(
            halves,
            [
                (0, "Fire", Some("fire-ice-art#0"), false),
                (1, "Ice", Some("fire-ice-art#1"), false),
            ]
        );
        assert!(faces.iter().all(|f| f.folders == ["Split"] && f.is_split_half()));
    }

    #[test]
    fn a_split_card_from_scryfall_is_one_image_under_its_full_name() {
        let scryfall_only = Settings {
            sources: SourceMode::ScryfallOnly,
            ..Settings::default()
        };
        let faces = plan_faces(&fire_ice(), &scryfall_only);
        assert_eq!(faces, [whole_card(&fire_ice(), &scryfall_only)]);
        assert_eq!(faces[0].name, "Fire // Ice");
        assert_eq!(faces[0].folders, ["Split"]);
        assert_eq!(faces[0].illustration.as_deref(), Some("fire-ice-art"));
        assert!(faces[0].scryfall_url.is_some() && !faces[0].is_split_half());
    }
}
