//! Ready-made card lists: EDHREC's most-played cards.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{http::Http, Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresetInfo {
    pub id: String,
    pub label: String,
    pub group: String,
}

/// (id, label, EDHREC page). The page is `json.edhrec.com/pages/<page>.json`.
const EDHREC_LISTS: [(&str, &str, &str); 13] = [
    ("edhrec-top-year", "Top 100, past 2 years", "top/year"),
    ("edhrec-top-month", "Top 100, past month", "top/month"),
    ("edhrec-top-week", "Top 100, past week", "top/week"),
    ("edhrec-salt", "Saltiest 100", "top/salt"),
    ("edhrec-white", "Top white cards", "top/white"),
    ("edhrec-blue", "Top blue cards", "top/blue"),
    ("edhrec-black", "Top black cards", "top/black"),
    ("edhrec-red", "Top red cards", "top/red"),
    ("edhrec-green", "Top green cards", "top/green"),
    ("edhrec-multicolor", "Top multicolor cards", "top/multicolor"),
    ("edhrec-colorless", "Top colorless cards", "top/colorless"),
    ("edhrec-lands", "Top lands", "top/lands"),
    ("edhrec-commanders", "Top commanders, past 2 years", "commanders/year"),
];

pub fn presets() -> Vec<PresetInfo> {
    EDHREC_LISTS
        .iter()
        .map(|(id, label, _)| PresetInfo {
            id: (*id).into(),
            label: (*label).into(),
            group: "EDHREC".into(),
        })
        .collect()
}

#[derive(Deserialize)]
struct EdhrecPage {
    container: EdhrecContainer,
}
#[derive(Deserialize)]
struct EdhrecContainer {
    json_dict: EdhrecDict,
}
#[derive(Deserialize)]
struct EdhrecDict {
    #[serde(default)]
    cardlists: Vec<EdhrecList>,
}
#[derive(Deserialize)]
struct EdhrecList {
    #[serde(default)]
    cardviews: Vec<EdhrecCard>,
}
#[derive(Deserialize)]
struct EdhrecCard {
    name: String,
}

/// The card names in a preset, in its order.
pub async fn preset_names(http: &Http, id: &str) -> Result<Vec<String>> {
    let (_, label, page) = EDHREC_LISTS
        .iter()
        .find(|(preset, _, _)| *preset == id)
        .ok_or_else(|| Error::Other(format!("unknown preset {id}")))?;
    let url = format!("{}/pages/{page}.json", http.endpoints().edhrec);
    let page: EdhrecPage = http
        .get_json(&url)
        .await?
        .ok_or_else(|| Error::Other(format!("EDHREC no longer has the list \u{201c}{label}\u{201d}")))?;
    let mut names: Vec<String> = Vec::new();
    for card in page
        .container
        .json_dict
        .cardlists
        .into_iter()
        .flat_map(|list| list.cardviews)
    {
        if !names.contains(&card.name) {
            names.push(card.name);
        }
    }
    Ok(names)
}
