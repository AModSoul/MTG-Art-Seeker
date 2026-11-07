//! The window a user looks for a larger copy of a card's art in: Google Lens,
//! or an artist search on ArtStation, DeviantArt or Pixiv.
//!
//! The app opens a page the core chose and then listens: a person does the
//! searching, and when they save an image, the download is caught here and
//! handed on. The app reads a page only when the user asks, from the window's
//! menu, to find the art on it, and then reads only the addresses and sizes
//! of its images ([`PAGE_IMAGES`]); it never clicks, scrolls or navigates for
//! them. The pages, and every site the user goes on to, get none of the
//! app's commands: the capabilities in `capabilities/` name only `main`.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
};

use mtg_art_core::manual::PageImage;
use tauri::{
    menu::{Menu, MenuItem},
    webview::{DownloadEvent, NewWindowResponse},
    AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub const LABEL: &str = "finder";

/// The window's menu items.
const FIND_ART: &str = "find-art";
const SIGN_OUT: &str = "sign-out";

/// Lists the images on the page: each `<img>`'s source and the largest in its
/// `srcset`, `<picture>` sources, and the page's `og:image`, with the sizes
/// the page knows. It reads nothing else and changes nothing, and returns an
/// empty list rather than throw, since an exception is lost on Windows.
const PAGE_IMAGES: &str = r#"(() => {
  try {
    const found = new Map();
    const add = (src, width, height) => {
      if (!src) return;
      let url;
      try { url = new URL(src, location.href).href; } catch (_) { return; }
      const known = found.get(url);
      if (!known || width * Math.max(height, 1) > known.width * Math.max(known.height, 1)) {
        found.set(url, { url, width, height });
      }
    };
    const largest = (srcset) => {
      let best = null;
      for (const part of (srcset || "").split(/,\s+/)) {
        const [src, size] = part.trim().split(/\s+/);
        const width = size && size.endsWith("w") ? parseInt(size, 10) : 0;
        if (src && (!best || width > best.width)) best = { src, width };
      }
      return best;
    };
    for (const img of document.images) {
      add(img.currentSrc || img.src, img.naturalWidth, img.naturalHeight);
      const best = largest(img.srcset);
      if (best) {
        const height = best.width && img.naturalWidth
          ? Math.round((best.width * img.naturalHeight) / img.naturalWidth)
          : 0;
        add(best.src, best.width, height);
      }
    }
    for (const source of document.querySelectorAll("picture source[srcset]")) {
      const best = largest(source.srcset);
      if (best) add(best.src, best.width, 0);
    }
    const og = document.querySelector('meta[property="og:image"]');
    if (og) add(og.content, 0, 0);
    return [...found.values()];
  } catch (_) {
    return [];
  }
})()"#;

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
pub fn downloads_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_cache_dir()?.join("finder-downloads"))
}

/// Deletes downloads an earlier session left behind.
pub fn clear_downloads(app: &AppHandle) {
    if let Ok(dir) = downloads_dir(app) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Opens the window on `url`, or moves the open one there. `on_saved` gets
/// every image the user saves in it; `on_page` gets the images of the page
/// they're on, with its address, when they ask to find the art on it.
pub fn open(
    app: &AppHandle,
    url: Url,
    title: &str,
    on_saved: impl Fn(&AppHandle, Saved) + Send + Sync + 'static,
    on_page: impl Fn(&AppHandle, Vec<PageImage>, Option<Url>) + Send + Sync + 'static,
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
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, FIND_ART, "Find the art on this page", true, None::<&str>)?,
            &MenuItem::with_id(app, SIGN_OUT, "Sign out of sites", true, None::<&str>)?,
        ],
    )?;
    let on_page = std::sync::Arc::new(on_page);

    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title(title)
        .inner_size(1100.0, 800.0)
        .menu(menu)
        .on_menu_event(move |window, event| {
            let Some(webview) = window.app_handle().get_webview_window(LABEL) else {
                return;
            };
            match event.id().as_ref() {
                FIND_ART => {
                    let (app, on_page) = (window.app_handle().clone(), on_page.clone());
                    let page = webview.url().ok();
                    let _ = webview.eval_with_callback(PAGE_IMAGES, move |json| {
                        let images = serde_json::from_str(&json).unwrap_or_default();
                        on_page(&app, images, page.clone());
                    });
                }
                SIGN_OUT => {
                    // Cookies and storage go; the page is loaded again without them.
                    let _ = webview.clear_all_browsing_data();
                    if let Ok(url) = webview.url() {
                        let _ = webview.navigate(url);
                    }
                }
                _ => {}
            }
        })
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
