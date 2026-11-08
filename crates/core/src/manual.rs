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
//!
//! Some sites cover their images so they can't be saved. For those, the user
//! asks the app to look at the page they have open, and [`fetch_page_images`]
//! is handed the images the page lists: it downloads and checks them, and the
//! user chooses which to save.

use std::{
    cmp::Reverse,
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use futures::{stream, StreamExt};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use ts_rs::TS;

use crate::{
    card::Card,
    carddb::CardDb,
    engine::{face_dir, face_stem, output_dir_for, place, ArtSource, PathReserver, SavedFile, TempFile},
    faces::{plan_faces, whole_card, FacePlan},
    history::{FoundByHand, FoundCopy, History, KnownWebsite},
    http::{Http, ImageKind},
    imagesize::{image_size, ImageSize},
    mtgpics::{self, Artists},
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

    /// Where to open `site` to look for a larger copy of the face's art;
    /// `None` for [`Site::Website`], which has to be looked up: see
    /// [`site_url`].
    pub fn search_url(&self, site: Site) -> Option<Url> {
        let parse = |url: &str| Url::parse(url).expect("a valid URL");
        let artist = self.face.artist.as_str();
        Some(match site {
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
            // Instagram has no search without a login; Google finds the
            // artist's account.
            Site::Instagram => {
                let mut url = parse("https://www.google.com/search");
                url.query_pairs_mut()
                    .append_pair("q", &format!("\"{artist}\" site:instagram.com"));
                url
            }
            Site::Website => return None,
        })
    }
}

/// Where to open `site` for `target`: its search, or for [`Site::Website`],
/// the website MTGPics links for the artist. What MTGPics says is logged in
/// `history`, and asked again a month later.
pub async fn site_url(
    http: &Http,
    history: Option<&History>,
    artists: &Artists,
    target: &Target,
    site: Site,
) -> Result<Url> {
    if let Some(url) = target.search_url(site) {
        return Ok(url);
    }
    let artist = target.face.artist.as_str();
    let known = history
        .and_then(|history| history.artist_website(artist).ok().flatten())
        .filter(KnownWebsite::is_recent);
    let website = match known {
        Some(known) => known.url,
        None => {
            let website = mtgpics::artist_website(http, artists, artist).await?;
            if let Some(history) = history {
                let _ = history.remember_artist_website(artist, website.as_deref());
            }
            website
        }
    };
    let website = website.ok_or_else(|| Error::Other(format!("MTGPics doesn't list a website for {artist}.")))?;
    Url::parse(&website).map_err(|error| Error::Invalid {
        url: website.clone(),
        reason: error.to_string(),
    })
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
    /// A Google search for the artist's Instagram account.
    Instagram,
    /// The artist's own website, as MTGPics links it.
    Website,
}

impl Site {
    const ALL: [Self; 6] = [
        Self::Lens,
        Self::Artstation,
        Self::Deviantart,
        Self::Pixiv,
        Self::Instagram,
        Self::Website,
    ];

    /// The site's name in the history, as the front end names it too.
    pub fn key(self) -> &'static str {
        match self {
            Self::Lens => "lens",
            Self::Artstation => "artstation",
            Self::Deviantart => "deviantart",
            Self::Pixiv => "pixiv",
            Self::Instagram => "instagram",
            Self::Website => "website",
        }
    }

    /// The site [`Site::key`] names.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|site| site.key() == key)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Lens => "Google Lens",
            Self::Artstation => "ArtStation",
            Self::Deviantart => "DeviantArt",
            Self::Pixiv => "Pixiv",
            Self::Instagram => "Instagram",
            Self::Website => "the artist's website",
        }
    }
}

/// The face `face_index` of the printing `set` `number`, as a run with
/// `settings` would save it; with `face_name`, the image saved under that
/// name, which tells a split card saved whole from its first half. Looked up
/// in the card data first.
pub async fn prepare(
    http: &Http,
    cards: Option<&CardDb>,
    settings: &Settings,
    set: &str,
    number: &str,
    face_index: usize,
    face_name: Option<&str>,
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
    let mut faces = plan_faces(&card, settings);
    faces.push(whole_card(&card, settings));
    let face = faces
        .into_iter()
        .find(|face| face.index == face_index && face_name.map_or(true, |name| face.name == name))
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
    /// An image the user saved for `face`, or with `page`, the images of the
    /// page they're on, are being checked. Something else follows.
    Checking { face: String, page: bool },
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
    /// The images of the page the user asked about, for them to choose
    /// `face`'s art from; `token` names them.
    Choose {
        token: u32,
        face: String,
        images: Vec<PageChoice>,
    },
    /// The page the user asked about has no image large enough to be `face`'s art.
    NoImages { face: String },
    /// It couldn't be saved.
    Failed { message: String },
}

/// One of a page's images, as the user chooses from them: the downloaded
/// file, to show; its size; what the check said of it; and the site it's on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PageChoice {
    pub path: String,
    pub size: ImageSize,
    /// Why it isn't a larger copy of the art; `null` when it is.
    pub doubt: Option<Doubt>,
    /// The host the image is on, such as `cdna.artstation.com`.
    pub host: String,
}

impl From<&PageCandidate> for PageChoice {
    fn from(candidate: &PageCandidate) -> Self {
        Self {
            path: candidate.downloaded.to_string_lossy().into_owned(),
            size: candidate.size,
            doubt: candidate.doubt,
            host: Url::parse(&candidate.image_url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_default(),
        }
    }
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
    /// Where they started looking for it.
    pub site: Option<Site>,
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

    let dir = source_dir(target, destination, ArtSource::Web);
    tokio::fs::create_dir_all(&dir).await?;
    let temp = TempFile::beside(&dir, TEMP_IDS.fetch_add(1, Ordering::Relaxed));
    tokio::fs::copy(downloaded, temp.path()).await?;
    let (file, replaced) = put_in_place(target, destination, ArtSource::Web, &dir, temp, kind).await?;
    let file = SavedFile {
        page: found.page.map(str::to_owned),
        ..file
    };

    if let (Some(history), Some(art)) = (history, target.face.illustration.as_deref()) {
        // The file is saved either way; only the next run, or going back to
        // this copy, would miss it.
        let _ = history.remember_found_by_hand(
            art,
            &FoundByHand {
                url: found.image_url.to_owned(),
                page: found.page.map(str::to_owned),
            },
        );
        let _ = history.remember_found_copy(
            art,
            &FoundCopy {
                url: found.image_url.to_owned(),
                page: found.page.map(str::to_owned),
                site: found.site.map(|site| site.key().to_owned()),
                size: file.size,
            },
        );
    }
    Ok(Imported::Saved { file, replaced })
}

/// The folder the face's art from `source` goes in, as the run saved it.
pub(crate) fn source_dir(target: &Target, destination: Destination<'_>, source: ArtSource) -> PathBuf {
    let settings = destination.settings;
    face_dir(
        &output_dir_for(destination.list_folder, settings),
        settings,
        source,
        &target.face,
    )
}

/// Moves `temp`, an image of kind `kind` in `dir` (see [`source_dir`]), into
/// place as the face's art from `source`, in place of the file the run saved,
/// which is deleted. Says which file that was, if there was one.
pub(crate) async fn put_in_place(
    target: &Target,
    destination: Destination<'_>,
    source: ArtSource,
    dir: &Path,
    temp: TempFile,
    kind: ImageKind,
) -> Result<(SavedFile, Option<String>)> {
    let Destination { settings, replace, .. } = destination;
    let (card, face) = (&target.card, &target.face);
    let stem = face_stem(settings, card, face);
    let named = dir.join(format!("{stem}.{}", kind.extension()));
    // Taking the place of the file it replaces is no clash; any other is.
    let overwrite = settings.overwrite || replace == Some(named.as_path());
    let path = PathReserver::default().reserve(dir, &stem, kind.extension(), overwrite);
    let file = place(temp, kind, &path, card, face, source).await?;

    let replaced = match replace {
        Some(old) if old != path => match tokio::fs::remove_file(old).await {
            Ok(()) => Some(old.to_string_lossy().into_owned()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        },
        Some(old) => Some(old.to_string_lossy().into_owned()),
        None => None,
    };
    Ok((file, replaced))
}

/// Why `downloaded` isn't a larger copy of the face's art, if it isn't.
async fn doubt(http: &Http, target: &Target, downloaded: &Path) -> Result<Option<Doubt>> {
    let crop = ArtCrop::fetch(http, target, downloaded.parent().unwrap_or(Path::new("."))).await?;
    crop.doubt(downloaded).await
}

/// Scryfall's art crop of a face, downloaded to check images against;
/// deleted when dropped.
struct ArtCrop(PathBuf);

impl ArtCrop {
    async fn fetch(http: &Http, target: &Target, dir: &Path) -> Result<Self> {
        let crop = Self(dir.join(format!(".art-crop-{}", TEMP_IDS.fetch_add(1, Ordering::Relaxed))));
        if http.download(target.art_crop(), &crop.0).await?.is_none() {
            return Err(Error::Other(format!(
                "Scryfall's art crop of {} is gone",
                target.face.name
            )));
        }
        Ok(crop)
    }

    /// Why `candidate` isn't a larger copy of the art, if it isn't.
    async fn doubt(&self, candidate: &Path) -> Result<Option<Doubt>> {
        let (crop, candidate) = (self.0.clone(), candidate.to_owned());
        let verdict = tokio::task::spawn_blocking(move || verify::compare(&crop, &candidate))
            .await
            .map_err(|error| Error::Other(error.to_string()))??;
        Ok(match verdict {
            Verdict::Same { .. } => None,
            Verdict::Smaller => Some(Doubt::Smaller),
            Verdict::CardScan => Some(Doubt::CardScan),
            Verdict::Different => Some(Doubt::Different),
        })
    }
}

impl Drop for ArtCrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The most images of a page offered to choose from.
pub const MAX_PAGE_IMAGES: usize = 24;

/// How many of a page's images are downloaded at once.
const PAGE_DOWNLOADS: usize = 4;

/// An image on the page the user has open, as the page gives it: its address,
/// and its size in pixels when the page knows it (`0` when it doesn't).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PageImage {
    pub url: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

/// One of a page's images, downloaded for the user to choose from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCandidate {
    pub image_url: String,
    /// Where it was downloaded, named with its type's extension. Saving it
    /// is [`import`] with `force`, since the user saw what the check said.
    pub downloaded: PathBuf,
    /// Its size in pixels, read from the file.
    pub size: ImageSize,
    /// Why it isn't a larger copy of the art; `None` when it is.
    pub doubt: Option<Doubt>,
}

/// Downloads the `images` of a page into `dir` and checks each against
/// `target`'s art crop, for the user to choose from: a larger copy of the art
/// first, then near misses (a card scan, a copy no larger), then the rest,
/// each largest first. One that won't download, isn't a JPEG, PNG or WebP,
/// or can't be read is left out.
pub async fn fetch_page_images(
    http: &Http,
    target: &Target,
    images: &[PageImage],
    dir: &Path,
) -> Result<Vec<PageCandidate>> {
    let wanted = page_candidates(images);
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    tokio::fs::create_dir_all(dir).await?;
    let crop = ArtCrop::fetch(http, target, dir).await?;
    let crop = &crop;
    let mut found: Vec<PageCandidate> = stream::iter(wanted)
        .map(|image| async move { fetch_candidate(http, crop, image, dir).await })
        .buffer_unordered(PAGE_DOWNLOADS)
        .filter_map(|candidate| async move { candidate })
        .collect()
        .await;
    found.sort_by_key(|candidate| {
        let rank = match candidate.doubt {
            None => 0,
            Some(Doubt::Smaller | Doubt::CardScan) => 1,
            Some(Doubt::Different) => 2,
        };
        let area = u64::from(candidate.size.width) * u64::from(candidate.size.height);
        (rank, Reverse(area))
    });
    Ok(found)
}

async fn fetch_candidate(http: &Http, crop: &ArtCrop, image: PageImage, dir: &Path) -> Option<PageCandidate> {
    let part = dir.join(format!("page-{}.part", TEMP_IDS.fetch_add(1, Ordering::Relaxed)));
    let Ok(Some(kind)) = http.download(&image.url, &part).await else {
        let _ = tokio::fs::remove_file(&part).await;
        return None;
    };
    // Named for its type, so the app's window can show it.
    let downloaded = part.with_extension(kind.extension());
    if tokio::fs::rename(&part, &downloaded).await.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
        return None;
    }
    match (image_size(&downloaded, kind), crop.doubt(&downloaded).await) {
        (Some(size), Ok(doubt)) => Some(PageCandidate {
            image_url: image.url,
            downloaded,
            size,
            doubt,
        }),
        _ => {
            let _ = tokio::fs::remove_file(&downloaded).await;
            None
        }
    }
}

/// The images worth downloading: web addresses, each once, none known to be
/// too small to be a larger copy of an art crop, largest first, at most
/// [`MAX_PAGE_IMAGES`]. Ones whose size the page didn't give come last.
fn page_candidates(images: &[PageImage]) -> Vec<PageImage> {
    // Scryfall's art crops are 626 pixels wide; a copy has to be wider still.
    const TOO_NARROW: u32 = 640;
    let mut seen = HashSet::new();
    let mut candidates: Vec<PageImage> = images
        .iter()
        .filter(|image| image.url.starts_with("https://") || image.url.starts_with("http://"))
        .filter(|image| image.width == 0 || image.width >= TOO_NARROW)
        .filter(|image| seen.insert(image.url.clone()))
        .cloned()
        .collect();
    candidates.sort_by_key(|image| match image.width {
        0 => (1, 0),
        width => (0, u64::MAX - u64::from(width) * u64::from(image.height.max(1))),
    });
    candidates.truncate(MAX_PAGE_IMAGES);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tries_a_pages_large_images_first() {
        let image = |url: &str, width, height| PageImage {
            url: url.into(),
            width,
            height,
        };
        let images = [
            image("https://cdn.example/avatar.jpg", 150, 150),
            image("https://cdn.example/post-1080.jpg", 1080, 1350),
            image("data:image/png;base64,AAAA", 2000, 2000),
            image("https://cdn.example/og.jpg", 0, 0),
            image("https://cdn.example/post-1440.jpg", 1440, 1800),
            image("https://cdn.example/post-1440.jpg", 1440, 1800),
        ];
        let urls: Vec<_> = page_candidates(&images).into_iter().map(|i| i.url).collect();
        assert_eq!(
            urls,
            [
                "https://cdn.example/post-1440.jpg",
                "https://cdn.example/post-1080.jpg",
                "https://cdn.example/og.jpg",
            ]
        );
    }

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
        let url = |site| target.search_url(site).unwrap().to_string();
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
        assert_eq!(
            url(Site::Instagram),
            "https://www.google.com/search?q=%22Seb+McKinnon%22+site%3Ainstagram.com"
        );
        assert_eq!(target.search_url(Site::Website), None);
    }

    #[test]
    fn keeps_an_artists_name_in_one_piece() {
        // A slash or an ampersand stays part of the name.
        let target = target("Jesper Ejsing & Ron Spencer/RK");
        assert_eq!(
            target.search_url(Site::Pixiv).unwrap().as_str(),
            "https://www.pixiv.net/tags/Jesper%20Ejsing%20&%20Ron%20Spencer%2FRK"
        );
        assert_eq!(
            target.search_url(Site::Artstation).unwrap().as_str(),
            "https://www.artstation.com/search/artists?query=Jesper+Ejsing+%26+Ron+Spencer%2FRK"
        );
    }
}
