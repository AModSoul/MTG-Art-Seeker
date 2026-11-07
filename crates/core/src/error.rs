/// Everything that can go wrong while finding or saving art.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("{}", status_message(*.status, .url, .detail))]
    Status {
        status: u16,
        url: String,
        /// The explanation the service sent back, when it sent one (Scryfall does).
        detail: Option<String>,
    },

    #[error("unexpected response from {url}: {reason}")]
    Invalid { url: String, reason: String },

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Json(#[from] serde_json::Error),

    #[error("card data: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

fn status_message(status: u16, url: &str, detail: &Option<String>) -> String {
    match detail {
        Some(detail) => format!("{detail} (HTTP {status})"),
        None => format!("HTTP {status} from {url}"),
    }
}
