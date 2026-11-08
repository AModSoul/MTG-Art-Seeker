//! The places a face's art is known to be, so the user can go back to one:
//! MTGPics, where the history logged it; Scryfall; or any of the copies they
//! found themselves ([`crate::manual`]). Only addresses are kept: saving the
//! art from one downloads it again, in place of the file the run saved.

use std::sync::atomic::{AtomicU64, Ordering};

use futures::future::join_all;
use reqwest::Url;
use serde::Serialize;
use ts_rs::TS;

use crate::{
    engine::{ArtSource, SavedFile, TempFile},
    faces::plan_faces,
    history::{FoundByHand, History},
    http::Http,
    imagesize::ImageSize,
    manual::{put_in_place, source_dir, Destination, Site, Target},
    settings::Settings,
    Error, Result,
};

/// Numbers temporary files, so two saves at once can't share one.
static TEMP_IDS: AtomicU64 = AtomicU64::new(0);

/// A place a face's art is known to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnownSource {
    pub source: ArtSource,
    /// For a copy the user found, which one, as [`save_from`] takes it.
    #[ts(type = "number | null")]
    pub copy: Option<i64>,
    /// Where the user started looking for a copy they found.
    pub site: Option<Site>,
    /// The site a copy they found was on, such as `artstation.com`.
    pub host: Option<String>,
    /// The image's size, when it could be told.
    pub size: Option<ImageSize>,
    /// Whether it's the art saved now.
    pub current: bool,
}

/// The places known to have `target`'s art: MTGPics, if the history logged
/// where; Scryfall, if it has an image; and every copy the user found, largest
/// first. `current` is the file saved now. Each comes with its image's size:
/// the saved file's, one the history noted, or else read from the start of
/// the image, and noted.
pub async fn known_sources(
    http: &Http,
    history: Option<&History>,
    target: &Target,
    settings: &Settings,
    current: &SavedFile,
) -> Vec<KnownSource> {
    let mut known: Vec<(KnownSource, String)> = Vec::new();
    let mut plain = |source: ArtSource, url: Option<String>| {
        if let Some(url) = url {
            let is_current = current.source == source;
            let size = if is_current { current.size } else { None };
            let plain = KnownSource {
                source,
                copy: None,
                site: None,
                host: None,
                size,
                current: is_current,
            };
            known.push((plain, url));
        }
    };
    plain(ArtSource::Mtgpics, mtgpics_url(history, target, settings));
    plain(ArtSource::Scryfall, target.face.scryfall_url.clone());
    let art = target.card.illustration(target.face.index);
    if let (Some(history), Some(art)) = (history, art) {
        // The copy in use is the one later runs save, and so the one saved now.
        let in_use = history.found_by_hand(art).ok().flatten().map(|found| found.url);
        for (id, copy) in history.found_copies(art).unwrap_or_default() {
            let host = Url::parse(copy.page.as_deref().unwrap_or(&copy.url))
                .ok()
                .and_then(|url| url.host_str().map(|host| host.trim_start_matches("www.").to_owned()));
            let is_current = current.source == ArtSource::Web && in_use.as_ref() == Some(&copy.url);
            let found = KnownSource {
                source: ArtSource::Web,
                copy: Some(id),
                site: copy.site.as_deref().and_then(Site::from_key),
                host,
                size: copy.size,
                current: is_current,
            };
            known.push((found, copy.url));
        }
    }
    join_all(known.into_iter().map(|(mut source, url)| async move {
        if source.size.is_none() {
            source.size = image_size(http, history, &url).await;
        }
        source
    }))
    .await
}

/// The size of the image at `url`: as the history noted it, or read from the
/// start of the image and noted.
async fn image_size(http: &Http, history: Option<&History>, url: &str) -> Option<ImageSize> {
    if let Some(size) = history.and_then(|history| history.image_size(url).ok().flatten()) {
        return Some(size);
    }
    let size = http.image_size(url).await.ok().flatten()?;
    if let Some(history) = history {
        let _ = history.remember_image_size(url, size);
    }
    Some(size)
}

fn mtgpics_url(history: Option<&History>, target: &Target, settings: &Settings) -> Option<String> {
    let faces = plan_faces(&target.card, settings).len();
    let remembered = history?.remembered(&target.card, faces).ok().flatten()?;
    remembered.mtgpics.get(target.face.index).cloned().flatten()
}

/// Saves `target`'s art again from `source` (for [`ArtSource::Web`], the copy
/// the user found that `copy` names), in place of the file the run saved. The
/// copy saved becomes the one later runs save; going back to MTGPics or
/// Scryfall stops them saving a found copy. A found copy that's gone from
/// where it was is forgotten. Says which file it replaced.
pub async fn save_from(
    http: &Http,
    history: Option<&History>,
    target: &Target,
    destination: Destination<'_>,
    source: ArtSource,
    copy: Option<i64>,
) -> Result<(SavedFile, Option<String>)> {
    let name = &target.face.name;
    let art = target.card.illustration(target.face.index);
    let (url, page) = match source {
        ArtSource::Mtgpics => mtgpics_url(history, target, destination.settings)
            .map(|url| (url, None))
            .ok_or_else(|| Error::Other(format!("MTGPics' art for {name} isn't known. Download the card again.")))?,
        ArtSource::Scryfall => target
            .face
            .scryfall_url
            .clone()
            .map(|url| (url, None))
            .ok_or_else(|| Error::Other(format!("Scryfall has no image of {name}.")))?,
        ArtSource::Web => {
            let found = history
                .zip(copy)
                .and_then(|(history, id)| history.found_copy(id).ok().flatten())
                // Only a copy of this face's art.
                .filter(|(of, _)| Some(of.as_str()) == art)
                .ok_or_else(|| Error::Other(format!("That copy of {name}'s art is no longer known.")))?;
            (found.1.url, found.1.page)
        }
    };

    let dir = source_dir(target, destination, source);
    tokio::fs::create_dir_all(&dir).await?;
    let temp = TempFile::beside(&dir, TEMP_IDS.fetch_add(1, Ordering::Relaxed));
    let Some(kind) = http.download(&url, temp.path()).await? else {
        if let (Some(history), Some(id), ArtSource::Web) = (history, copy, source) {
            let _ = history.forget_found_copy(id);
        }
        return Err(Error::Other(format!("{name}'s art is gone from {url}")));
    };
    let (file, replaced) = put_in_place(target, destination, source, &dir, temp, kind).await?;

    if let (Some(history), Some(size)) = (history, file.size) {
        let _ = history.remember_image_size(&url, size);
    }
    if let (Some(history), Some(art)) = (history, art) {
        let _ = match source {
            ArtSource::Web => history.remember_found_by_hand(
                art,
                &FoundByHand {
                    url,
                    page: page.clone(),
                },
            ),
            _ => history.forget_found_by_hand(art),
        };
    }
    Ok((SavedFile { page, ..file }, replaced))
}
