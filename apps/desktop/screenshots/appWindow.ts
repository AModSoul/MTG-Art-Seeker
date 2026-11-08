// Stands in for `src/appWindow.ts` while the screenshots are taken: the window as on Windows,
// with the app's own title bar, not maximized; its buttons do nothing.

import type { WindowFrame } from "../src/appWindow";

export const windowFrame = (): WindowFrame => "custom";
export const minimizeWindow = async () => {};
export const toggleMaximizeWindow = async () => {};
export const closeWindow = async () => {};
export const isWindowMaximized = async () => false;
export const onWindowResized = async (_handler: () => void) => () => {};
