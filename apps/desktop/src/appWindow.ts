// The app's own window, whose title bar the app draws itself (see `AppHeader`): what frames
// it, and the calls the title bar's buttons make. Storybook and the screenshots stand in for
// this module, as they do for `api.ts`.

import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** What frames the window: the app's own title bar on Windows and Linux, where the native one
 * is turned off; macOS's traffic lights over the header, which keeps the native one; or
 * nothing, in a browser. */
export type WindowFrame = "custom" | "mac" | "browser";

export function windowFrame(): WindowFrame {
  if (!isTauri()) return "browser";
  // Set by the Rust side as it opens the window (`open_main_window`), which knows the system
  // it was built for. Without it, the app's own buttons: a window with no way to close it is
  // worse than one with two.
  return Reflect.get(window, "__MTG_ART_WINDOW_FRAME__") === "mac" ? "mac" : "custom";
}

export const minimizeWindow = () => getCurrentWindow().minimize();
export const toggleMaximizeWindow = () => getCurrentWindow().toggleMaximize();
export const closeWindow = () => getCurrentWindow().close();
export const isWindowMaximized = () => getCurrentWindow().isMaximized();

/** Calls `handler` whenever the window is resized, by anything; resolves to a function that
 * stops it. */
export const onWindowResized = (handler: () => void) => getCurrentWindow().onResized(handler);
