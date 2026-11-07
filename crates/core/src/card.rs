//! The parts of a Scryfall card object this app reads.
//! See <https://scryfall.com/docs/api/cards>.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Card {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub layout: String,
    #[serde(default)]
    pub set: String,
    #[serde(default)]
    pub set_name: String,
    #[serde(default)]
    pub set_type: String,
    #[serde(default)]
    pub collector_number: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub type_line: Option<String>,
    #[serde(default)]
    pub full_art: bool,
    #[serde(default)]
    pub image_uris: Option<ImageUris>,
    #[serde(default)]
    pub card_faces: Vec<CardFace>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CardFace {
    pub name: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub type_line: Option<String>,
    #[serde(default)]
    pub image_uris: Option<ImageUris>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageUris {
    #[serde(default)]
    pub art_crop: Option<String>,
    #[serde(default)]
    pub png: Option<String>,
}

impl Card {
    /// `Name (SET) 123` — how a card is shown, and a line the list parser reads back.
    pub fn label(&self) -> String {
        format!("{} ({}) {}", self.name, self.set.to_uppercase(), self.collector_number)
    }

    /// The name on the card's front: the first face's for a card with several.
    pub fn front_name(&self) -> &str {
        self.card_faces.first().map_or(&self.name, |face| &face.name)
    }

    /// Whether each face carries its own image (transform, modal double-faced,
    /// reversible…), rather than the card having one image for all of it
    /// (split, adventure, flip).
    pub fn has_face_images(&self) -> bool {
        self.card_faces.len() >= 2 && self.card_faces.iter().all(|face| face.image_uris.is_some())
    }
}

/// A page of a Scryfall list, such as search results.
#[derive(Debug, Deserialize)]
pub struct CardList {
    #[serde(default)]
    pub data: Vec<Card>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub next_page: Option<String>,
    #[serde(default)]
    pub total_cards: Option<u32>,
}
