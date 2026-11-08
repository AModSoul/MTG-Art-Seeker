import { Button } from "@chakra-ui/react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useState } from "react";
import { LuDownload } from "react-icons/lu";

import { showError } from "@/notify";

/** Checks GitHub for a newer release once at start-up, and offers to install it.
 * Development builds don't check. */
export function UpdateButton() {
  const [update, setUpdate] = useState<Update | null>(null);

  useEffect(() => {
    if (!import.meta.env.PROD) return;
    let current = true;
    // Offline, or no release yet: say nothing.
    check().then(
      (found) => current && setUpdate(found),
      () => {},
    );
    return () => {
      current = false;
    };
  }, []);

  return update ? <UpdateOffer update={update} /> : null;
}

/** The parts of an update the button uses. */
export type AvailableUpdate = Pick<Update, "version" | "downloadAndInstall">;

/** Offers `update`, and installs it and restarts the app when clicked. */
export function UpdateOffer({ update }: { update: AvailableUpdate }) {
  const [installing, setInstalling] = useState(false);

  async function install() {
    setInstalling(true);
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (error) {
      setInstalling(false);
      showError("The update failed", error);
    }
  }

  return (
    // As small as the header's other buttons.
    <Button
      size="2xs"
      variant="subtle"
      loading={installing}
      loadingText="Updating…"
      onClick={() => void install()}
    >
      <LuDownload /> Update to {update.version}
    </Button>
  );
}
