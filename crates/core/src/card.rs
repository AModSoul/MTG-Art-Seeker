//! The parts of a Scryfall card object this app reads.
//! See <https://scryfall.com/docs/api/cards>.
//!
//! Cards are written back out, with only these fields, by the local card
//! database ([`crate::carddb`]).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<String>,
    /// The name a crossover printing shows instead of its own — "Megatron" on
    /// a Secret Lair Blightsteel Colossus — which some deck sites export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    /// Which artwork this printing has; reprints of one art share it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub illustration_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_line: Option<String>,
    #[serde(default)]
    pub full_art: bool,
    #[serde(default)]
    pub oversized: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_uris: Option<ImageUris>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_faces: Vec<CardFace>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CardFace {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub illustration_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_line: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_uris: Option<ImageUris>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageUris {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art_crop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

    /// The artwork of one of the card's images, numbered as
    /// [`crate::faces::FacePlan::index`] numbers them.
    pub fn illustration(&self, image: usize) -> Option<&str> {
        if self.has_face_images() {
            self.card_faces.get(image)?.illustration_id.as_deref()
        } else {
            self.front_illustration()
        }
    }

    /// Scryfall's art crop of one of the card's images, numbered as
    /// [`crate::faces::FacePlan::index`] numbers them.
    pub fn art_crop(&self, image: usize) -> Option<&str> {
        let uris = if self.has_face_images() {
            self.card_faces.get(image)?.image_uris.as_ref()
        } else {
            self.image_uris.as_ref()
        };
        uris?.art_crop.as_deref()
    }

    /// The artwork on the card's front, for telling reprints of one art apart
    /// from new art.
    pub fn front_illustration(&self) -> Option<&str> {
        self.illustration_id
            .as_deref()
            .or_else(|| self.card_faces.first()?.illustration_id.as_deref())
    }

    /// What Scryfall's searches leave out unless asked for extras: tokens,
    /// emblems, planes and schemes, art series, oversized cards, and
    /// memorabilia such as championship decks.
    pub fn is_extra(&self) -> bool {
        matches!(
            self.layout.as_str(),
            "token" | "double_faced_token" | "emblem" | "art_series" | "planar" | "scheme" | "vanguard"
        ) || matches!(self.set_type.as_str(), "token" | "memorabilia" | "minigame")
            || self.oversized
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
