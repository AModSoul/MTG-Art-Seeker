import type { IconButtonProps } from "@chakra-ui/react";

/** The app header's height, which dialogs and drawers open below (`theme.ts`) so it stays in
 * reach: Card Anvil's, its heading's 30px line and 2px above and below. */
export const HEADER_HEIGHT = "2.125rem";

const HEADER_ID = "app-header";

/** The props that mark the app's header. */
export const headerProps = { id: HEADER_ID };

/** Keeps the app's header working while a dialog or drawer is open, for that overlay's
 * `persistentElements`: a press on it, to drag, minimize or close the window or to use one of
 * its buttons, is then not one outside the overlay, which would close it, and the overlay
 * doesn't turn off its clicks as it does the rest of the page's. The app's own `DialogRoot` and
 * `DrawerRoot` (`components/ui`) pass it to every overlay, as a native title bar is never part
 * of the page.
 *
 * The overlay waits a second for each such element to appear, and then fails with an error
 * nothing catches. Where there's no header, as in a dialog's own story, a detached element
 * stands in, which keeps nothing. */
export const keepHeader = [
  () => document.getElementById(HEADER_ID) ?? document.createElement("header"),
];

/** The header's icon buttons: small, so the bar stays slim, with 16px icons, as Card Anvil's. */
export const HEADER_BUTTON = {
  variant: "ghost",
  size: "2xs",
  _icon: { boxSize: "4" },
} satisfies Pick<IconButtonProps, "variant" | "size" | "_icon">;
