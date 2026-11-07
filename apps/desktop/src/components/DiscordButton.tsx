import { IconButton } from "@chakra-ui/react";
import { RiDiscordFill } from "react-icons/ri";

import { api } from "@/api";
import { Tooltip } from "@mtg-art/ui/tooltip";
import { showError } from "@/notify";

async function openDiscord() {
  try {
    await api.openDiscord();
  } catch (error) {
    showError("Couldn't open Discord", error);
  }
}

/** Opens the Proxy Community Discord in the browser, as Card Anvil's dock does. */
export function DiscordButton() {
  return (
    <Tooltip content="Join the Proxy Community Discord">
      <IconButton
        variant="ghost"
        size="sm"
        aria-label="Join the Proxy Community Discord"
        onClick={() => void openDiscord()}
      >
        <RiDiscordFill />
      </IconButton>
    </Tooltip>
  );
}
