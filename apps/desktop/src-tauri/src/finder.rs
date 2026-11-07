//! The window a user looks for a larger copy of a card's art in: Google Lens,
//! or an artist search on ArtStation, DeviantArt or Pixiv.
//!
//! The app opens a page the core chose and then only listens: it injects no
//! scripts and reads nothing from the pages. A person does the searching, and
//! when they save an image, the download is caught here and handed on. The
//! pages, and every site the user goes on to, get none of the app's commands:
//! the capabilities in `capabilities/` name only `main`.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
};

use tauri::{
    webview::{DownloadEvent, NewWindowResponse},
    AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub const LABEL: &str = "finder";

/// Numbers downloads for the whole session, so a download still waiting on
/// the user keeps its name when the window is closed and opened again.
static NEXT_DOWNLOAD: AtomicU64 = AtomicU64::new(0);

/// An image the user saved in the window.
#[derive(Debug, Clone)]
pub struct Saved {
    /// The image's address.
    pub image_url: Url,
    /// The page the user was on when they saved it.
    pub page: Option<Url>,
    /// Where it was saved: a file of its own in [`downloads_dir`].
    pub path: PathBuf,
}

/// Where the window's downloads go until they're dealt with.
fn downloads_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_cache_dir()?.join("finder-downloads"))
}

/// Deletes downloads an earlier session left behind.
pub fn clear_downloads(app: &AppHandle) {
    if let Ok(dir) = downloads_dir(app) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Opens the window on `url`, or moves the open one there.
/// `on_saved` gets every image the user saves in it.
pub fn open(
    app: &AppHandle,
    url: Url,
    title: &str,
    on_saved: impl Fn(&AppHandle, Saved) + Send + Sync + 'static,
) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.navigate(url)?;
        window.set_title(title)?;
        let _ = window.unminimize();
        window.set_focus()?;
        return Ok(window);
    }

    let downloads = downloads_dir(app)?;
    std::fs::create_dir_all(&downloads)?;
    // Where each download was sent, by its URL: macOS never says where a
    // finished download went.
    let pending: Mutex<HashMap<Url, PathBuf>> = Mutex::default();
    let opener = app.clone();
    let saver = app.clone();

    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title(title)
        .inner_size(1100.0, 800.0)
        // The sites' cookies are kept apart from the app's own storage.
        .data_directory(app.path().app_local_data_dir()?.join("finder-webview"))
        .on_navigation(|url| matches!(url.scheme(), "http" | "https"))
        // A result that opens in a new window opens here instead, so every
        // download goes through the one handler below.
        .on_new_window(move |url, _features| {
            if matches!(url.scheme(), "http" | "https") {
                if let Some(window) = opener.get_webview_window(LABEL) {
                    let _ = window.navigate(url);
                }
            }
            NewWindowResponse::Deny
        })
        .on_download(move |webview, event| {
            match event {
                DownloadEvent::Requested { url, destination } => {
                    let n = NEXT_DOWNLOAD.fetch_add(1, Ordering::Relaxed);
                    *destination = downloads.join(format!("{n}.part"));
                    lock(&pending).insert(url, destination.clone());
                }
                DownloadEvent::Finished { url, path, success } => {
                    let path = lock(&pending).remove(&url).or(path);
                    match (success, path) {
                        (true, Some(path)) => on_saved(
                            &saver,
                            Saved {
                                image_url: url,
                                page: webview.url().ok(),
                                path,
                            },
                        ),
                        (false, Some(path)) => {
                            let _ = std::fs::remove_file(path);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            true
        })
        .build()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
