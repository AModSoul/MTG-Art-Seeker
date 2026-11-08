import { Box, Heading, HStack, Icon, VisuallyHidden } from "@chakra-ui/react";
import { RiImageDownloadFill } from "react-icons/ri";

import { windowFrame } from "@/appWindow";
import { DiscordButton } from "@/components/DiscordButton";
import { SettingsDrawer } from "@/components/SettingsDrawer";
import { UpdateButton } from "@/components/UpdateButton";
import { HEADER_HEIGHT } from "@/layout";
import { WindowControls } from "@/components/WindowControls";
import { ColorModeButton } from "@mtg-art/ui/color-mode";

const APP_NAME = "MTG Art Seeker";

/** The app's title bar, as Card Anvil's: its icon and name, its buttons, and on Windows and
 * Linux the window's own, the native title bar being turned off. On macOS the native one
 * stays, with its traffic lights over the start of this bar, so the name is left out there
 * but for screen readers. */
export function AppHeader() {
  const frame = windowFrame();
  // This bar is all there is to drag the window by. Tauri starts a drag only from an element
  // that carries the attribute itself, so it goes on the bar's empty parts and the name, not
  // on the buttons.
  const drag = frame === "browser" ? {} : { "data-tauri-drag-region": true };

  return (
    <HStack
      as="header"
      gap="0"
      align="stretch"
      bg="bg.emphasized"
      shadow="sm"
      h={HEADER_HEIGHT}
      // Above drawers and dialogs and their backdrops, and still taking clicks while one is
      // open (a modal turns them off for the rest of the page), so the window can always be
      // dragged, minimized or closed. Drawers open below it.
      position="relative"
      zIndex="popover"
      pointerEvents="auto"
      {...drag}
    >
      <Heading
        as="h1"
        flex="1"
        display="flex"
        alignItems="center"
        px="4"
        py="0.5"
        // Card Anvil's: the default heading's line height, with a smaller font, sets the
        // bar's height.
        fontSize="md"
        {...drag}
      >
        {frame === "mac" ? (
          <VisuallyHidden>{APP_NAME}</VisuallyHidden>
        ) : (
          <HStack gap="2" {...drag}>
            <Icon size="lg" color="emerald.solid" pointerEvents="none" aria-hidden>
              <RiImageDownloadFill />
            </Icon>
            <Box as="span" {...drag}>
              {APP_NAME}
            </Box>
          </HStack>
        )}
      </Heading>
      {/* Small buttons, as Card Anvil's, so the bar stays slim. Each sets its own size: a group
          would hand its size to the settings drawer's buttons too, which render inside it. */}
      <HStack gap="2" pe="4">
        <UpdateButton />
        <DiscordButton />
        <ColorModeButton size="2xs" />
        <SettingsDrawer />
      </HStack>
      {frame === "custom" && <WindowControls />}
    </HStack>
  );
}
