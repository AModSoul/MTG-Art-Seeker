//! End-to-end runs against mock Scryfall, MTGPics and image servers.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use mtg_art_core::{
    carddb::CardData,
    decks::Decks,
    engine::{run, ArtSource, JobStatus, Local, RunEvent, RunInput, RunSummary},
    history::{FoundByHand, History},
    http::{Endpoints, Http, HttpConfig},
    known, manual,
    scryfall::{search, SearchOptions, Unique},
    settings::Settings,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use wiremock::{
    matchers::{body_partial_json, header, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0];
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

struct Servers {
    scryfall: MockServer,
    mtgpics: MockServer,
    cdn: MockServer,
}

impl Servers {
    async fn start() -> Self {
        Self {
            scryfall: MockServer::start().await,
            mtgpics: MockServer::start().await,
            cdn: MockServer::start().await,
        }
    }

    fn http(&self) -> Http {
        Http::new(HttpConfig {
            endpoints: Endpoints {
                scryfall_api: self.scryfall.uri(),
                mtgpics: self.mtgpics.uri(),
                edhrec: self.cdn.uri(),
                mtgjson: self.cdn.uri(),
                box_images: self.cdn.uri(),
            },
            max_attempts: 3,
            retry_delay: Duration::from_millis(5),
            rate_limited_wait: Duration::from_millis(5),
            mtgpics_spacing: Duration::from_millis(1),
        })
        .unwrap()
    }

    /// A card from the fixtures, its Scryfall image links pointed at the mock CDN.
    fn card(&self, name_prefix: &str) -> Value {
        let fixture: Value = serde_json::from_str(include_str!("fixtures/collection.json")).unwrap();
        let card = fixture["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"].as_str().unwrap().starts_with(name_prefix))
            .unwrap();
        serde_json::from_str(&card.to_string().replace("https://cards.scryfall.io", &self.cdn.uri())).unwrap()
    }

    async fn mtgpics_page(&self, card_ref: &str, html: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path("/card"))
            .and(query_param("ref", card_ref))
            .respond_with(ResponseTemplate::new(200).set_body_raw(html, "text/html; charset=ISO-8859-1"))
            .mount(&self.mtgpics)
            .await;
    }

    async fn image(server: &MockServer, at: &str, body: &[u8]) {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body.to_vec(), "image/jpeg"))
            .mount(server)
            .await;
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn settings(out: &Path) -> Settings {
    Settings {
        output_dir: out.to_string_lossy().into_owned(),
        naming: "NAME [SET] {NUMBER}".into(),
        ..Settings::default()
    }
}

/// Runs to the end, returning the summary and every job's final status by id.
async fn run_to_end(http: &Http, input: RunInput, settings: &Settings) -> (RunSummary, Vec<(u32, JobStatus)>) {
    let events = Mutex::new(Vec::new());
    let emit = |event: RunEvent| events.lock().unwrap().push(event);
    let summary = run(
        http,
        input,
        settings,
        Local::default(),
        &CancellationToken::new(),
        &emit,
    )
    .await;
    let events = events.into_inner().unwrap();
    assert!(matches!(events.last(), Some(RunEvent::Finished { .. })));
    let mut finals: Vec<(u32, JobStatus)> = Vec::new();
    for event in events {
        if let RunEvent::Job { id, status } = event {
            finals.retain(|(seen, _)| *seen != id);
            finals.push((id, status));
        }
    }
    finals.sort_by_key(|(id, _)| *id);
    (summary, finals)
}

fn relative(file: &str, out: &Path) -> String {
    Path::new(file)
        .strip_prefix(out)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

#[tokio::test]
async fn downloads_a_list_with_mtgpics_first_and_scryfall_for_what_it_lacks() {
    let servers = Servers::start().await;
    let out = tempfile::tempdir().unwrap();

    // Two cards the list gives sets for are looked up in one batch; the third
    // isn't on Scryfall at all.
    Mock::given(method("POST"))
        .and(path("/cards/collection"))
        .and(body_partial_json(json!({ "identifiers": [
            { "set": "dom", "collector_number": "1" },
            { "set": "znr", "collector_number": "90" },
            { "name": "Nonexistent Card", "set": "dom" },
        ]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [servers.card("Karn"), servers.card("Agadeem")],
            "not_found": [{ "name": "Nonexistent Card", "set": "dom" }],
        })))
        .expect(1)
        .mount(&servers.scryfall)
        .await;
    Mock::given(method("GET"))
        .and(path("/cards/named"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "object": "error" })))
        .mount(&servers.scryfall)
        .await;

    servers.mtgpics_page("dom001", fixture("card_dom001.html")).await;
    servers.mtgpics_page("znr090", fixture("card_znr090.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/001_1.jpg", JPEG).await;
    Servers::image(&servers.mtgpics, "/pics/art/znr/090.jpg", JPEG).await;
    // The back face's MTGPics image is gone, so it comes from Scryfall.
    Servers::image(
        &servers.cdn,
        "/art_crop/back/0/f/0f29a5f8-cd4c-4c6e-9a31-b8b1c1288a9e.jpg",
        JPEG,
    )
    .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(JPEG.to_vec(), "image/jpeg"))
        .mount(&servers.cdn)
        .await;

    let text = "4 Karn, Scion of Urza (DOM) 1 *F*\nSideboard\nAgadeem's Awakening (ZNR) 90\n1 Nonexistent Card (dom)\n";
    let (summary, finals) = run_to_end(
        &servers.http(),
        RunInput::List {
            text: text.into(),
            folder: None,
        },
        &settings(out.path()),
    )
    .await;

    assert_eq!((summary.jobs, summary.done, summary.failed), (3, 2, 1));
    assert_eq!((summary.mtgpics_files, summary.scryfall_files), (2, 1));

    let JobStatus::Done { files, missing } = &finals[0].1 else {
        panic!("{:?}", finals[0])
    };
    assert!(missing.is_empty());
    assert_eq!(files[0].source, ArtSource::Mtgpics);
    assert_eq!(
        relative(&files[0].path, out.path()),
        "mtgpics/Karn, Scion of Urza [DOM] {1}.jpg"
    );

    let JobStatus::Done { files, .. } = &finals[1].1 else {
        panic!("{:?}", finals[1])
    };
    let saved: Vec<_> = files
        .iter()
        .map(|f| (f.source, relative(&f.path, out.path())))
        .collect();
    assert_eq!(
        saved,
        [
            (
                ArtSource::Mtgpics,
                "mtgpics/MDFC Front/Agadeem's Awakening [ZNR] {90}.jpg".to_owned()
            ),
            (
                ArtSource::Scryfall,
                "scryfall/MDFC Back/Agadeem, the Undercrypt [ZNR] {90}.jpg".to_owned()
            ),
        ]
    );
    for file in files {
        assert_eq!(std::fs::read(&file.path).unwrap(), JPEG);
    }

    assert_eq!(
        finals[2].1,
        JobStatus::Failed {
            error: "Not found on Scryfall".into()
        }
    );
    // Nothing half-downloaded is left behind.
    let leftovers = walk(out.path()).into_iter().filter(|p| p.ends_with(".part")).count();
    assert_eq!(leftovers, 0);
}

#[tokio::test]
async fn finds_cards_mtgpics_numbers_differently_through_one_checklist_read() {
    let servers = Servers::start().await;
    let out = tempfile::tempdir().unwrap();

    // Two cards whose collector numbers aren't MTGPics' refs for them.
    let mut karn = servers.card("Karn");
    karn["collector_number"] = json!("901");
    let mut will = karn.clone();
    will["name"] = json!("Adamant Will");
    will["collector_number"] = json!("902");
    will["artist"] = json!("Alex Konstad");
    Mock::given(method("GET"))
        .and(path("/cards/search"))
        .and(query_param("q", "set:dom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [karn, will], "has_more": false, "total_cards": 2,
        })))
        .mount(&servers.scryfall)
        .await;

    // dom901 and dom902 don't exist; dom001 is Karn's page, which links its set.
    servers.mtgpics_page("dom001", fixture("card_dom001.html")).await;
    servers
        .mtgpics_page(
            "dom002",
            b"<title>Adamant Will - mtgpics.com</title><img src=pics/art_th/dom/002.jpg>".to_vec(),
        )
        .await;
    Mock::given(method("GET"))
        .and(path("/card"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"No card found.".to_vec(), "text/html"))
        .mount(&servers.mtgpics)
        .await;
    Mock::given(method("GET"))
        .and(path("/set_checklist"))
        .and(query_param("set", "288"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture("checklist_288.html"), "text/html; charset=ISO-8859-1"),
        )
        .expect(1)
        .mount(&servers.mtgpics)
        .await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/001_1.jpg", JPEG).await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/002.jpg", PNG).await;

    let (summary, finals) = run_to_end(
        &servers.http(),
        RunInput::Search {
            query: "set:dom".into(),
        },
        &settings(out.path()),
    )
    .await;

    assert_eq!((summary.done, summary.mtgpics_files), (2, 2), "{finals:?}");
    let mut names: Vec<String> = finals
        .iter()
        .flat_map(|(_, status)| match status {
            JobStatus::Done { files, .. } => files.iter().map(|f| relative(&f.path, out.path())).collect::<Vec<_>>(),
            other => panic!("{other:?}"),
        })
        .collect();
    names.sort();
    // The PNG is saved as a PNG, whatever the URL said.
    assert_eq!(
        names,
        [
            "mtgpics/Adamant Will [DOM] {902}.png",
            "mtgpics/Karn, Scion of Urza [DOM] {901}.jpg"
        ]
    );
}

#[tokio::test]
async fn retries_a_rate_limited_request_and_follows_pages() {
    let servers = Servers::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/search"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&servers.scryfall)
        .await;
    let next = format!("{}/cards/search?page=2", servers.scryfall.uri());
    Mock::given(method("GET"))
        .and(path("/cards/search"))
        .and(query_param("q", "t:goblin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [servers.card("Karn")], "has_more": true, "next_page": next, "total_cards": 2,
        })))
        .mount(&servers.scryfall)
        .await;
    Mock::given(method("GET"))
        .and(path("/cards/search"))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [servers.card("Swamp")], "has_more": false,
        })))
        .mount(&servers.scryfall)
        .await;

    let options = SearchOptions {
        unique: Unique::Art,
        include_extras: false,
        by_release: None,
    };
    let results = search(&servers.http(), "t:goblin", &options, 100).await.unwrap();
    let names: Vec<_> = results.cards.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Karn, Scion of Urza", "Swamp"]);
    assert_eq!(results.total, 2);
}

#[tokio::test]
async fn reports_a_bad_search_with_scryfalls_explanation() {
    let servers = Servers::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/search"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "object": "error", "details": "All of your terms were ignored.",
        })))
        .mount(&servers.scryfall)
        .await;
    let out = tempfile::tempdir().unwrap();
    let events = Mutex::new(Vec::new());
    let emit = |event: RunEvent| events.lock().unwrap().push(event);
    let input = RunInput::Search { query: "zzz:".into() };
    run(
        &servers.http(),
        input,
        &settings(out.path()),
        Local::default(),
        &CancellationToken::new(),
        &emit,
    )
    .await;
    let events = events.into_inner().unwrap();
    assert!(
        matches!(&events[0], RunEvent::Error { message } if message.starts_with("All of your terms were ignored.")),
        "{events:?}"
    );
}

#[tokio::test]
async fn a_cancelled_run_cancels_its_jobs() {
    let servers = Servers::start().await;
    // Every lookup hangs, so the run is still working when it's cancelled.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
        .mount(&servers.scryfall)
        .await;
    let out = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let events = Mutex::new(Vec::new());
    let emit = |event: RunEvent| events.lock().unwrap().push(event);
    let input = RunInput::List {
        text: "Damnation\nSwamp\nIsland".into(),
        folder: None,
    };
    let canceller = async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();
    };
    let (http, settings) = (servers.http(), settings(out.path()));
    let (summary, ()) = tokio::join!(
        run(&http, input, &settings, Local::default(), &cancel, &emit),
        canceller
    );
    assert_eq!((summary.jobs, summary.cancelled), (3, 3));
}

#[tokio::test]
async fn saves_a_list_in_its_own_folder() {
    let servers = Servers::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/collection"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [servers.card("Karn")],
            "not_found": [],
        })))
        .mount(&servers.scryfall)
        .await;
    servers.mtgpics_page("dom001", fixture("card_dom001.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/001_1.jpg", JPEG).await;

    let out = tempfile::tempdir().unwrap();
    let settings = Settings {
        list_folders: true,
        ..settings(out.path())
    };
    let events = Mutex::new(Vec::new());
    let emit = |event: RunEvent| events.lock().unwrap().push(event);
    let input = RunInput::List {
        text: "Karn, Scion of Urza (DOM) 1".into(),
        // Made safe as a folder name: no slashes, so no folders above it.
        folder: Some("../Karn: Superfriends".into()),
    };
    run(
        &servers.http(),
        input,
        &settings,
        Local::default(),
        &CancellationToken::new(),
        &emit,
    )
    .await;

    let folder = out.path().join("..Karn Superfriends");
    let events = events.into_inner().unwrap();
    assert!(
        matches!(&events[0], RunEvent::Planned { folder: planned, .. } if Path::new(planned) == folder),
        "{events:?}"
    );
    let saved: Vec<_> = walk(out.path()).iter().map(|file| relative(file, out.path())).collect();
    assert_eq!(saved, ["..Karn Superfriends/mtgpics/Karn, Scion of Urza [DOM] {1}.jpg"]);
}

#[tokio::test]
async fn a_list_folder_needs_the_setting_and_a_name() {
    let out = Path::new("out");
    let list = |folder: &str| RunInput::List {
        text: String::new(),
        folder: Some(folder.into()),
    };
    let on = Settings {
        list_folders: true,
        ..settings(out)
    };
    let planned_folder = |input: RunInput, settings: Settings| async move {
        let events = Mutex::new(Vec::new());
        let emit = |event: RunEvent| events.lock().unwrap().push(event);
        let http = Servers::start().await.http();
        run(
            &http,
            input,
            &settings,
            Local::default(),
            &CancellationToken::new(),
            &emit,
        )
        .await;
        match events.into_inner().unwrap().remove(0) {
            RunEvent::Planned { folder, .. } => PathBuf::from(folder),
            event => panic!("{event:?}"),
        }
    };
    assert_eq!(planned_folder(list("Deck"), on.clone()).await, out.join("Deck"));
    assert_eq!(planned_folder(list("Deck"), settings(out)).await, out);
    assert_eq!(planned_folder(list("   "), on).await, out);
}

#[tokio::test]
async fn saves_art_from_where_it_was_found_before() {
    let servers = Servers::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/collection"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [servers.card("Karn")],
            "not_found": [],
        })))
        .mount(&servers.scryfall)
        .await;
    // Karn's MTGPics page is read once, on the first run; its art is saved on both.
    Mock::given(method("GET"))
        .and(path("/card"))
        .and(query_param("ref", "dom001"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture("card_dom001.html"), "text/html; charset=ISO-8859-1"),
        )
        .expect(1)
        .mount(&servers.mtgpics)
        .await;
    Mock::given(method("GET"))
        .and(path("/pics/art/dom/001_1.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(JPEG.to_vec(), "image/jpeg"))
        .expect(2)
        .mount(&servers.mtgpics)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let local = Local {
        cards: None,
        history: Some(&history),
    };
    let out = tempfile::tempdir().unwrap();
    let list = || RunInput::List {
        text: "Karn, Scion of Urza (DOM) 1".into(),
        folder: None,
    };
    for _ in 0..2 {
        let summary = run(
            &servers.http(),
            list(),
            &settings(out.path()),
            local,
            &CancellationToken::new(),
            &|_| {},
        )
        .await;
        assert_eq!((summary.done, summary.mtgpics_files), (1, 1));
    }

    let karn: mtg_art_core::card::Card = serde_json::from_value(servers.card("Karn")).unwrap();
    let remembered = history.remembered(&karn, 1).unwrap().unwrap();
    assert_eq!(
        remembered.mtgpics,
        [Some(format!("{}/pics/art/dom/001_1.jpg", servers.mtgpics.uri()))]
    );
}

#[tokio::test]
async fn finds_art_again_when_it_has_moved() {
    let servers = Servers::start().await;
    let karn_json = servers.card("Karn");
    Mock::given(method("POST"))
        .and(path("/cards/collection"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [karn_json],
            "not_found": [],
        })))
        .mount(&servers.scryfall)
        .await;
    servers.mtgpics_page("dom001", fixture("card_dom001.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/001_1.jpg", JPEG).await;

    // The log has Karn's art somewhere MTGPics no longer has it.
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let karn: mtg_art_core::card::Card = serde_json::from_value(karn_json).unwrap();
    let moved = format!("{}/pics/art/dom/old.jpg", servers.mtgpics.uri());
    history
        .remember(&karn, &[Some(moved)], std::time::SystemTime::now())
        .unwrap();

    let out = tempfile::tempdir().unwrap();
    let local = Local {
        cards: None,
        history: Some(&history),
    };
    let input = RunInput::List {
        text: "Karn, Scion of Urza (DOM) 1".into(),
        folder: None,
    };
    let summary = run(
        &servers.http(),
        input,
        &settings(out.path()),
        local,
        &CancellationToken::new(),
        &|_| {},
    )
    .await;
    assert_eq!((summary.done, summary.mtgpics_files), (1, 1));
    assert_eq!(
        history.remembered(&karn, 1).unwrap().unwrap().mtgpics,
        [Some(format!("{}/pics/art/dom/001_1.jpg", servers.mtgpics.uri()))]
    );
}

/// Real cards from Scryfall, gzipped as its bulk data files are, with their
/// image links pointed at `cdn`.
fn bulk_file(cdn: &str) -> Vec<u8> {
    let jsonl = String::from_utf8(fixture("default_cards.jsonl"))
        .unwrap()
        .replace("https://cards.scryfall.io", cdn);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    std::io::Write::write_all(&mut gz, jsonl.as_bytes()).unwrap();
    gz.finish().unwrap()
}

/// Mounts Scryfall's bulk data list and file, as published at `updated_at`.
async fn publish_bulk_data(servers: &Servers, updated_at: &str) {
    Mock::given(method("GET"))
        .and(path("/bulk-data/default_cards"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "bulk_data",
            "type": "default_cards",
            "updated_at": updated_at,
            "jsonl_download_uri": format!("{}/file/default-cards.jsonl.gz", servers.scryfall.uri()),
        })))
        .mount(&servers.scryfall)
        .await;
    Mock::given(method("GET"))
        .and(path("/file/default-cards.jsonl.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(bulk_file(&servers.cdn.uri()), "application/gzip"))
        .expect(1)
        .mount(&servers.scryfall)
        .await;
}

#[tokio::test]
async fn downloads_card_data_once_per_published_file() {
    let servers = Servers::start().await;
    publish_bulk_data(&servers, "2026-11-07T10:06:55.230+00:00").await;
    let dir = tempfile::tempdir().unwrap();
    let http = servers.http();

    let data = CardData::open(dir.path().to_owned());
    assert!(data.current().is_none());
    assert!(data.refresh(&http).await.unwrap());
    assert_eq!(data.status().cards, 27);
    // The same file again isn't downloaded again — by this process or the next.
    assert!(!data.refresh(&http).await.unwrap());
    let next = CardData::open(dir.path().to_owned());
    assert!(!next.refresh(&http).await.unwrap());
    assert_eq!(next.current().unwrap().updated_at(), "2026-11-07T10:06:55.230+00:00");
}

#[tokio::test]
async fn looks_a_list_up_in_the_card_data_without_asking_scryfall() {
    let servers = Servers::start().await;
    publish_bulk_data(&servers, "2026-11-07T10:06:55.230+00:00").await;
    let dir = tempfile::tempdir().unwrap();
    let http = servers.http();
    let data = CardData::open(dir.path().to_owned());
    data.refresh(&http).await.unwrap();

    // Scryfall's lookups aren't asked: every card is in the data.
    for at in ["/cards/collection", "/cards/search", "/cards/named"] {
        Mock::given(path(at))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&servers.scryfall)
            .await;
    }
    servers.mtgpics_page("dom001", fixture("card_dom001.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/001_1.jpg", JPEG).await;
    // Nothing else is on MTGPics, so the rest comes from Scryfall's images.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(JPEG.to_vec(), "image/jpeg"))
        .mount(&servers.cdn)
        .await;

    let out = tempfile::tempdir().unwrap();
    let settings = Settings {
        layout_folders: false,
        ..settings(out.path())
    };
    let text =
        "Karn, Scion of Urza (DOM) 1\nBlightsteel Colossus\n1 Agadeem's Awakening [znr] {90}\nLightning Bolt (lea)";
    let cards = data.current().unwrap();
    let events = Mutex::new(Vec::new());
    let emit = |event: RunEvent| events.lock().unwrap().push(event);
    let input = RunInput::List {
        text: text.into(),
        folder: None,
    };
    let summary = run(
        &http,
        input,
        &settings,
        Local {
            cards: Some(&cards),
            history: None,
        },
        &CancellationToken::new(),
        &emit,
    )
    .await;
    assert_eq!(
        (summary.jobs, summary.done, summary.failed),
        (4, 4, 0),
        "{:?}",
        events.lock().unwrap()
    );

    let mut saved: Vec<_> = walk(out.path()).iter().map(|file| relative(file, out.path())).collect();
    saved.sort();
    assert_eq!(
        saved,
        [
            "mtgpics/Karn, Scion of Urza [DOM] {1}.jpg",
            "scryfall/Agadeem's Awakening [ZNR] {90}.jpg",
            "scryfall/Agadeem, the Undercrypt [ZNR] {90}.jpg",
            // The newest printing of a name.
            "scryfall/Blightsteel Colossus [SLD] {2223}.jpg",
            "scryfall/Lightning Bolt [LEA] {161}.jpg",
        ]
    );

    // Each job says how many images it will save: a card found in the data
    // as soon as the jobs are announced, and a name alone once it's looked up.
    let events = events.into_inner().unwrap();
    let Some(RunEvent::Planned { jobs, .. }) = events.iter().find(|event| matches!(event, RunEvent::Planned { .. }))
    else {
        panic!("no planned event: {events:?}")
    };
    let planned: Vec<_> = jobs.iter().map(|job| job.images).collect();
    assert_eq!(planned, [Some(1), None, Some(2), Some(1)]);
    let mut downloading: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            RunEvent::Job {
                id,
                status: JobStatus::Downloading { images },
            } => Some((*id, *images)),
            _ => None,
        })
        .collect();
    downloading.sort_unstable();
    assert_eq!(downloading, [(1, 1), (2, 1), (3, 2), (4, 1)]);
}

fn walk(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path.to_string_lossy().into_owned());
        }
    }
    found
}

// --- Art the user found themselves ------------------------------------------

/// A made-up painting: overlapping ellipses of random sizes and colours, as
/// `verify`'s own tests draw them. Tests never use real art.
fn painting(width: u32, height: u32, seed: u32) -> image::DynamicImage {
    let mut state = u64::from(seed).wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut random = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as f32 / (1u64 << 31) as f32
    };
    let mut image = image::RgbImage::from_pixel(width, height, image::Rgb([70, 80, 90]));
    for _ in 0..60 {
        let (cx, cy) = (random() * width as f32, random() * height as f32);
        let (rx, ry) = (
            (0.02 + random() * 0.15) * width as f32,
            (0.02 + random() * 0.15) * height as f32,
        );
        let colour = image::Rgb([random(), random(), random()].map(|c| (c * 255.0) as u8));
        let span = |c: f32, r: f32, max: u32| ((c - r).max(0.0) as u32)..((c + r).ceil() as u32).min(max);
        for y in span(cy, ry, height) {
            for x in span(cx, rx, width) {
                let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    image.put_pixel(x, y, colour);
                }
            }
        }
    }
    image::DynamicImage::ImageRgb8(image)
}

fn jpeg(image: &image::DynamicImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .to_rgb8()
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .unwrap();
    bytes
}

/// Karn's art, uncropped, with Scryfall's art crop of it on the mock CDN;
/// and Karn on the mock Scryfall.
async fn karn_with_art(servers: &Servers) -> image::DynamicImage {
    let art = painting(1200, 900, 1);
    let crop =
        art.crop_imm(120, 100, 960, 960 * 457 / 626)
            .resize_exact(626, 457, image::imageops::FilterType::Triangle);
    Servers::image(
        &servers.cdn,
        "/art_crop/front/0/7/07a3d9e8-8597-498b-869c-cff79e0df516.jpg",
        &jpeg(&crop),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/cards/collection"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [servers.card("Karn")],
            "not_found": [],
        })))
        .mount(&servers.scryfall)
        .await;
    art
}

const KARN_ART: &str = "792b699f-87ea-4608-aac5-e6d5e96feacb";

#[tokio::test]
async fn saves_a_larger_copy_the_user_found_in_place_of_scryfalls() {
    let servers = Servers::start().await;
    let art = karn_with_art(&servers).await;
    let http = servers.http();
    let out = tempfile::tempdir().unwrap();
    let settings = settings(out.path());
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();

    // What the run saved, and what the user saved in the Lens window.
    let scryfall_file = out.path().join("scryfall/Karn, Scion of Urza [DOM] {1}.jpg");
    std::fs::create_dir_all(scryfall_file.parent().unwrap()).unwrap();
    std::fs::write(&scryfall_file, JPEG).unwrap();
    let downloaded = dir.path().join("lens-0.part");
    std::fs::write(&downloaded, jpeg(&art)).unwrap();

    let target = manual::prepare(&http, None, &settings, "DOM", "1", 0, None)
        .await
        .unwrap();
    assert_eq!(target.face.name, "Karn, Scion of Urza");
    let found = manual::Found {
        image_url: "https://cdna.artstation.com/p/assets/images/karn/4k/karn.jpg",
        page: Some("https://www.artstation.com/artwork/karn"),
        site: Some(manual::Site::Artstation),
    };
    let destination = manual::Destination {
        settings: &settings,
        list_folder: None,
        replace: Some(&scryfall_file),
    };
    let imported = manual::import(&http, Some(&history), &target, destination, &downloaded, found, false)
        .await
        .unwrap();

    let manual::Imported::Saved { file, replaced } = imported else {
        panic!("not saved: {imported:?}");
    };
    assert_eq!(
        relative(&file.path, out.path()),
        "web/Karn, Scion of Urza [DOM] {1}.jpg"
    );
    assert_eq!(file.source, ArtSource::Web);
    assert_eq!(file.page.as_deref(), found.page);
    assert_eq!(file.size.map(|s| (s.width, s.height)), Some((1200, 900)));
    assert_eq!(replaced.as_deref(), Some(scryfall_file.to_string_lossy().as_ref()));
    assert!(!scryfall_file.exists(), "Scryfall's file is replaced");
    assert!(downloaded.exists(), "the download is left for the caller");
    assert_eq!(
        history.found_by_hand(KARN_ART).unwrap().map(|f| f.url),
        Some(found.image_url.to_owned())
    );
}

#[tokio::test]
async fn asks_before_saving_an_image_that_is_not_the_art() {
    let servers = Servers::start().await;
    karn_with_art(&servers).await;
    let http = servers.http();
    let out = tempfile::tempdir().unwrap();
    let settings = settings(out.path());
    let dir = tempfile::tempdir().unwrap();
    let downloaded = dir.path().join("lens-0.part");
    std::fs::write(&downloaded, jpeg(&painting(1200, 900, 2))).unwrap();
    let target = manual::prepare(&http, None, &settings, "dom", "1", 0, None)
        .await
        .unwrap();
    let found = manual::Found {
        image_url: "https://example.com/other.jpg",
        page: None,
        site: None,
    };
    let destination = manual::Destination {
        settings: &settings,
        list_folder: None,
        replace: None,
    };

    let imported = manual::import(&http, None, &target, destination, &downloaded, found, false).await;
    assert_eq!(imported.unwrap(), manual::Imported::Doubtful(manual::Doubt::Different));
    assert!(!out.path().join("web").exists(), "nothing is saved");

    // The user says to save it anyway.
    let imported = manual::import(&http, None, &target, destination, &downloaded, found, true).await;
    assert!(matches!(
        imported.unwrap(),
        manual::Imported::Saved { replaced: None, .. }
    ));
    assert_eq!(walk(out.path()).len(), 1);

    // Not an image at all.
    std::fs::write(&downloaded, b"<html>").unwrap();
    let imported = manual::import(&http, None, &target, destination, &downloaded, found, true).await;
    assert!(imported.is_err());
}

#[tokio::test]
async fn later_runs_save_the_users_copy_from_where_they_found_it() {
    let servers = Servers::start().await;
    let art = karn_with_art(&servers).await;
    // Asked for as the page it was on asks for it: some sites refuse a
    // request for an image that doesn't come from one of their pages.
    Mock::given(method("GET"))
        .and(path("/found/karn.jpg"))
        .and(header("referer", "https://example.com/karn"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(jpeg(&art), "image/jpeg"))
        .mount(&servers.cdn)
        .await;
    // MTGPics isn't asked about a face the user found art for.
    Mock::given(method("GET"))
        .and(path("/card"))
        .respond_with(ResponseTemplate::new(404))
        .expect(0)
        .mount(&servers.mtgpics)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let found = FoundByHand {
        url: format!("{}/found/karn.jpg", servers.cdn.uri()),
        page: Some("https://example.com/karn".into()),
    };
    history.remember_found_by_hand(KARN_ART, &found).unwrap();

    let out = tempfile::tempdir().unwrap();
    let events = Mutex::new(Vec::new());
    let summary = run(
        &servers.http(),
        RunInput::List {
            text: "Karn, Scion of Urza (DOM) 1".into(),
            folder: None,
        },
        &settings(out.path()),
        Local {
            cards: None,
            history: Some(&history),
        },
        &CancellationToken::new(),
        &|event| events.lock().unwrap().push(event),
    )
    .await;
    assert_eq!((summary.done, summary.web_files), (1, 1));
    let files: Vec<_> = events
        .into_inner()
        .unwrap()
        .into_iter()
        .filter_map(|event| match event {
            RunEvent::Job {
                status: JobStatus::Done { files, .. },
                ..
            } => Some(files),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(
        relative(&files[0].path, out.path()),
        "web/Karn, Scion of Urza [DOM] {1}.jpg"
    );
    assert_eq!(files[0].page, found.page);
}

#[tokio::test]
async fn forgets_the_users_copy_once_it_is_gone() {
    let servers = Servers::start().await;
    karn_with_art(&servers).await;
    servers.mtgpics_page("dom001", fixture("card_dom001.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/001_1.jpg", JPEG).await;
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let gone = FoundByHand {
        url: format!("{}/gone.jpg", servers.cdn.uri()),
        page: None,
    };
    history.remember_found_by_hand(KARN_ART, &gone).unwrap();

    let out = tempfile::tempdir().unwrap();
    let summary = run(
        &servers.http(),
        RunInput::List {
            text: "Karn, Scion of Urza (DOM) 1".into(),
            folder: None,
        },
        &settings(out.path()),
        Local {
            cards: None,
            history: Some(&history),
        },
        &CancellationToken::new(),
        &|_| {},
    )
    .await;
    assert_eq!((summary.done, summary.web_files, summary.mtgpics_files), (1, 0, 1));
    assert_eq!(history.found_by_hand(KARN_ART).unwrap(), None);
}

/// Runs Karn once with `history`, returning the summary.
async fn run_karn(servers: &Servers, history: &History, out: &Path) -> RunSummary {
    run(
        &servers.http(),
        RunInput::List {
            text: "Karn, Scion of Urza (DOM) 1".into(),
            folder: None,
        },
        &settings(out),
        Local {
            cards: None,
            history: Some(history),
        },
        &CancellationToken::new(),
        &|_| {},
    )
    .await
}

#[tokio::test]
async fn checks_the_users_copy_again_before_saving_it() {
    let servers = Servers::start().await;
    karn_with_art(&servers).await;
    servers.mtgpics_page("dom001", fixture("card_dom001.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/dom/001_1.jpg", JPEG).await;
    // Something else is at the copy's address now.
    Servers::image(&servers.cdn, "/found/karn.jpg", &jpeg(&painting(1400, 1050, 9))).await;
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let url = format!("{}/found/karn.jpg", servers.cdn.uri());
    let found = FoundByHand {
        url: url.clone(),
        page: None,
    };
    history.remember_found_by_hand(KARN_ART, &found).unwrap();

    let out = tempfile::tempdir().unwrap();
    let summary = run_karn(&servers, &history, out.path()).await;
    assert_eq!((summary.done, summary.web_files, summary.mtgpics_files), (1, 0, 1));
    assert_eq!(history.found_by_hand(KARN_ART).unwrap(), None, "it's forgotten");

    // But an image the user saved though the check doubted it is theirs to
    // keep, while it's the size it was when they saved it.
    history.remember_found_by_hand(KARN_ART, &found).unwrap();
    let copy = |width, height| mtg_art_core::history::FoundCopy {
        url: url.clone(),
        page: None,
        site: None,
        size: Some(mtg_art_core::imagesize::ImageSize { width, height }),
    };
    history.remember_found_copy(KARN_ART, &copy(1400, 1050)).unwrap();
    let out = tempfile::tempdir().unwrap();
    let summary = run_karn(&servers, &history, out.path()).await;
    assert_eq!((summary.done, summary.web_files), (1, 1));
    assert!(history.found_by_hand(KARN_ART).unwrap().is_some());
}

#[tokio::test]
async fn keeps_only_a_copy_that_can_be_downloaded_again() {
    let servers = Servers::start().await;
    let art = karn_with_art(&servers).await;
    let http = servers.http();
    let out = tempfile::tempdir().unwrap();
    let settings = settings(out.path());
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let downloaded = dir.path().join("blob.part");
    std::fs::write(&downloaded, jpeg(&art)).unwrap();
    let target = manual::prepare(&http, None, &settings, "dom", "1", 0, None)
        .await
        .unwrap();
    let found = manual::Found {
        image_url: "blob:https://www.instagram.com/3f1c",
        page: Some("https://www.instagram.com/p/karn/"),
        site: Some(manual::Site::Instagram),
    };
    let destination = manual::Destination {
        settings: &settings,
        list_folder: None,
        replace: None,
    };
    let imported = manual::import(&http, Some(&history), &target, destination, &downloaded, found, false)
        .await
        .unwrap();
    assert!(matches!(imported, manual::Imported::Saved { .. }), "saved this once");
    assert_eq!(history.found_by_hand(KARN_ART).unwrap(), None);
    assert!(history.found_copies(KARN_ART).unwrap().is_empty());
}

/// A card by `artist`, to look for a larger copy of.
fn target_by(artist: &str) -> manual::Target {
    let card: mtg_art_core::card::Card = serde_json::from_value(json!({
        "name": "Twilight Prophet", "set": "rix", "collector_number": "88", "artist": artist,
        "image_uris": { "art_crop": "https://cards.scryfall.io/art_crop/front/x.jpg" }
    }))
    .unwrap();
    let face = mtg_art_core::faces::plan_faces(&card, &Settings::default()).remove(0);
    manual::Target { card, face }
}

#[tokio::test]
async fn opens_the_website_mtgpics_links_for_the_artist() {
    let servers = Servers::start().await;
    // The list of artists is read once, and each artist's page once: after
    // that, the history answers.
    Mock::given(method("GET"))
        .and(path("/illustrators"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture("illustrators.html"), "text/html; charset=ISO-8859-1"),
        )
        .expect(1)
        .mount(&servers.mtgpics)
        .await;
    for id in ["729", "841"] {
        Mock::given(method("GET"))
            .and(path("/illus"))
            .and(query_param("art", id))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(fixture(&format!("illus_{id}.html")), "text/html; charset=ISO-8859-1"),
            )
            .expect(1)
            .mount(&servers.mtgpics)
            .await;
    }
    let http = servers.http();
    let artists = mtg_art_core::mtgpics::Artists::default();
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let website = manual::Site::Website;

    for _ in 0..2 {
        let url = manual::site_url(&http, Some(&history), &artists, &target_by("Seb McKinnon"), website)
            .await
            .unwrap();
        assert_eq!(url.as_str(), "http://www.sebmckinnon.com/");
    }
    for _ in 0..2 {
        let error = manual::site_url(&http, Some(&history), &artists, &target_by("Dmitry Burmak"), website)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "MTGPics doesn't list a website for Dmitry Burmak.");
    }
    // The other sites are searches, with nothing to look up.
    let url = manual::site_url(
        &http,
        None,
        &artists,
        &target_by("Dmitry Burmak"),
        manual::Site::Artstation,
    )
    .await
    .unwrap();
    assert_eq!(
        url.as_str(),
        "https://www.artstation.com/search/artists?query=Dmitry+Burmak"
    );
}

#[tokio::test]
async fn asks_for_mtgpics_list_of_artists_again_when_it_had_no_one_on_it() {
    let servers = Servers::start().await;
    // A page with no artists on it the first time, the list the next.
    Mock::given(method("GET"))
        .and(path("/illustrators"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>Down for maintenance</html>"))
        .up_to_n_times(1)
        .mount(&servers.mtgpics)
        .await;
    Mock::given(method("GET"))
        .and(path("/illustrators"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture("illustrators.html"), "text/html; charset=ISO-8859-1"),
        )
        .mount(&servers.mtgpics)
        .await;
    Mock::given(method("GET"))
        .and(path("/illus"))
        .and(query_param("art", "729"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture("illus_729.html"), "text/html; charset=ISO-8859-1"),
        )
        .mount(&servers.mtgpics)
        .await;
    let http = servers.http();
    let artists = mtg_art_core::mtgpics::Artists::default();
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let target = target_by("Seb McKinnon");
    let website = manual::Site::Website;

    assert!(manual::site_url(&http, Some(&history), &artists, &target, website)
        .await
        .is_err());
    // Nothing was noted of the artist, so the next try asks MTGPics.
    assert_eq!(history.artist_website("Seb McKinnon").unwrap(), None);
    let url = manual::site_url(&http, Some(&history), &artists, &target, website)
        .await
        .unwrap();
    assert_eq!(url.as_str(), "http://www.sebmckinnon.com/");
}

#[tokio::test]
async fn offers_the_images_of_a_page_art_first() {
    let servers = Servers::start().await;
    let art = karn_with_art(&servers).await;
    // The page's images: an avatar, another painting, a card scan of the art,
    // and the art itself.
    let crop =
        art.crop_imm(120, 100, 960, 960 * 457 / 626)
            .resize_exact(626, 457, image::imageops::FilterType::Triangle);
    let mut card = image::RgbImage::from_pixel(745, 1040, image::Rgb([30, 30, 30]));
    image::imageops::overlay(&mut card, &crop.to_rgb8(), 60, 110);
    Servers::image(&servers.cdn, "/avatar.jpg", &jpeg(&painting(150, 150, 3))).await;
    Servers::image(&servers.cdn, "/other.jpg", &jpeg(&painting(900, 675, 4))).await;
    Servers::image(&servers.cdn, "/scan.jpg", &jpeg(&image::DynamicImage::ImageRgb8(card))).await;
    Servers::image(&servers.cdn, "/art.jpg", &jpeg(&art)).await;
    let image = |name: &str, width, height| manual::PageImage {
        url: format!("{}/{name}", servers.cdn.uri()),
        width,
        height,
    };

    let http = servers.http();
    let out = tempfile::tempdir().unwrap();
    let settings = settings(out.path());
    let dir = tempfile::tempdir().unwrap();
    let target = manual::prepare(&http, None, &settings, "dom", "1", 0, None)
        .await
        .unwrap();
    let images = vec![
        image("avatar.jpg", 150, 150),
        image("scan.jpg", 745, 1040),
        image("other.jpg", 900, 675),
        image("missing.jpg", 2000, 1500),
        image("art.jpg", 1200, 900),
    ];
    let found = manual::fetch_page_images(&http, &target, &images, None, dir.path())
        .await
        .unwrap();

    // The avatar is too small to try and the missing image is left out; the
    // art comes first, then the card scan, then the other painting.
    let seen: Vec<_> = found
        .iter()
        .map(|c| (c.image_url.rsplit('/').next().unwrap(), c.size.width, c.doubt))
        .collect();
    assert_eq!(
        seen,
        [
            ("art.jpg", 1200, None),
            ("scan.jpg", 745, Some(manual::Doubt::CardScan)),
            ("other.jpg", 900, Some(manual::Doubt::Different)),
        ]
    );
    assert!(found.iter().all(|c| c.downloaded.exists()));
    assert_eq!(found[0].downloaded.extension().unwrap(), "jpg");

    // The one the user chooses is saved as the art.
    let destination = manual::Destination {
        settings: &settings,
        list_folder: None,
        replace: None,
    };
    let page = Some("https://www.instagram.com/p/karn/");
    let from = manual::Found {
        image_url: &found[0].image_url,
        page,
        site: Some(manual::Site::Instagram),
    };
    let imported = manual::import(&http, None, &target, destination, &found[0].downloaded, from, true)
        .await
        .unwrap();
    let manual::Imported::Saved { file, .. } = imported else {
        panic!("the art wasn't saved");
    };
    assert_eq!(
        relative(&file.path, out.path()),
        "web/Karn, Scion of Urza [DOM] {1}.jpg"
    );
    assert_eq!(file.page.as_deref(), page);
    assert_eq!(file.size.map(|s| s.width), Some(1200));

    // A page with nothing large enough has nothing to choose from.
    let none = manual::fetch_page_images(&http, &target, &[image("avatar.jpg", 150, 150)], None, dir.path())
        .await
        .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn goes_back_to_mtgpics_art_and_to_a_copy_the_user_found() {
    let servers = Servers::start().await;
    let art = karn_with_art(&servers).await;
    Servers::image(
        &servers.mtgpics,
        "/pics/art/dom/001_1.jpg",
        &jpeg(&painting(1600, 1200, 5)),
    )
    .await;
    Servers::image(&servers.cdn, "/found/karn.jpg", &jpeg(&art)).await;
    let http = servers.http();
    let out = tempfile::tempdir().unwrap();
    let settings = settings(out.path());
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let target = manual::prepare(&http, None, &settings, "DOM", "1", 0, None)
        .await
        .unwrap();
    let scryfall = target.face.scryfall_url.clone().unwrap();
    // Scryfall's image is the art crop karn_with_art serves.

    // MTGPics had Karn's art, and the user found two larger copies, the
    // ArtStation one saved last.
    let mtgpics = format!("{}/pics/art/dom/001_1.jpg", servers.mtgpics.uri());
    history
        .remember(&target.card, &[Some(mtgpics)], std::time::SystemTime::now())
        .unwrap();
    let artstation = format!("{}/found/karn.jpg", servers.cdn.uri());
    let copies = [
        ("https://cdn.example/lens.jpg", None, manual::Site::Lens, 1000),
        (
            artstation.as_str(),
            Some("https://www.artstation.com/artwork/karn"),
            manual::Site::Artstation,
            1200,
        ),
    ];
    for (url, page, site, width) in copies {
        let copy = mtg_art_core::history::FoundCopy {
            url: url.into(),
            page: page.map(Into::into),
            site: Some(site.key().into()),
            size: Some(mtg_art_core::imagesize::ImageSize { width, height: 900 }),
        };
        history.remember_found_copy(KARN_ART, &copy).unwrap();
    }
    let in_use = mtg_art_core::history::FoundByHand {
        url: artstation.clone(),
        page: Some("https://www.artstation.com/artwork/karn".into()),
    };
    history.remember_found_by_hand(KARN_ART, &in_use).unwrap();
    let web_file = out.path().join("web/Karn, Scion of Urza [DOM] {1}.jpg");
    std::fs::create_dir_all(web_file.parent().unwrap()).unwrap();
    std::fs::write(&web_file, jpeg(&art)).unwrap();

    let saved_now = mtg_art_core::engine::SavedFile {
        path: web_file.to_string_lossy().into_owned(),
        face: target.face.name.clone(),
        face_index: 0,
        source: ArtSource::Web,
        set: "DOM".into(),
        number: "1".into(),
        artist: target.face.artist.clone(),
        size: Some(mtg_art_core::imagesize::ImageSize {
            width: 1200,
            height: 900,
        }),
        page: None,
        split_half: false,
    };

    let known = known::known_sources(&http, Some(&history), &target, &settings, &saved_now).await;
    let seen: Vec<_> = known
        .iter()
        .map(|k| {
            let size = k.size.map(|size| (size.width, size.height));
            (k.source, k.site, k.host.as_deref(), size, k.current)
        })
        .collect();
    // MTGPics' and Scryfall's sizes are read from the start of their images.
    assert_eq!(
        seen,
        [
            (ArtSource::Mtgpics, None, None, Some((1600, 1200)), false),
            (ArtSource::Scryfall, None, None, Some((626, 457)), false),
            (
                ArtSource::Web,
                Some(manual::Site::Artstation),
                Some("artstation.com"),
                Some((1200, 900)),
                true
            ),
            (
                ArtSource::Web,
                Some(manual::Site::Lens),
                Some("cdn.example"),
                Some((1000, 900)),
                false
            ),
        ]
    );
    // And noted, so they aren't read again.
    assert_eq!(history.image_size(&scryfall).unwrap().map(|size| size.width), Some(626));
    let artstation_copy = known[2].copy;
    // Without the log, only Scryfall is known.
    let without = known::known_sources(&http, None, &target, &settings, &saved_now).await;
    assert_eq!(without.len(), 1);

    // Back to MTGPics: the found copy's file goes, and later runs stop saving it.
    let destination = |replace| manual::Destination {
        settings: &settings,
        list_folder: None,
        replace,
    };
    let (file, replaced) = known::save_from(
        &http,
        Some(&history),
        &target,
        destination(Some(&web_file)),
        ArtSource::Mtgpics,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        relative(&file.path, out.path()),
        "mtgpics/Karn, Scion of Urza [DOM] {1}.jpg"
    );
    assert_eq!((file.source, file.page.as_deref()), (ArtSource::Mtgpics, None));
    assert_eq!(replaced.as_deref(), Some(web_file.to_string_lossy().as_ref()));
    assert!(!web_file.exists());
    assert_eq!(history.found_by_hand(KARN_ART).unwrap(), None);
    // Every found copy is still offered, none of them saved now.
    let known = known::known_sources(&http, Some(&history), &target, &settings, &file).await;
    assert_eq!(known.len(), 4);
    assert!(known[0].current && !known[2].current);

    // And back to the ArtStation copy, downloaded again from where it was.
    let mtgpics_file = std::path::PathBuf::from(&file.path);
    let (file, _) = known::save_from(
        &http,
        Some(&history),
        &target,
        destination(Some(&mtgpics_file)),
        ArtSource::Web,
        artstation_copy,
    )
    .await
    .unwrap();
    assert_eq!(
        relative(&file.path, out.path()),
        "web/Karn, Scion of Urza [DOM] {1}.jpg"
    );
    assert_eq!(file.page.as_deref(), Some("https://www.artstation.com/artwork/karn"));
    assert_eq!(file.size.map(|s| s.width), Some(1200));
    assert!(!mtgpics_file.exists());
    assert_eq!(history.found_by_hand(KARN_ART).unwrap(), Some(in_use));
}

/// A card as Scryfall has it, saved as fixture `name`, its image links
/// pointed at the mock CDN, which serves `crop` as its art crop.
async fn scryfall_card(servers: &Servers, name: &str, crop: &[u8]) -> Value {
    let json = String::from_utf8(fixture(name)).unwrap();
    let card: Value = serde_json::from_str(&json.replace("https://cards.scryfall.io", &servers.cdn.uri())).unwrap();
    let at = card["image_uris"]["art_crop"].as_str().unwrap();
    Servers::image(&servers.cdn, reqwest::Url::parse(at).unwrap().path(), crop).await;
    Mock::given(method("POST"))
        .and(path("/cards/collection"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [card], "not_found": [] })))
        .mount(&servers.scryfall)
        .await;
    card
}

#[tokio::test]
async fn saves_a_split_cards_halves_from_mtgpics_or_the_whole_card_from_scryfall() {
    let servers = Servers::start().await;
    // Fire // Ice from Modern Horizons 2: a split card, with a painting per half.
    scryfall_card(&servers, "card_mh2_290.json", JPEG).await;
    let input = || RunInput::List {
        text: "Fire // Ice (MH2) 290".into(),
        folder: None,
    };
    let saved_in = |out: &Path| -> Vec<String> { walk(out).iter().map(|file| relative(file, out)).collect() };

    // MTGPics has neither half: the whole card, as Scryfall has it.
    let out = tempfile::tempdir().unwrap();
    let (summary, _) = run_to_end(&servers.http(), input(), &settings(out.path())).await;
    assert_eq!((summary.done, summary.scryfall_files), (1, 1));
    assert_eq!(saved_in(out.path()), ["scryfall/Split/Fire - Ice [MH2] {290}.jpg"]);

    // MTGPics has a painting for each half, Fire's first: one image each, as
    // a two-faced card has.
    servers.mtgpics_page("mh2290", fixture("card_mh2290.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/ivg/032.jpg", JPEG).await;
    Servers::image(&servers.mtgpics, "/pics/art/ivg/091.jpg", PNG).await;
    let out = tempfile::tempdir().unwrap();
    let (summary, jobs) = run_to_end(&servers.http(), input(), &settings(out.path())).await;
    assert_eq!((summary.done, summary.mtgpics_files), (1, 2));
    assert_eq!(
        saved_in(out.path()),
        [
            "mtgpics/Split/Fire [MH2] {290}.jpg",
            "mtgpics/Split/Ice [MH2] {290}.png"
        ]
    );
    let JobStatus::Done { files, missing } = &jobs[0].1 else {
        panic!("{jobs:?}");
    };
    let faces: Vec<_> = files
        .iter()
        .map(|f| (f.face_index, f.face.as_str(), f.split_half))
        .collect();
    assert_eq!(faces, [(0, "Fire", true), (1, "Ice", true)]);
    assert!(missing.is_empty());
}

#[tokio::test]
async fn later_runs_save_a_copy_the_user_found_of_a_split_card_saved_whole() {
    let servers = Servers::start().await;
    // Fire // Ice, saved whole from Scryfall, and a larger copy of the whole
    // card the user found for it.
    let whole = painting(1600, 700, 31);
    let crop =
        whole
            .crop_imm(160, 70, 1280, 1280 * 280 / 808)
            .resize_exact(808, 280, image::imageops::FilterType::Triangle);
    scryfall_card(&servers, "card_mh2_290.json", &jpeg(&crop)).await;
    Servers::image(&servers.cdn, "/found/fire-ice.jpg", &jpeg(&whole)).await;
    // MTGPics isn't asked about the halves.
    Mock::given(method("GET"))
        .and(path("/card"))
        .respond_with(ResponseTemplate::new(404))
        .expect(0)
        .mount(&servers.mtgpics)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.sqlite")).unwrap();
    let found = FoundByHand {
        url: format!("{}/found/fire-ice.jpg", servers.cdn.uri()),
        page: None,
    };
    history
        .remember_found_by_hand("511174a2-18dc-4582-9de1-c04871eb77de", &found)
        .unwrap();

    let out = tempfile::tempdir().unwrap();
    let summary = run(
        &servers.http(),
        RunInput::List {
            text: "Fire // Ice (MH2) 290".into(),
            folder: None,
        },
        &settings(out.path()),
        Local {
            cards: None,
            history: Some(&history),
        },
        &CancellationToken::new(),
        &|_| {},
    )
    .await;
    assert_eq!((summary.done, summary.web_files), (1, 1));
    let saved: Vec<_> = walk(out.path()).iter().map(|file| relative(file, out.path())).collect();
    assert_eq!(saved, ["web/Split/Fire - Ice [MH2] {290}.jpg"]);
}

#[tokio::test]
async fn saves_a_split_card_whole_when_mtgpics_files_one_image_under_both_halves() {
    let servers = Servers::start().await;
    // Bottomless Pool // Locker Room, a Room: MTGPics lists `dsk/043` and
    // `dsk/1009` for its halves, and both are the whole card's image.
    scryfall_card(&servers, "card_dsk_43.json", JPEG).await;
    servers.mtgpics_page("dsk043", fixture("card_dsk043.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/dsk/043.jpg", JPEG).await;
    Servers::image(&servers.mtgpics, "/pics/art/dsk/1009.jpg", JPEG).await;

    let out = tempfile::tempdir().unwrap();
    let input = RunInput::List {
        text: "Bottomless Pool // Locker Room (DSK) 43".into(),
        folder: None,
    };
    let (summary, jobs) = run_to_end(&servers.http(), input, &settings(out.path())).await;
    assert_eq!((summary.done, summary.mtgpics_files), (1, 1));
    let saved: Vec<_> = walk(out.path()).iter().map(|file| relative(file, out.path())).collect();
    assert_eq!(saved, ["mtgpics/Split/Bottomless Pool - Locker Room [DSK] {43}.jpg"]);
    let JobStatus::Done { files, missing } = &jobs[0].1 else {
        panic!("{jobs:?}");
    };
    // One picture of the whole card: not a half.
    assert_eq!(
        (files[0].face.as_str(), files[0].face_index, files[0].split_half),
        ("Bottomless Pool // Locker Room", 0, false)
    );
    assert!(missing.is_empty());
}

#[tokio::test]
async fn puts_a_split_cards_paintings_under_the_halves_they_belong_to() {
    let servers = Servers::start().await;
    // Fire // Ice from Dominaria Remastered: its MTGPics page lists Ice's
    // painting (`apo/129`) before Fire's (`apo/128`). Scryfall's art crop
    // shows Fire on the left and Ice on the right.
    let (fire, ice) = (painting(1000, 680, 21), painting(960, 652, 22));
    let part = |art: &image::DynamicImage| {
        let (w, h) = (art.width(), art.height());
        art.crop_imm(w / 10, h / 10, w * 8 / 10, h * 8 / 10)
            .resize_exact(404, 280, image::imageops::FilterType::Triangle)
            .to_rgb8()
    };
    let mut crop = image::RgbImage::new(808, 280);
    image::imageops::overlay(&mut crop, &part(&fire), 0, 0);
    image::imageops::overlay(&mut crop, &part(&ice), 404, 0);
    scryfall_card(
        &servers,
        "card_dmr_215.json",
        &jpeg(&image::DynamicImage::ImageRgb8(crop)),
    )
    .await;
    servers.mtgpics_page("dmr215", fixture("card_dmr215.html")).await;
    Servers::image(&servers.mtgpics, "/pics/art/apo/129.jpg", &jpeg(&ice)).await;
    Servers::image(&servers.mtgpics, "/pics/art/apo/128.jpg", &jpeg(&fire)).await;

    let out = tempfile::tempdir().unwrap();
    let input = RunInput::List {
        text: "Fire // Ice (DMR) 215".into(),
        folder: None,
    };
    let (_, jobs) = run_to_end(&servers.http(), input, &settings(out.path())).await;
    let JobStatus::Done { files, missing } = &jobs[0].1 else {
        panic!("{jobs:?}");
    };
    assert!(missing.is_empty());
    // Each half's own painting, under its own name and artist.
    let saved: Vec<_> = files
        .iter()
        .map(|f| {
            let width = image::image_dimensions(&f.path).unwrap().0;
            (f.face.as_str(), f.artist.as_str(), width, relative(&f.path, out.path()))
        })
        .collect();
    assert_eq!(
        saved,
        [
            (
                "Fire",
                "David Martin",
                1000,
                "mtgpics/Split/Fire [DMR] {215}.jpg".to_owned()
            ),
            (
                "Ice",
                "Franz Vohwinkel",
                960,
                "mtgpics/Split/Ice [DMR] {215}.jpg".to_owned()
            ),
        ]
    );
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

async fn mtgjson_file(servers: &Servers, at: &str, fixture_name: &str) {
    Mock::given(method("GET"))
        .and(path(at))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(gzip(&fixture(fixture_name))))
        .mount(&servers.cdn)
        .await;
}

#[tokio::test]
async fn downloads_a_precon_decks_printings_into_a_folder_named_for_it() {
    let servers = Servers::start().await;
    mtgjson_file(
        &servers,
        "/decks/AbzanArmor_TDC.json.gz",
        "mtgjson_deck_AbzanArmor_TDC.json",
    )
    .await;
    let out = tempfile::tempdir().unwrap();
    let settings = Settings {
        list_folders: true,
        ..settings(out.path())
    };
    let events = Mutex::new(Vec::new());
    let emit = |event: RunEvent| events.lock().unwrap().push(event);
    let input = RunInput::Deck {
        file_name: "AbzanArmor_TDC".into(),
        folder: Some("Abzan Armor".into()),
    };
    run(
        &servers.http(),
        input,
        &settings,
        Local::default(),
        &CancellationToken::new(),
        &emit,
    )
    .await;

    let events = events.into_inner().unwrap();
    let RunEvent::Planned { jobs, folder, .. } = &events[0] else {
        panic!("{events:?}");
    };
    assert_eq!(Path::new(folder), out.path().join("Abzan Armor"));
    let lines: Vec<_> = jobs.iter().map(|job| job.line.as_str()).collect();
    assert_eq!(
        lines,
        [
            "Felothar the Steadfast (TDC) 4",
            "Betor, Ancestor's Voice (TDC) 1",
            "Protector of the Wastes (TDC) 14",
            "Reunion of the House (TDC) 15",
        ]
    );
    // Each printing is asked for by its set and number, not its name.
    let asked: Vec<Value> = servers
        .scryfall
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.url.path() == "/cards/collection")
        .flat_map(|request| {
            request.body_json::<Value>().unwrap()["identifiers"]
                .as_array()
                .unwrap()
                .clone()
        })
        .collect();
    assert_eq!(asked[0], json!({ "set": "tdc", "collector_number": "4" }));
    assert_eq!(asked.len(), 4);
}

#[tokio::test]
async fn refuses_a_deck_name_that_is_not_mtgjsons() {
    let servers = Servers::start().await;
    let out = tempfile::tempdir().unwrap();
    let input = RunInput::Deck {
        file_name: "../../SetList".into(),
        folder: None,
    };
    let (summary, jobs) = run_to_end(&servers.http(), input, &settings(out.path())).await;
    assert_eq!(summary.jobs, 0);
    assert!(jobs.is_empty());
    assert!(servers.cdn.received_requests().await.unwrap().is_empty());
}

/// A product photo: a dark box on a white backdrop.
fn product_photo() -> image::DynamicImage {
    let mut photo = image::RgbImage::from_pixel(300, 420, image::Rgb([255, 255, 255]));
    for y in 30..390 {
        for x in 40..260 {
            photo.put_pixel(x, y, image::Rgb([40, 30, 60]));
        }
    }
    image::DynamicImage::ImageRgb8(photo)
}

#[tokio::test]
async fn lists_precons_and_cuts_their_boxes_out_of_tcgplayers_photos() {
    let servers = Servers::start().await;
    mtgjson_file(&servers, "/SetList.json.gz", "mtgjson_set_list.json").await;
    mtgjson_file(&servers, "/DeckList.json.gz", "mtgjson_deck_list.json").await;
    Servers::image(
        &servers.cdn,
        "/product/619664_in_1000x1000.jpg",
        &jpeg(&product_photo()),
    )
    .await;
    let http = servers.http();
    let dir = tempfile::tempdir().unwrap();
    let decks = Decks::open(dir.path().to_owned());

    let found = decks.search(&http, "set:tdc abzan", 10).await.unwrap();
    assert_eq!(found.total, 1);
    assert_eq!(found.decks[0].set_name, "Tarkir: Dragonstorm Commander");
    assert!(found.decks[0].has_box);

    let saved = decks.box_image(&http, "AbzanArmor_TDC").await.unwrap().unwrap();
    let cut = image::open(&saved).unwrap().to_rgba8();
    // Cropped to the box, without the backdrop.
    assert_eq!(cut.dimensions(), (220, 360));
    assert!(cut.pixels().all(|pixel| pixel[3] > 0));
    // Asked for once.
    assert_eq!(decks.box_image(&http, "AbzanArmor_TDC").await.unwrap(), Some(saved));
    let photos = servers
        .cdn
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.url.path().starts_with("/product/"))
        .count();
    assert_eq!(photos, 1);

    // The catalog is kept: a second start reads it rather than MTGJSON.
    let again = Decks::open(dir.path().to_owned());
    let lists_before = servers.cdn.received_requests().await.unwrap().len();
    assert_eq!(again.search(&http, "", 100).await.unwrap().total, 12);
    assert_eq!(servers.cdn.received_requests().await.unwrap().len(), lists_before);
}
