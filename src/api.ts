// Typed wrappers over the app's Tauri commands (src-tauri/src/lib.rs). The types
// come from the Rust core through ts-rs — regenerate them with `npm run bindings`.

import { Channel, invoke } from "@tauri-apps/api/core";

import type { CardDataStatus } from "@/bindings/CardDataStatus";
import type { ListFile } from "@/bindings/ListFile";
import type { ParsedList } from "@/bindings/ParsedList";
import type { PresetInfo } from "@/bindings/PresetInfo";
import type { RunEvent } from "@/bindings/RunEvent";
import type { RunInput } from "@/bindings/RunInput";
import type { Settings } from "@/bindings/Settings";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  parseCardList: (text: string) => invoke<ParsedList>("parse_card_list", { text }),
  listPresets: () => invoke<PresetInfo[]>("list_presets"),
  countSearch: (query: string, settings: Settings) =>
    invoke<number>("count_search", { query, settings }),
  readCardList: (path: string) => invoke<ListFile>("read_card_list", { path }),
  cancelRun: (id: number) => invoke<void>("cancel_run", { id }),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  cardDataStatus: () => invoke<CardDataStatus>("card_data_status"),
  openDiscord: () => invoke<void>("open_discord"),

  /** Starts a run; its events arrive on `onEvent`, ending with `finished`. */
  startRun: (input: RunInput, settings: Settings, onEvent: (event: RunEvent) => void) => {
    const channel = new Channel<RunEvent>();
    channel.onmessage = onEvent;
    return invoke<number>("start_run", { input, settings, onEvent: channel });
  },
};
