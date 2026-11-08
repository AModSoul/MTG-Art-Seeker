// The app's own window, whose title bar the app draws itself (see `AppHeader`): what frames
// it, and the calls the title bar's buttons make. Storybook and the screenshots stand in for
// this module, as they do for `api.ts`.

import { getCurrentWindow } from "@tauri-apps/api/window";

/** What frames the window: the app's own title bar on Windows and Linux, where the native one
 * is turned off; macOS's traffic lights over the header, which keeps the native one; or
 * nothing, in a browser. */
export type WindowFrame = "custom" | "mac" | "browser";

export function windowFrame(): WindowFrame {
  if (!("__TAURI_INTERNALS__" in window)) return "browser";
  // WKWebView, on macOS, says "Macintosh"; WebView2 and WebKitGTK don't.
  return /mac/i.test(navigator.userAgent) ? "mac" : "custom";
}

export const minimizeWindow = () => getCurrentWindow().minimize();
export const toggleMaximizeWindow = () => getCurrentWindow().toggleMaximize();
export const closeWindow = () => getCurrentWindow().close();
export const isWindowMaximized = () => getCurrentWindow().isMaximized();

/** Calls `handler` whenever the window is resized, by anything; resolves to a function that
 * stops it. */
export const onWindowResized = (handler: () => void) => getCurrentWindow().onResized(handler);
