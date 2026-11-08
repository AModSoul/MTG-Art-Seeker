// Typed wrappers over the app's Tauri commands (src-tauri/src/lib.rs). The types
// come from the Rust core through ts-rs — regenerate them with `npm run bindings`.

import { Channel, invoke } from "@tauri-apps/api/core";

import type { ArtSource } from "@/bindings/ArtSource";
import type { CardDataStatus } from "@/bindings/CardDataStatus";
import type { DeckSearch } from "@/bindings/DeckSearch";
import type { KnownSource } from "@/bindings/KnownSource";
import type { FoundEvent } from "@/bindings/FoundEvent";
import type { ListFile } from "@/bindings/ListFile";
import type { ParsedList } from "@/bindings/ParsedList";
import type { PresetInfo } from "@/bindings/PresetInfo";
import type { RunEvent } from "@/bindings/RunEvent";
import type { RunInput } from "@/bindings/RunInput";
import type { SavedFile } from "@/bindings/SavedFile";
import type { Site } from "@/bindings/Site";
import type { Settings } from "@/bindings/Settings";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  parseCardList: (text: string) => invoke<ParsedList>("parse_card_list", { text }),
  listPresets: () => invoke<PresetInfo[]>("list_presets"),
  countSearch: (query: string, settings: Settings) =>
    invoke<number>("count_search", { query, settings }),
  /** The precon decks `query` matches, newest first, up to `limit` of them. */
  searchDecks: (query: string, limit: number) =>
    invoke<DeckSearch>("search_decks", { query, limit }),
  /** Where the picture of a deck's box is saved; `null` when there's none. */
  deckBox: (fileName: string) => invoke<string | null>("deck_box", { fileName }),
  readCardList: (path: string) => invoke<ListFile>("read_card_list", { path }),
  cancelRun: (id: number) => invoke<void>("cancel_run", { id }),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  cardDataStatus: () => invoke<CardDataStatus>("card_data_status"),
  openDiscord: () => invoke<void>("open_discord"),

  confirmFoundArt: (token: number, save: boolean) =>
    invoke<void>("confirm_found_art", { token, save }),
  /** Saves image `index` of the page images named by `token`, or with `null`, none. */
  choosePageImage: (token: number, index: number | null) =>
    invoke<void>("choose_page_image", { token, index }),
  /** Where a saved face's art is known to be: MTGPics, Scryfall, the copies the user found. */
  artSources: (run: number, job: number, face: number) =>
    invoke<KnownSource[]>("art_sources", { run, job, face }),
  /** Saves a face's art again from `source` (for a found copy, the one `copy` names), in
   * place of the file saved for it. */
  saveArtFrom: (run: number, job: number, face: number, source: ArtSource, copy: number | null) =>
    invoke<SavedFile>("save_art_from", { run, job, face, source, copy }),
  openFoundPage: (run: number, job: number, face: number) =>
    invoke<void>("open_found_page", { run, job, face }),

  /** Opens `site`, in a window of its own, to look for a larger copy of a
   * saved face's art. What becomes of each image the user saves there arrives
   * on `onEvent`. */
  findLargerCopy: (
    run: number,
    job: number,
    face: number,
    site: Site,
    onEvent: (event: FoundEvent) => void,
  ) => {
    const channel = new Channel<FoundEvent>(onEvent);
    return invoke<void>("find_larger_copy", { run, job, face, site, onEvent: channel });
  },

  /** Starts a run; its events arrive on `onEvent`, ending with `finished`. */
  startRun: (input: RunInput, settings: Settings, onEvent: (event: RunEvent) => void) => {
    const channel = new Channel<RunEvent>();
    // Tauri's Channel has no addEventListener; onmessage is its only API.
    // oxlint-disable-next-line unicorn/prefer-add-event-listener
    channel.onmessage = onEvent;
    return invoke<number>("start_run", { input, settings, onEvent: channel });
  },
};
