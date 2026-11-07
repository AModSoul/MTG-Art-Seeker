//! Art the user found themselves, saved in place of Scryfall's.
//!
//! For a face that got Scryfall's art, the desktop app opens a window of its
//! own on one of the [`Site`]s: Google Lens searching for Scryfall's art
//! crop, or a search for the artist on a site artists post their work. The
//! user finds a larger copy and saves it, and the file they saved comes here:
//! [`prepare`] says which card and face it's for, and [`import`] checks it
//! against the art crop
//! ([`crate::verify`]) and saves it where the run saved Scryfall's, under the
//! same name but in the `web` source folder, in place of Scryfall's file. The
//! history remembers where it came from, so later runs save it from there.
//!
//! An image that doesn't look like a larger copy of the art isn't saved
//! unless the user says to: a person picked it, but may have picked wrong.

use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use ts_rs::TS;

use crate::{
    card::Card,
    carddb::CardDb,
    engine::{face_dir, face_stem, output_dir_for, place, ArtSource, PathReserver, SavedFile, TempFile},
    faces::{plan_faces, FacePlan},
    history::{FoundByHand, History},
    http::{Http, ImageKind},
    scryfall::{self, Identifier},
    settings::Settings,
    verify::{self, Verdict},
    Error, Result,
};

/// Numbers temporary files, so two imports at once can't share one.
static TEMP_IDS: AtomicU64 = AtomicU64::new(0);

/// The face a larger copy is being looked for.
#[derive(Debug, Clone)]
pub struct Target {
    pub card: Card,
    pub face: FacePlan,
}

impl Target {
    /// Scryfall's art crop of the face: what Lens is asked to search for, and
    /// what an image the user saves is checked against.
    pub fn art_crop(&self) -> &str {
        self.card
            .art_crop(self.face.index)
            .expect("prepare checks there's an art crop")
    }

    /// Where to open `site` to look for a larger copy of the face's art.
    pub fn search_url(&self, site: Site) -> Url {
        let parse = |url: &str| Url::parse(url).expect("a valid URL");
        let artist = self.face.artist.as_str();
        match site {
            Site::Lens => {
                let mut url = parse("https://lens.google.com/uploadbyurl");
                url.query_pairs_mut().append_pair("url", self.art_crop());
                url
            }
            Site::Artstation => {
                let mut url = parse("https://www.artstation.com/search/artists");
                url.query_pairs_mut().append_pair("query", artist);
                url
            }
            Site::Deviantart => {
                let mut url = parse("https://www.deviantart.com/search");
                url.query_pairs_mut().append_pair("q", artist);
                url
            }
            // Pixiv's user search wants a login; its keyword search doesn't.
            Site::Pixiv => {
                let mut url = parse("https://www.pixiv.net/tags");
                url.path_segments_mut().expect("an http URL").push(artist);
                url
            }
        }
    }
}

/// Where the user can look for a larger copy of a face's art.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Site {
    /// Google Lens, searching for Scryfall's art crop.
    Lens,
    /// ArtStation's search for the artist.
    Artstation,
    /// DeviantArt's search for the artist.
    Deviantart,
    /// Pixiv's search for the artist's name.
    Pixiv,
}

impl Site {
    pub fn name(self) -> &'static str {
        match self {
            Self::Lens => "Google Lens",
            Self::Artstation => "ArtStation",
            Self::Deviantart => "DeviantArt",
            Self::Pixiv => "Pixiv",
        }
    }
}

/// The face `face_index` of the printing `set` `number`, as a run with
/// `settings` would save it. Looked up in the card data first.
pub async fn prepare(
    http: &Http,
    cards: Option<&CardDb>,
    settings: &Settings,
    set: &str,
    number: &str,
    face_index: usize,
) -> Result<Target> {
    let local = cards.and_then(|db| db.by_number(set, number).ok().flatten());
    let card = match local {
        Some(card) => card,
        None => {
            let id = Identifier::SetNumber {
                set: set.to_lowercase(),
                collector_number: number.to_owned(),
            };
            scryfall::collection(http, &[id])
                .await
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(|| Error::Other(format!("Scryfall has no card {} {number}", set.to_uppercase())))?
        }
    };
    let face = plan_faces(&card, settings)
        .into_iter()
        .find(|face| face.index == face_index)
        .ok_or_else(|| Error::Other(format!("{} has no image {face_index}", card.name)))?;
    if card.art_crop(face.index).is_none() {
        return Err(Error::Other(format!(
            "Scryfall has no art crop of {} to search with",
            face.name
        )));
    }
    Ok(Target { card, face })
}

/// Why an image the user saved wasn't saved for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Doubt {
    /// The same art, but no larger than Scryfall's.
    Smaller,
    /// The art inside a card's frame.
    CardScan,
    /// It doesn't look like this art.
    Different,
}

/// What the app reports about an image the user saved while looking for a
/// larger copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "event", rename_all = "camelCase")]
#[ts(export)]
pub enum FoundEvent {
    /// It's saved as the face's art: in `job` of run `run`, `file` takes the
    /// place of the file at `replaced`.
    Saved {
        run: u32,
        job: u32,
        file: SavedFile,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        replaced: Option<String>,
    },
    /// It doesn't look like a larger copy of `face`'s art, and is kept until
    /// the user says whether to save it anyway; `token` names it.
    Doubtful { token: u32, face: String, doubt: Doubt },
    /// It couldn't be saved.
    Failed { message: String },
}

/// What became of an image the user saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Imported {
    Saved {
        file: SavedFile,
        /// Scryfall's file it replaced, if there was one.
        replaced: Option<String>,
    },
    /// Not saved: it doesn't look like a larger copy of the art. Importing it
    /// again with `force` saves it anyway.
    Doubtful(Doubt),
}

/// Where the image the user saved came from.
#[derive(Debug, Clone, Copy)]
pub struct Found<'a> {
    pub image_url: &'a str,
    /// The page they were on when they saved it.
    pub page: Option<&'a str>,
}

/// Where the face's art was saved by the run it came from.
#[derive(Debug, Clone, Copy)]
pub struct Destination<'a> {
    /// The run's settings, which decide the folders and the name.
    pub settings: &'a Settings,
    /// The run's list folder, if it had one.
    pub list_folder: Option<&'a str>,
    /// The file the run saved for the face, which this one replaces.
    pub replace: Option<&'a Path>,
}

/// Saves the image at `downloaded` as the art of `target`'s face, if it's a
/// larger copy of it or `force` is set. `downloaded` is left where it is.
pub async fn import(
    http: &Http,
    history: Option<&History>,
    target: &Target,
    destination: Destination<'_>,
    downloaded: &Path,
    found: Found<'_>,
    force: bool,
) -> Result<Imported> {
    let invalid = |reason: &str| Error::Invalid {
        url: found.image_url.to_owned(),
        reason: reason.to_owned(),
    };
    let mut head = Vec::with_capacity(12);
    tokio::fs::File::open(downloaded)
        .await?
        .take(12)
        .read_to_end(&mut head)
        .await?;
    let kind = ImageKind::sniff(&head).ok_or_else(|| invalid("it isn't a JPEG, PNG or WebP image"))?;

    if !force {
        if let Some(doubt) = doubt(http, target, downloaded).await? {
            return Ok(Imported::Doubtful(doubt));
        }
    }

    let Destination {
        settings,
        list_folder,
        replace,
    } = destination;
    let (card, face) = (&target.card, &target.face);
    let dir = face_dir(&output_dir_for(list_folder, settings), settings, ArtSource::Web, face);
    tokio::fs::create_dir_all(&dir).await?;
    let temp = TempFile::beside(&dir, TEMP_IDS.fetch_add(1, Ordering::Relaxed));
    tokio::fs::copy(downloaded, temp.path()).await?;

    let stem = face_stem(settings, card, face);
    let named = dir.join(format!("{stem}.{}", kind.extension()));
    // Taking the place of the file it replaces is no clash; any other is.
    let overwrite = settings.overwrite || replace == Some(named.as_path());
    let path = PathReserver::default().reserve(&dir, &stem, kind.extension(), overwrite);
    let file = place(temp, kind, &path, card, face, ArtSource::Web).await?;
    let file = SavedFile {
        page: found.page.map(str::to_owned),
        ..file
    };

    let replaced = match replace {
        Some(old) if old != path => match tokio::fs::remove_file(old).await {
            Ok(()) => Some(old.to_string_lossy().into_owned()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        },
        Some(old) => Some(old.to_string_lossy().into_owned()),
        None => None,
    };

    if let (Some(history), Some(art)) = (history, card.illustration(face.index)) {
        let found = FoundByHand {
            url: found.image_url.to_owned(),
            page: found.page.map(str::to_owned),
        };
        // The file is saved either way; only the next run would miss it.
        let _ = history.remember_found_by_hand(art, &found);
    }
    Ok(Imported::Saved { file, replaced })
}

/// Why `downloaded` isn't a larger copy of the face's art, if it isn't.
async fn doubt(http: &Http, target: &Target, downloaded: &Path) -> Result<Option<Doubt>> {
    let crop = downloaded.with_extension(format!("art-crop-{}", TEMP_IDS.fetch_add(1, Ordering::Relaxed)));
    if http.download(target.art_crop(), &crop).await?.is_none() {
        return Err(Error::Other(format!(
            "Scryfall's art crop of {} is gone",
            target.face.name
        )));
    }
    let candidate = downloaded.to_owned();
    let compared = {
        let crop = crop.clone();
        tokio::task::spawn_blocking(move || verify::compare(&crop, &candidate)).await
    };
    let _ = tokio::fs::remove_file(&crop).await;
    let verdict = compared.map_err(|error| Error::Other(error.to_string()))??;
    Ok(match verdict {
        Verdict::Same { .. } => None,
        Verdict::Smaller => Some(Doubt::Smaller),
        Verdict::CardScan => Some(Doubt::CardScan),
        Verdict::Different => Some(Doubt::Different),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(artist: &str) -> Target {
        let card: Card = serde_json::from_value(serde_json::json!({
            "name": "Twilight Prophet", "set": "rix", "collector_number": "88", "artist": artist,
            "image_uris": { "art_crop": "https://cards.scryfall.io/art_crop/front/x.jpg?1" }
        }))
        .unwrap();
        let face = plan_faces(&card, &Settings::default()).remove(0);
        Target { card, face }
    }

    #[test]
    fn opens_each_site_on_the_art_or_its_artist() {
        let target = target("Seb McKinnon");
        let url = |site| target.search_url(site).to_string();
        assert_eq!(
            url(Site::Lens),
            "https://lens.google.com/uploadbyurl?url=https%3A%2F%2Fcards.scryfall.io%2Fart_crop%2Ffront%2Fx.jpg%3F1"
        );
        assert_eq!(
            url(Site::Artstation),
            "https://www.artstation.com/search/artists?query=Seb+McKinnon"
        );
        assert_eq!(
            url(Site::Deviantart),
            "https://www.deviantart.com/search?q=Seb+McKinnon"
        );
        assert_eq!(url(Site::Pixiv), "https://www.pixiv.net/tags/Seb%20McKinnon");
    }

    #[test]
    fn keeps_an_artists_name_in_one_piece() {
        // A slash or an ampersand stays part of the name.
        let target = target("Jesper Ejsing & Ron Spencer/RK");
        assert_eq!(
            target.search_url(Site::Pixiv).as_str(),
            "https://www.pixiv.net/tags/Jesper%20Ejsing%20&%20Ron%20Spencer%2FRK"
        );
        assert_eq!(
            target.search_url(Site::Artstation).as_str(),
            "https://www.artstation.com/search/artists?query=Jesper+Ejsing+%26+Ron+Spencer%2FRK"
        );
    }
}
