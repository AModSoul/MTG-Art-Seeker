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
      {/* As small as the header's other buttons. */}
      <IconButton
        variant="ghost"
        size="2xs"
        aria-label="Join the Proxy Community Discord"
        _icon={{ boxSize: "4" }}
        onClick={() => void openDiscord()}
      >
        <RiDiscordFill />
      </IconButton>
    </Tooltip>
  );
}
