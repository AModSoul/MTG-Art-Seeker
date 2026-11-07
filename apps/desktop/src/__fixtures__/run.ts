import type { CardDataStatus } from "@/bindings/CardDataStatus";
import type { PresetInfo } from "@/bindings/PresetInfo";
import type { RunSummary } from "@/bindings/RunSummary";
import type { SavedFile } from "@/bindings/SavedFile";
import type { SkippedLine } from "@/bindings/SkippedLine";
import type { JobView } from "@/store";

const dir = "/home/user/Pictures/MTG Art";

export const karnFile: SavedFile = {
  path: `${dir}/MTGPics/Karn, Scion of Urza (Chase Stone) [DOM] {1}.jpg`,
  face: "Karn, Scion of Urza",
  source: "mtgpics",
  set: "dom",
  number: "1",
  artist: "Chase Stone",
  size: { width: 1920, height: 1408 },
};

export const boltFile: SavedFile = {
  path: `${dir}/Scryfall/Lightning Bolt (Christopher Moeller) [M10] {146}.jpg`,
  face: "Lightning Bolt",
  source: "scryfall",
  set: "m10",
  number: "146",
  artist: "Christopher Moeller",
  size: { width: 626, height: 457 },
};

/** One job in each state a run can show. */
export const mixedJobs: JobView[] = [
  {
    id: 1,
    label: "Karn, Scion of Urza (DOM) 1",
    line: "Karn, Scion of Urza (DOM) 1",
    status: { state: "done", files: [karnFile], missing: [] },
  },
  {
    id: 2,
    label: "Lightning Bolt",
    line: "Lightning Bolt",
    status: { state: "done", files: [boltFile], missing: [] },
  },
  {
    id: 3,
    label: "Delver of Secrets",
    line: "Delver of Secrets",
    status: { state: "done", files: [], missing: ["Insectile Aberration"] },
  },
  {
    id: 4,
    label: "Not A Real Card",
    line: "Not A Real Card",
    status: { state: "failed", error: 'Scryfall has no card named "Not A Real Card"' },
  },
  {
    id: 5,
    label: "Sol Ring (C21) 263",
    line: "Sol Ring (C21) 263",
    status: { state: "downloading" },
  },
  { id: 6, label: "Damnation", line: "Damnation", status: { state: "resolving" } },
  { id: 7, label: "Counterspell", line: "Counterspell", status: { state: "queued" } },
];

/** A saved image whose header couldn't be read, so its size is unknown. */
export const unknownSizeFile: SavedFile = {
  ...boltFile,
  path: `${dir}/Scryfall/Counterspell (Mark Poole) [2ED] {55}.jpg`,
  face: "Counterspell",
  artist: "Mark Poole",
  set: "2ed",
  number: "55",
  size: null,
};

/** The same jobs once the run has finished. */
export const finishedJobs: JobView[] = mixedJobs.map((job) =>
  job.status.state === "done" || job.status.state === "failed"
    ? job
    : { ...job, status: { state: "done", files: [unknownSizeFile], missing: [] } },
);

export const finishedSummary: RunSummary = {
  jobs: 7,
  done: 5,
  partial: 1,
  failed: 1,
  cancelled: 0,
  mtgpicsFiles: 1,
  scryfallFiles: 4,
  seconds: 12.4,
};

export const skippedLines: SkippedLine[] = [
  { line: "// Commander", reason: "comment" },
  { line: "Creatures (12)", reason: "heading" },
];

export const presets: PresetInfo[] = [
  { id: "edhrec-top-year", label: "Top 100, past 2 years", group: "EDHREC" },
  { id: "edhrec-top-month", label: "Top 100, past month", group: "EDHREC" },
  { id: "edhrec-salt", label: "Saltiest 100", group: "EDHREC" },
];

/** A local copy of Scryfall's card data, downloaded and current. */
export const cardData: CardDataStatus = {
  updatedAt: "2026-11-06T09:00:00Z",
  cards: 112_345,
  updating: false,
  error: null,
};
