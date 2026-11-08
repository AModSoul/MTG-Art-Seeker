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
};
