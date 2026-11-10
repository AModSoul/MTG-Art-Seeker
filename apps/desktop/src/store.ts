import { type DialogFilter, open } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";
import { persist } from "zustand/middleware";

import { api } from "@/api";
import { windowFrame } from "@/appWindow";
import { toaster } from "@mtg-art/ui/toaster";
import type { ArtSource } from "@/bindings/ArtSource";
import type { DeckSummary } from "@/bindings/DeckSummary";
import type { FoundEvent } from "@/bindings/FoundEvent";
import type { JobInfo } from "@/bindings/JobInfo";
import type { KnownSource } from "@/bindings/KnownSource";
import type { JobStatus } from "@/bindings/JobStatus";
import type { RunEvent } from "@/bindings/RunEvent";
import type { RunInput } from "@/bindings/RunInput";
import type { RunSummary } from "@/bindings/RunSummary";
import type { SavedFile } from "@/bindings/SavedFile";
import type { Site } from "@/bindings/Site";
import type { Settings } from "@/bindings/Settings";
import type { SkippedLine } from "@/bindings/SkippedLine";
import {
  errorText,
  showError,
  showInfo,
  showLoading,
  showSuccess,
  showWarning,
  type ToastAction,
} from "@/notify";

// --- What the user is about to download, remembered between launches -------

export type InputTab = "list" | "search" | "preset" | "deck";

/** The precon deck picked to download, as much of it as the panel shows. */
export type PickedDeck = Pick<DeckSummary, "fileName" | "name" | "code" | "setName">;

interface InputState {
  tab: InputTab;
  text: string;
  /** The folder a list's art goes in, when the setting for it is on. */
  folder: string;
  query: string;
  presetId: string;
  /** The search typed above the precon decks. */
  deckQuery: string;
  deck: PickedDeck | null;
  /** The folder a deck's art goes in, when the setting for it is on: the
   * deck's name when it's picked, and whatever the user makes it after. */
  deckFolder: string;
  setInput: (
    patch: Partial<
      Pick<
        InputState,
        "tab" | "text" | "folder" | "query" | "presetId" | "deckQuery" | "deck" | "deckFolder"
      >
    >,
  ) => void;
}

export const useInput = create<InputState>()(
  persist(
    (set) => ({
      tab: "list",
      text: "",
      folder: "",
      query: "",
      presetId: "edhrec-top-year",
      deckQuery: "",
      deck: null,
      deckFolder: "",
      setInput: (patch) => set(patch),
    }),
    { name: "input" },
  ),
);

// --- Settings, kept by the Rust side in the shared settings file ------------

interface SettingsState {
  settings: Settings | null;
  load: () => Promise<void>;
  update: (patch: Partial<Settings>) => void;
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

export const useSettings = create<SettingsState>()((set, get) => ({
  settings: null,
  load: async () => {
    try {
      set({ settings: await api.getSettings() });
    } catch (error) {
      showError("Couldn't read settings", error);
    }
  },
  update: (patch) => {
    const current = get().settings;
    if (!current) return;
    const settings = { ...current, ...patch };
    set({ settings });
    // Typing in a field saves once it pauses, not on every key.
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      api
        .saveSettings(settings)
        .catch((error: unknown) => showError("Couldn't save settings", error));
    }, 300);
  },
}));

/** Asks for the folder art is saved to, starting from the current one. */
export async function chooseOutputDir() {
  const { settings, update } = useSettings.getState();
  const dir = await open({ directory: true, defaultPath: settings?.outputDir });
  if (typeof dir === "string") update({ outputDir: dir });
}

/** Asks which Card Anvil to send art to, starting from the one chosen before. */
export async function chooseCardAnvil() {
  const { settings, update } = useSettings.getState();
  const path = await open({
    title: "Choose Card Anvil",
    directory: false,
    multiple: false,
    defaultPath: settings?.cardAnvilPath,
    filters: cardAnvilFilters(),
  });
  if (typeof path === "string") update({ cardAnvilPath: path });
}

/** What Card Anvil is on this system, for the picker to show: an app bundle on macOS, a
 * program on Windows. Linux's programs have no extension to filter by. */
function cardAnvilFilters(): DialogFilter[] {
  if (windowFrame() === "mac") return [{ name: "Card Anvil", extensions: ["app"] }];
  return navigator.userAgent.includes("Windows")
    ? [{ name: "Card Anvil", extensions: ["exe"] }]
    : [];
}

// --- The current run, fed by its event channel -----------------------------

export type JobView = JobInfo & { status: JobStatus };

/** An image the user saved in the Lens window that didn't look like a larger
 * copy of the art, waiting for them to say whether to save it anyway. */
export type FoundDoubt = Extract<FoundEvent, { event: "doubtful" }>;

/** The images of a page the user asked to find the art on, for them to choose
 * from. */
export type PageImages = Extract<FoundEvent, { event: "choose" }>;

interface RunState {
  runId: number | null;
  running: boolean;
  jobs: JobView[];
  skipped: SkippedLine[];
  error: string | null;
  summary: RunSummary | null;
  /** The folder the run saves to, as the core reports it. */
  folder: string | null;
  /** The list folder name the run was started with, for retrying into it. */
  listFolder: string | undefined;
  /** Found images waiting on the user, oldest first. They outlast a run. */
  doubts: FoundDoubt[];
  /** A page's images, while the user chooses one. They outlast a run. */
  pageImages: PageImages | null;
}

const idle: RunState = {
  runId: null,
  running: false,
  jobs: [],
  skipped: [],
  error: null,
  summary: null,
  folder: null,
  listFolder: undefined,
  doubts: [],
  pageImages: null,
};

export const useRun = create<RunState>()(() => idle);

/** Applies a batch of events. Job ids count up from 1, so a job is at `id - 1`. */
function applyEvents(events: RunEvent[]) {
  useRun.setState((state) => {
    let { jobs, skipped, error, summary, running, folder } = state;
    let copied = false;
    for (const event of events) {
      switch (event.event) {
        case "planned":
          jobs = event.jobs.map((job) => ({ ...job, status: { state: "queued" } }));
          copied = true;
          skipped = event.skipped;
          folder = event.folder;
          break;
        case "job": {
          const job = jobs[event.id - 1];
          if (!job) break;
          if (!copied) {
            jobs = jobs.slice();
            copied = true;
          }
          // The count of images it will save outlasts the status that says it,
          // so a row keeps its height if the job then fails.
          jobs[event.id - 1] = {
            ...job,
            status: event.status,
            images: event.status.state === "downloading" ? event.status.images : job.images,
          };
          break;
        }
        case "error":
          error = event.message;
          break;
        case "finished":
          summary = event.summary;
          running = false;
          break;
      }
    }
    return { jobs, skipped, error, summary, running, folder };
  });
}

/** How long events are gathered before the table updates, so a busy run
 * re-renders a few times a second rather than once per event. */
const BATCH_MS = 80;

export async function startRun(input: RunInput) {
  const { settings } = useSettings.getState();
  if (!settings || useRun.getState().running) return;
  useRun.setState((state) => ({
    ...idle,
    running: true,
    listFolder: input.kind === "list" ? input.folder : undefined,
    doubts: state.doubts,
    pageImages: state.pageImages,
  }));

  let queue: RunEvent[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;
  const flush = () => {
    timer = undefined;
    const events = queue;
    queue = [];
    applyEvents(events);
  };
  // Kept, so a send when the run finishes goes to this run, whenever its id arrives.
  const runIdPromise = api.startRun(input, settings, (event) => {
    queue.push(event);
    timer ??= setTimeout(flush, BATCH_MS);
    // The setting as it is when the run finishes, not as it was when it started.
    const current = event.event === "finished" ? useSettings.getState().settings : null;
    if (event.event === "finished" && current && sendsWhenFinished(current, event.summary)) {
      // Asked for a moment later: an event can arrive before `runIdPromise` is set.
      void Promise.resolve()
        .then(() => runIdPromise)
        .then(
          (id) => sendToCardAnvil(undefined, id),
          () => {},
        );
    }
  });
  try {
    const runId = await runIdPromise;
    useRun.setState({ runId });
  } catch (error) {
    useRun.setState({ running: false, error: errorText(error) });
  }
}

/** Whether a run that finished as `summary` says is sent to Card Anvil on its own: when
 * the setting is on, it saved art, and nothing was cancelled. */
function sendsWhenFinished(settings: Settings, summary: RunSummary): boolean {
  const saved = summary.mtgpicsFiles + summary.scryfallFiles + summary.webFiles;
  return settings.autoSendToCardAnvil && summary.cancelled === 0 && saved > 0;
}

export function cancelRun() {
  const { runId } = useRun.getState();
  if (runId !== null) void api.cancelRun(runId);
}

// --- Sending art to Card Anvil ------------------------------------------------

interface CardAnvilState {
  /** How many sends are on their way to Card Anvil, each until it's taken or it's clear it
   * won't be soon. They can overlap: one when a run finishes doesn't wait for the user's. */
  sending: number;
}

export const useCardAnvil = create<CardAnvilState>()(() => ({ sending: 0 }));

/** Numbers each send's toast, so a second doesn't take over the first's. */
let sendToasts = 0;

/** The button on a toast that says Card Anvil needs installing or updating. */
const GET_CARD_ANVIL: ToastAction = {
  label: "Get Card Anvil",
  onClick: () => void openCardAnvilSite(),
};

/** Sends the art run `run` (the one shown, by default) saved to Card Anvil: jobs `jobs`'
 * art, or all of it. Card Anvil opens and asks where to put it. */
export async function sendToCardAnvil(jobs?: number[], run?: number) {
  const runId = run ?? useRun.getState().runId;
  const { settings } = useSettings.getState();
  if (runId === null || !settings) return;
  const id = `card-anvil-${++sendToasts}`;
  useCardAnvil.setState((state) => ({ sending: state.sending + 1 }));
  showLoading("Sending the art to Card Anvil…", id);
  try {
    const sent = await api.sendToCardAnvil(runId, jobs ?? null, settings);
    switch (sent.outcome) {
      case "delivered":
        showSent(sent.images, id);
        break;
      case "notFound":
        showWarning("Card Anvil isn't installed", {
          id,
          description: "Install it, or choose where it is in Settings.",
          action: GET_CARD_ANVIL,
        });
        break;
      case "notYetTaken":
        showWarning("Card Anvil hasn't taken the art yet", {
          id,
          description: `It may still be starting, so give it a minute before sending again. If it doesn't ask where to put ${sent.images === 1 ? "it" : "them"} by then, update Card Anvil.`,
          action: GET_CARD_ANVIL,
        });
        // Card Anvil may yet take it: the toast says so if it does. That isn't a send in
        // progress, so the buttons don't wait for it.
        void api.cardAnvilTaken(sent.request).then(
          (taken) => {
            if (taken) showSent(sent.images, id);
          },
          () => {},
        );
        break;
      case "tooMany":
        showError(
          "Too many cards for Card Anvil",
          `Card Anvil takes up to ${count(sent.max, "card")} at a time, and that's ${count(sent.cards, "card")}. Send cards one at a time instead: right-click a card.`,
          { id },
        );
        break;
    }
  } catch (error) {
    showError("Couldn't send the art to Card Anvil", error, { id });
  } finally {
    useCardAnvil.setState((state) => ({ sending: state.sending - 1 }));
  }
}

/** The toast for `images` images Card Anvil has taken, in place of toast `id`. */
function showSent(images: number, id: string) {
  showSuccess(`Sent ${count(images, "image")} to Card Anvil`, {
    id,
    description: `Card Anvil will ask where to put ${images === 1 ? "it" : "them"}.`,
  });
}

/** Opens Card Anvil's download page. */
export async function openCardAnvilSite() {
  try {
    await api.openCardAnvilSite();
  } catch (error) {
    showError("Couldn't open Card Anvil's website", error);
  }
}

/** `n` of `noun`, counted: "1 image", "1,200 images". */
function count(n: number, noun: string): string {
  return `${n.toLocaleString()} ${noun}${n === 1 ? "" : "s"}`;
}

// --- Larger copies the user finds in the finder window ----------------------

/** Opens `site` to look for a larger copy of face `face` of job `job` of the
 * current run. */
export async function findLargerCopy(job: number, face: number, site: Site) {
  const { runId } = useRun.getState();
  if (runId === null) return;
  try {
    await api.findLargerCopy(runId, job, face, site, onFound);
  } catch (error) {
    showError("Couldn't open the search", error);
  }
}

/** Opens the page a found image was saved from. */
export async function openFoundPage(job: number, face: number) {
  const { runId } = useRun.getState();
  if (runId === null) return;
  try {
    await api.openFoundPage(runId, job, face);
  } catch (error) {
    showError("Couldn't open the page", error);
  }
}

/** Where face `face` of job `job` is known to have art; empty if that can't be told. */
export async function artSources(job: number, face: number): Promise<KnownSource[]> {
  const { runId } = useRun.getState();
  if (runId === null) return [];
  try {
    return await api.artSources(runId, job, face);
  } catch (error) {
    showError("Couldn't tell where the art is", error);
    return [];
  }
}

/** What each source is called where the user picks one. */
const SOURCE_NAMES: Record<ArtSource, string> = {
  mtgpics: "MTGPics",
  scryfall: "Scryfall",
  web: "a copy you found",
};

/** The sites a copy was found from, by name. */
export const SITE_NAMES: Record<Site, string> = {
  lens: "Google Lens",
  artstation: "ArtStation",
  deviantart: "DeviantArt",
  pixiv: "Pixiv",
  instagram: "Instagram",
  website: "the artist's website",
};

/** Where `known` is, in a few words: MTGPics, Scryfall, or the site a copy was on. */
export function knownSourceName(known: KnownSource): string {
  if (known.source !== "web") return SOURCE_NAMES[known.source];
  return known.host ?? (known.site ? SITE_NAMES[known.site] : SOURCE_NAMES.web);
}

/** Saves face `face` of job `job` again from `known`, in place of its file. */
export async function saveArtFrom(job: number, face: number, known: KnownSource) {
  const { runId } = useRun.getState();
  if (runId === null) return;
  const toast = `source-${job}-${face}`;
  const name = knownSourceName(known);
  showLoading(`Saving the art from ${name}…`, toast);
  try {
    const file = await api.saveArtFrom(runId, job, face, known.source, known.copy);
    replaceFile(runId, job, file);
    showSuccess(`Saved ${file.face}'s art from ${name}`, { id: toast });
  } catch (error) {
    showError("Couldn't save the art", error, { id: toast });
  }
}

/** Puts `file` in its face's place among job `job`'s files, if run `run` is still shown. */
function replaceFile(run: number, job: number, file: SavedFile) {
  useRun.setState((state) => {
    const shown = state.jobs[job - 1];
    if (state.runId !== run || !shown || shown.status.state !== "done") return {};
    const files = shown.status.files
      .filter((other) => other.faceIndex !== file.faceIndex)
      .concat(file)
      .toSorted((a, b) => a.faceIndex - b.faceIndex);
    const jobs = state.jobs.slice();
    jobs[job - 1] = { ...shown, status: { ...shown.status, files } };
    return { jobs };
  });
}

/** Answers the oldest waiting image: saves it anyway, or throws it away. */
export async function answerDoubt(save: boolean) {
  const [first, ...rest] = useRun.getState().doubts;
  if (!first) return;
  useRun.setState({ doubts: rest });
  try {
    await api.confirmFoundArt(first.token, save);
  } catch (error) {
    showError("Couldn't save the image", error);
  }
}

/** Saves image `index` of the page's images as the art, or with `null`, none. */
export async function choosePageImage(index: number | null) {
  const { pageImages } = useRun.getState();
  if (!pageImages) return;
  useRun.setState({ pageImages: null });
  if (index !== null) showLoading(`Saving ${pageImages.face}'s art…`, FOUND_TOAST);
  try {
    await api.choosePageImage(pageImages.token, index);
  } catch (error) {
    showError("Couldn't save the image", error, { id: FOUND_TOAST });
  }
}

/** The toast that says what the finder window's images came to: one at a time,
 * a spinner while they're checked and then the result. */
const FOUND_TOAST = "found";

function onFound(event: FoundEvent) {
  switch (event.event) {
    case "checking":
      showLoading(
        event.page
          ? "Downloading the images on the page…"
          : `Checking the image you saved for ${event.face}…`,
        FOUND_TOAST,
      );
      break;
    case "saved":
      replaceFile(event.run, event.job, event.file);
      showSuccess(`Saved the larger copy of ${event.file.face}`, { id: FOUND_TOAST });
      break;
    case "doubtful":
      // The dialog asks; the spinner has nothing left to say.
      toaster.dismiss(FOUND_TOAST);
      useRun.setState((state) => ({ doubts: [...state.doubts, event] }));
      break;
    case "choose":
      // The dialog shows them; the spinner has nothing left to say.
      toaster.dismiss(FOUND_TOAST);
      useRun.setState({ pageImages: event });
      break;
    case "noImages":
      showInfo("No image on this page is large enough", {
        id: FOUND_TOAST,
        description: `None could be a larger copy of ${event.face}'s art.`,
      });
      break;
    case "failed":
      showError("Couldn't save the image", event.message, { id: FOUND_TOAST });
      break;
  }
}
