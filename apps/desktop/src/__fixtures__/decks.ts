import type { DeckSearch } from "@/bindings/DeckSearch";
import type { DeckSummary } from "@/bindings/DeckSummary";

// Decks as MTGJSON lists them, newest first.

export const counterIntelligence: DeckSummary = {
  fileName: "CounterIntelligence_EOC",
  name: "Counter Intelligence",
  code: "EOC",
  setName: "Edge of Eternities Commander",
  type: "Commander Deck",
  releaseDate: "2025-08-01",
  hasBox: true,
};

export const worldShaper: DeckSummary = {
  fileName: "WorldShaper_EOC",
  name: "World Shaper",
  code: "EOC",
  setName: "Edge of Eternities Commander",
  type: "Commander Deck",
  releaseDate: "2025-08-01",
  hasBox: true,
};

export const abzanArmor: DeckSummary = {
  fileName: "AbzanArmor_TDC",
  name: "Abzan Armor",
  code: "TDC",
  setName: "Tarkir: Dragonstorm Commander",
  type: "Commander Deck",
  releaseDate: "2025-04-11",
  hasBox: true,
};

/** A deck no picture of the box is known for. */
export const shandalarDeck: DeckSummary = {
  fileName: "ArzakonsMinions_PSAL",
  name: "Arzakon's Minions",
  code: "PSAL",
  setName: "Shandalar Enemy Decks",
  type: "Shandalar Enemy Deck",
  releaseDate: null,
  hasBox: false,
};

export const allDecks: DeckSearch = {
  decks: [counterIntelligence, worldShaper, abzanArmor, shandalarDeck],
  total: 4,
};

/** What `set:eoc` finds. */
export const eocDecks: DeckSearch = { decks: [counterIntelligence, worldShaper], total: 2 };
