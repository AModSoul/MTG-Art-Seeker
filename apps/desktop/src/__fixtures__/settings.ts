import type { CardAnvilStatus } from "@/bindings/CardAnvilStatus";
import type { Settings } from "@/bindings/Settings";

/** The Rust side's defaults (crates/core/src/settings.rs), with a fixed output folder. */
export const defaultSettings: Settings = {
  outputDir: "/home/user/Pictures/MTG Art",
  naming: "NAME (ARTIST) [SET] {NUMBER}",
  sources: "mtgpicsThenScryfall",
  scryfallImage: "artCrop",
  searchMtgpicsArt: false,
  printing: "newest",
  downloadAll: false,
  uniqueArt: true,
  excludeFullArt: false,
  includeExtras: true,
  listFolders: false,
  sourceFolders: true,
  layoutFolders: true,
  typeFolders: false,
  overwrite: true,
  autoSendToCardAnvil: false,
};

/** Card Anvil, installed where its installer puts it, and no other copy chosen. */
export const installedCardAnvil: CardAnvilStatus = {
  path: "/opt/Card Anvil/card-anvil",
  chosen: false,
  chosenMissing: false,
};

/** A development build of Card Anvil the user chose in place of the installed one. */
export const chosenCardAnvil: CardAnvilStatus = {
  path: "/home/user/Card-Anvil/src-tauri/target/debug/card-anvil",
  chosen: true,
  chosenMissing: false,
};
