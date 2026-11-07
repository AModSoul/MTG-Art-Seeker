import { basePreview, resetUi } from "@mtg-art/ui/storybook";
import type { Preview } from "@storybook/react-vite";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { mocked, sb } from "storybook/test";

import { cardData, presets } from "../src/__fixtures__/run.ts";
import { defaultSettings } from "../src/__fixtures__/settings.ts";
import { api } from "../src/api.ts";
import { useInput, useRun, useSettings } from "../src/store.ts";

// Storybook has no Tauri backend, so every module that reaches it is mocked.
// Each export becomes a spy; the defaults below make the app behave as if the
// Rust side answered, and a story overrides them with `mocked(api.x)` in its
// `beforeEach`.
sb.mock(import("../src/api.ts"));
sb.mock(import("@tauri-apps/api/core"));
sb.mock(import("@tauri-apps/plugin-dialog"));
sb.mock(import("@tauri-apps/plugin-process"));
sb.mock(import("@tauri-apps/plugin-updater"));

/** Stands in for saved art; tests never touch the network. */
const PLACEHOLDER_ART = `data:image/svg+xml,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 3"><rect width="4" height="3" fill="#7c6f64"/></svg>',
)}`;

const preview: Preview = {
  ...basePreview,
  beforeEach: () => {
    mocked(api.getSettings).mockResolvedValue(defaultSettings);
    mocked(api.saveSettings).mockResolvedValue();
    mocked(api.parseCardList).mockResolvedValue({ cards: [], skipped: [] });
    mocked(api.listPresets).mockResolvedValue(presets);
    mocked(api.countSearch).mockResolvedValue(0);
    mocked(api.readCardList).mockResolvedValue({ text: "", name: "" });
    mocked(api.cardDataStatus).mockResolvedValue(cardData);
    mocked(api.openDiscord).mockResolvedValue();
    mocked(api.cancelRun).mockResolvedValue();
    mocked(api.revealFile).mockResolvedValue();
    mocked(api.openFolder).mockResolvedValue();
    mocked(api.startRun).mockResolvedValue(1);
    // Saved art lives on disk, which Storybook can't read; show a local stand-in.
    mocked(convertFileSrc).mockReturnValue(PLACEHOLDER_ART);
    mocked(open).mockResolvedValue(null);
    mocked(check).mockResolvedValue(null);
    mocked(relaunch).mockResolvedValue();

    // Every story starts from the app's initial state, with settings loaded
    // and no toasts left over from the story before.
    resetUi();
    useInput.persist.clearStorage();
    useInput.setState(useInput.getInitialState(), true);
    useRun.setState(useRun.getInitialState(), true);
    useSettings.setState({ ...useSettings.getInitialState(), settings: defaultSettings }, true);
  },
};

export default preview;
