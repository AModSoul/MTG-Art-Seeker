import { ButtonGroup, IconButton } from "@chakra-ui/react";
import { useEffect, useState } from "react";
import { LuCopy, LuMinus, LuSquare, LuX } from "react-icons/lu";

import {
  closeWindow,
  isWindowMaximized,
  minimizeWindow,
  onWindowResized,
  toggleMaximizeWindow,
} from "@/appWindow";
import { showError } from "@/notify";

/** A click handler that does `action` to the window, and says so if it can't. */
function run(what: string, action: () => Promise<void>) {
  return () => {
    action().catch((error: unknown) => showError(`Couldn't ${what} the window`, error));
  };
}

/** The window's minimize, maximize or restore, and close buttons, at the end of the app's
 * header, for Windows and Linux, where the native title bar is turned off. As tall as the
 * header, square-cornered, as the system's own are. Their icons are the sizes Card Anvil's show:
 * 20px for minimize, 16px for the square and the cross. The sizes go on the buttons (`_icon`),
 * not on an `Icon` inside, whose size can lose to the button's own depending on the order the
 * styles are written in. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  // The maximize button shows restore while the window is maximized, however it got that way:
  // these buttons, a double click on the header, Win+Up or snapping it to the screen's edge.
  useEffect(() => {
    let current = true;
    let stop: (() => void) | undefined;
    const sync = () => {
      isWindowMaximized().then(
        (value) => current && setMaximized(value),
        () => {},
      );
    };
    sync();
    onWindowResized(sync).then(
      (unlisten) => {
        if (current) stop = unlisten;
        else unlisten();
      },
      () => {},
    );
    return () => {
      current = false;
      stop?.();
    };
  }, []);

  return (
    <ButtonGroup gap="0" alignSelf="stretch" colorPalette="gray" variant="ghost" size="xs">
      <IconButton
        h="full"
        rounded="none"
        aria-label="Minimize"
        _icon={{ boxSize: "5" }}
        onClick={run("minimize", minimizeWindow)}
      >
        <LuMinus />
      </IconButton>
      <IconButton
        h="full"
        rounded="none"
        aria-label={maximized ? "Restore" : "Maximize"}
        _icon={{ boxSize: "4" }}
        onClick={run(maximized ? "restore" : "maximize", toggleMaximizeWindow)}
      >
        {maximized ? <LuCopy /> : <LuSquare />}
      </IconButton>
      <IconButton
        h="full"
        rounded="none"
        aria-label="Close"
        _icon={{ boxSize: "4" }}
        // Like the others, and red only under the pointer, as the system's own.
        _hover={{ bg: "red.solid", color: "red.contrast" }}
        onClick={run("close", closeWindow)}
      >
        <LuX />
      </IconButton>
    </ButtonGroup>
  );
}
