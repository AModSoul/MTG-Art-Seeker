// Stands in for `src/api.ts` while the screenshots are taken: every answer comes from a run
// recorded by `crates/core/examples/record.rs`, which `shoot.ts` puts on `window.screenshotRecording`.
// It's typed as the real API, so `pnpm typecheck` says when the two drift apart.

import type { api as realApi } from "@/api";
import type { CardDataStatus } from "@/bindings/CardDataStatus";
import type { DeckSearch } from "@/bindings/DeckSearch";
import type { DeckSummary } from "@/bindings/DeckSummary";
import type { KnownSource } from "@/bindings/KnownSource";
import type { ParsedList } from "@/bindings/ParsedList";
import type { RunEvent } from "@/bindings/RunEvent";
import type { Settings } from "@/bindings/Settings";

/** What `record.rs` writes. */
export interface Recording {
  settings: Settings;
  cardData: CardDataStatus | null;
  input:
    | { kind: "list"; text: string }
    | { kind: "deck"; query: string; deck: DeckSummary; folder: string };
  events: RunEvent[];
  /** A list's: what its text parses to, and each saved face's sources, keyed `job:face`. */
  parsed?: ParsedList;
  sources?: Record<string, KnownSource[]>;
  /** A deck's: the search that found it, and where each deck's box picture was saved. */
  decks?: DeckSearch;
  boxes?: Record<string, string | null>;
}

declare global {
  interface Window {
    screenshotRecording: Recording;
  }
}

/** The folder the screenshots say art is saved to, wherever the recording saved it. */
const SHOWN_OUTPUT_DIR = "C:\\MTG Art";

const recording = () => window.screenshotRecording;
const done = async () => {};

export const api: typeof realApi = {
  getSettings: async () => ({ ...recording().settings, outputDir: SHOWN_OUTPUT_DIR }),
  saveSettings: done,
  parseCardList: async () => recording().parsed ?? { cards: [], skipped: [] },
  listPresets: async () => [],
  countSearch: async () => 0,
  searchDecks: async () => recording().decks ?? { decks: [], total: 0 },
  deckBox: async (fileName) => recording().boxes?.[fileName] ?? null,
  readCardList: async () => ({ text: "", name: "" }),
  cancelRun: done,
  revealFile: done,
  openFolder: done,
  cardDataStatus: async () =>
    recording().cardData ?? { updatedAt: null, cards: 0, updating: false, error: null },
  openDiscord: done,
  confirmFoundArt: done,
  choosePageImage: done,
  artSources: async (_run, job, face) => recording().sources?.[`${job}:${face}`] ?? [],
  saveArtFrom: () => Promise.reject(new Error("Not while taking screenshots")),
  openFoundPage: done,
  findLargerCopy: done,
  // The recorded events, all at once, as a fast run would send them.
  startRun: async (_input, _settings, onEvent) => {
    setTimeout(() => recording().events.forEach(onEvent));
    return 1;
  },
};
