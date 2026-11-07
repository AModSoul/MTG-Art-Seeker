import { Button } from "@chakra-ui/react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useState } from "react";
import { LuDownload } from "react-icons/lu";

import { errorText } from "@/api";
import { toaster } from "@/components/ui/toaster";

/** Checks GitHub for a newer release once at start-up, and offers to install it.
 * Development builds don't check. */
export function UpdateButton() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [installing, setInstalling] = useState(false);

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

  if (!update) return null;

  async function install() {
    if (!update) return;
    setInstalling(true);
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (error) {
      setInstalling(false);
      toaster.create({ type: "error", title: "The update failed", description: errorText(error) });
    }
  }

  return (
    <Button
      size="sm"
      colorPalette="green"
      variant="subtle"
      loading={installing}
      loadingText="Updating…"
      onClick={() => void install()}
    >
      <LuDownload /> Update to {update.version}
    </Button>
  );
}
