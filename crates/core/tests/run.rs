//! End-to-end runs against mock Scryfall, MTGPics and image servers.

use std::{path::Path, sync::Mutex, time::Duration};

use mtg_art_core::{
    engine::{run, ArtSource, JobStatus, RunEvent, RunInput, RunSummary},
    http::{Endpoints, Http, HttpConfig},
    scryfall::{search, SearchOptions, Unique},
    settings::Settings,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use wiremock::{
    matchers::{body_partial_json, method, path, query_param},
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
    let summary = run(http, input, settings, &CancellationToken::new(), &emit).await;
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
        RunInput::List { text: text.into() },
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
    };
    let canceller = async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();
    };
    let (http, settings) = (servers.http(), settings(out.path()));
    let (summary, ()) = tokio::join!(run(&http, input, &settings, &cancel, &emit), canceller);
    assert_eq!((summary.jobs, summary.cancelled), (3, 3));
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
