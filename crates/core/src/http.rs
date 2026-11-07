//! The one HTTP client every request goes through: rate limits, retries, and the
//! User-Agent all live here, so no caller can skip them.

use std::{collections::HashMap, path::Path, time::Duration};

use futures::StreamExt;
use reqwest::{header, Client, RequestBuilder, Response, StatusCode, Url};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::AsyncWriteExt,
    sync::Mutex,
    time::{sleep, sleep_until, Instant},
};

use crate::{Error, Result};

pub const USER_AGENT: &str = concat!(
    "MTGArtDownloaderRust/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/AModSoul/mtg-art-downloader-rust)"
);

/// Where each service lives. Tests point these at a local mock server.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub scryfall_api: String,
    pub mtgpics: String,
    pub edhrec: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            scryfall_api: "https://api.scryfall.com".into(),
            mtgpics: "https://www.mtgpics.com".into(),
            edhrec: "https://json.edhrec.com".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub endpoints: Endpoints,
    /// Tries per request, the first included, for timeouts, dropped
    /// connections, 5xx responses and 429s.
    pub max_attempts: u32,
    /// The wait before the first retry; it doubles each time after that.
    pub retry_delay: Duration,
    /// How long to hold back after a 429 that doesn't say how long
    /// (Scryfall locks a client out for 30 s).
    pub rate_limited_wait: Duration,
    /// The gap between two MTGPics requests.
    pub mtgpics_spacing: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            endpoints: Endpoints::default(),
            max_attempts: 3,
            retry_delay: Duration::from_secs(1),
            rate_limited_wait: Duration::from_secs(30),
            mtgpics_spacing: Duration::from_millis(100),
        }
    }
}

/// Scryfall's published limits, as the gap between two calls to one endpoint
/// (<https://scryfall.com/docs/api/rate-limits>).
const SCRYFALL_SLOW_ENDPOINTS: [&str; 4] = ["/cards/search", "/cards/named", "/cards/random", "/cards/collection"];
const SCRYFALL_SLOW_SPACING: Duration = Duration::from_millis(500);
/// Every other Scryfall endpoint's limit, and the floor between any two calls.
const SCRYFALL_SPACING: Duration = Duration::from_millis(100);

/// The longest `Retry-After` honoured, so a bad header can't stall a run.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(120);

pub struct Http {
    client: Client,
    config: HttpConfig,
    scryfall_authority: Option<String>,
    mtgpics_authority: Option<String>,
    scryfall: Limiter,
    mtgpics: Limiter,
}

/// Which limiter, if any, a request waits on.
enum Gate {
    Scryfall { endpoint: String, spacing: Duration },
    Mtgpics,
    Open,
}

impl Http {
    pub fn new(config: HttpConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(90))
            .build()?;
        Ok(Self {
            client,
            scryfall_authority: authority(&config.endpoints.scryfall_api),
            mtgpics_authority: authority(&config.endpoints.mtgpics),
            scryfall: Limiter::new(SCRYFALL_SPACING),
            mtgpics: Limiter::new(config.mtgpics_spacing),
            config,
        })
    }

    pub fn endpoints(&self) -> &Endpoints {
        &self.config.endpoints
    }

    /// GETs a URL. The response is returned whatever its status.
    pub async fn get(&self, url: &str) -> Result<Response> {
        self.send(url, |client| client.get(url)).await
    }

    /// POSTs a JSON body. The response is returned whatever its status.
    pub async fn post_json<T: Serialize + ?Sized>(&self, url: &str, body: &T) -> Result<Response> {
        self.send(url, |client| client.post(url).json(body)).await
    }

    /// GETs and decodes JSON; `None` for a 404.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<Option<T>> {
        match ok_or_missing(url, self.get(url).await?).await? {
            Some(response) => Ok(Some(response.json().await?)),
            None => Ok(None),
        }
    }

    /// GETs a page as text, decoded with the charset it declares; `None` for a 404.
    pub async fn get_text(&self, url: &str) -> Result<Option<String>> {
        match ok_or_missing(url, self.get(url).await?).await? {
            Some(response) => Ok(Some(response.text().await?)),
            None => Ok(None),
        }
    }

    /// Streams an image to `dest`; `None` for a 404. Fails, leaving whatever
    /// was written, if the body isn't a JPEG or PNG.
    pub async fn download(&self, url: &str, dest: &Path) -> Result<Option<ImageKind>> {
        let Some(response) = ok_or_missing(url, self.get(url).await?).await? else {
            return Ok(None);
        };
        let mut file = tokio::fs::File::create(dest).await?;
        let mut head = Vec::with_capacity(8);
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk?;
            if head.len() < 8 {
                head.extend(chunk.iter().take(8 - head.len()));
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        ImageKind::sniff(&head).map(Some).ok_or_else(|| Error::Invalid {
            url: url.to_owned(),
            reason: "the response is not a JPEG or PNG image".into(),
        })
    }

    async fn send(&self, url: &str, build: impl Fn(&Client) -> RequestBuilder) -> Result<Response> {
        let gate = self.gate(url)?;
        let mut attempt = 1;
        loop {
            match &gate {
                Gate::Scryfall { endpoint, spacing } => self.scryfall.wait(endpoint, *spacing).await,
                Gate::Mtgpics => self.mtgpics.wait("", self.config.mtgpics_spacing).await,
                Gate::Open => {}
            }
            let outcome = build(&self.client).send().await;
            let retry_in = match &outcome {
                Ok(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
                    let wait = retry_after(response).unwrap_or(self.config.rate_limited_wait);
                    // A limited host holds every queued call back, not just this
                    // one; the next `wait` above then sleeps it out.
                    match &gate {
                        Gate::Scryfall { .. } => self.scryfall.pause_for(wait).await,
                        Gate::Mtgpics => self.mtgpics.pause_for(wait).await,
                        Gate::Open => {}
                    }
                    Some(if matches!(gate, Gate::Open) {
                        wait
                    } else {
                        Duration::ZERO
                    })
                }
                Ok(response) if response.status().is_server_error() => Some(self.backoff(attempt)),
                Ok(_) => None,
                Err(error) if error.is_timeout() || error.is_connect() || error.is_request() => {
                    Some(self.backoff(attempt))
                }
                Err(_) => None,
            };
            match retry_in {
                Some(delay) if attempt < self.config.max_attempts => {
                    sleep(delay).await;
                    attempt += 1;
                }
                _ => return Ok(outcome?),
            }
        }
    }

    fn gate(&self, url: &str) -> Result<Gate> {
        let parsed = Url::parse(url).map_err(|e| Error::Invalid {
            url: url.to_owned(),
            reason: e.to_string(),
        })?;
        let host = url_authority(&parsed);
        if host.is_some() && host == self.scryfall_authority {
            let endpoint = parsed.path().trim_end_matches('/');
            let slow = SCRYFALL_SLOW_ENDPOINTS.contains(&endpoint);
            return Ok(Gate::Scryfall {
                endpoint: if slow { endpoint.to_owned() } else { String::new() },
                spacing: if slow { SCRYFALL_SLOW_SPACING } else { SCRYFALL_SPACING },
            });
        }
        if host.is_some() && host == self.mtgpics_authority {
            return Ok(Gate::Mtgpics);
        }
        Ok(Gate::Open)
    }

    fn backoff(&self, attempt: u32) -> Duration {
        self.config.retry_delay * 2u32.saturating_pow(attempt - 1)
    }
}

/// The response if it succeeded, `None` for a 404, otherwise an error carrying
/// the service's explanation when it gave one.
async fn ok_or_missing(url: &str, response: Response) -> Result<Option<Response>> {
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if status.is_success() {
        return Ok(Some(response));
    }
    // Scryfall explains a bad query in an error object's `details`.
    let detail = response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|body| body.get("details")?.as_str().map(str::to_owned));
    Err(Error::Status {
        status: status.as_u16(),
        url: url.to_owned(),
        detail,
    })
}

fn retry_after(response: &Response) -> Option<Duration> {
    let secs: u64 = response
        .headers()
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(secs).min(MAX_RETRY_AFTER))
}

fn authority(base: &str) -> Option<String> {
    Url::parse(base).ok().as_ref().and_then(url_authority)
}

fn url_authority(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

/// The image formats a download is accepted as, told apart by their first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
}

impl ImageKind {
    pub fn sniff(head: &[u8]) -> Option<Self> {
        if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if head.starts_with(&[0x89, b'P', b'N', b'G']) {
            Some(Self::Png)
        } else {
            None
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
}

/// Spaces calls out: each key has its own gap, and no two calls on any keys go
/// out closer than `floor`.
///
/// A call books its slot before it waits. Reading the time of the last call
/// and sleeping the remainder would hand every caller that arrives together the
/// same slot. Ported from Card Anvil's `ScryfallClient`.
pub struct Limiter {
    floor: Duration,
    state: Mutex<LimiterState>,
}

#[derive(Default)]
struct LimiterState {
    /// When each key's next call may go out.
    next_by_key: HashMap<String, Instant>,
    /// Every booked send time not yet long past, ascending.
    booked: Vec<Instant>,
    /// Set by a 429: nothing goes out before this.
    paused_until: Option<Instant>,
}

impl Limiter {
    pub fn new(floor: Duration) -> Self {
        Self {
            floor,
            state: Mutex::new(LimiterState::default()),
        }
    }

    /// Books the next slot for `key`, at least `spacing` after its last one,
    /// and waits for it.
    pub async fn wait(&self, key: &str, spacing: Duration) {
        let at = {
            let mut state = self.state.lock().await;
            let now = Instant::now();
            let floor = self.floor;
            state.booked.retain(|t| *t + floor > now);
            let mut at = now.max(state.next_by_key.get(key).copied().unwrap_or(now));
            if let Some(paused) = state.paused_until {
                at = at.max(paused);
            }
            // The first gap at least `floor` clear of every other booked call.
            for t in &state.booked {
                if *t >= at + floor {
                    break;
                }
                if *t + floor > at {
                    at = *t + floor;
                }
            }
            state.next_by_key.insert(key.to_owned(), at + spacing);
            let position = state.booked.partition_point(|t| *t <= at);
            state.booked.insert(position, at);
            at
        };
        sleep_until(at).await;
    }

    /// Holds every call booked from now on back for `wait`.
    pub async fn pause_for(&self, wait: Duration) {
        let mut state = self.state.lock().await;
        let until = Instant::now() + wait;
        state.paused_until = Some(state.paused_until.map_or(until, |p| p.max(until)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn spaces_one_key() {
        let limiter = Limiter::new(Duration::from_millis(100));
        let start = Instant::now();
        let mut sent = Vec::new();
        for _ in 0..3 {
            limiter.wait("/cards/search", Duration::from_millis(500)).await;
            sent.push(start.elapsed().as_millis());
        }
        assert_eq!(sent, [0, 500, 1000]);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_callers_get_their_own_slots() {
        let limiter = Limiter::new(Duration::from_millis(100));
        let start = Instant::now();
        let waits = (0..3).map(|_| async {
            limiter.wait("", Duration::from_millis(100)).await;
            start.elapsed().as_millis()
        });
        let mut sent = futures::future::join_all(waits).await;
        sent.sort_unstable();
        assert_eq!(sent, [0, 100, 200]);
    }

    #[tokio::test(start_paused = true)]
    async fn keys_share_the_floor_but_not_each_others_queue() {
        let limiter = Limiter::new(Duration::from_millis(100));
        let start = Instant::now();
        let call = |key: &'static str, spacing: u64| {
            let limiter = &limiter;
            async move {
                limiter.wait(key, Duration::from_millis(spacing)).await;
                (key, start.elapsed().as_millis())
            }
        };
        // A burst on the slow endpoint books seconds ahead, but a call to
        // another endpoint isn't queued behind it — it only keeps `floor`
        // clear of the burst's first call.
        let sent = futures::future::join_all([
            call("/cards/search", 500),
            call("/cards/search", 500),
            call("/cards/search", 500),
            call("", 100),
        ])
        .await;
        assert_eq!(
            sent,
            [
                ("/cards/search", 0),
                ("/cards/search", 500),
                ("/cards/search", 1000),
                ("", 100)
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn pause_holds_everything_back() {
        let limiter = Limiter::new(Duration::from_millis(100));
        let start = Instant::now();
        limiter.pause_for(Duration::from_secs(30)).await;
        limiter.wait("", Duration::from_millis(100)).await;
        assert_eq!(start.elapsed().as_secs(), 30);
    }

    #[test]
    fn sniffs_images() {
        assert_eq!(ImageKind::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageKind::Jpeg));
        assert_eq!(ImageKind::sniff(b"\x89PNG\r\n\x1a\n"), Some(ImageKind::Png));
        assert_eq!(ImageKind::sniff(b"<html>"), None);
    }
}
