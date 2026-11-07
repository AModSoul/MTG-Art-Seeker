import { open } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";
import { persist } from "zustand/middleware";

import { api } from "@/api";
import type { FoundEvent } from "@/bindings/FoundEvent";
import type { JobInfo } from "@/bindings/JobInfo";
import type { JobStatus } from "@/bindings/JobStatus";
import type { RunEvent } from "@/bindings/RunEvent";
import type { RunInput } from "@/bindings/RunInput";
import type { RunSummary } from "@/bindings/RunSummary";
import type { Site } from "@/bindings/Site";
import type { Settings } from "@/bindings/Settings";
import type { SkippedLine } from "@/bindings/SkippedLine";
import { errorText, showError, showSuccess } from "@/notify";

// --- What the user is about to download, remembered between launches -------

export type InputTab = "list" | "search" | "preset";

interface InputState {
  tab: InputTab;
  text: string;
  /** The folder a list's art goes in, when the setting for it is on. */
  folder: string;
  query: string;
  presetId: string;
  setInput: (
    patch: Partial<Pick<InputState, "tab" | "text" | "folder" | "query" | "presetId">>,
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

// --- The current run, fed by its event channel -----------------------------

export type JobView = JobInfo & { status: JobStatus };

/** An image the user saved in the Lens window that didn't look like a larger
 * copy of the art, waiting for them to say whether to save it anyway. */
export type FoundDoubt = Extract<FoundEvent, { event: "doubtful" }>;

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
          jobs[event.id - 1] = { ...job, status: event.status };
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
  }));

  let queue: RunEvent[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;
  const flush = () => {
    timer = undefined;
    const events = queue;
    queue = [];
    applyEvents(events);
  };
  try {
    const runId = await api.startRun(input, settings, (event) => {
      queue.push(event);
      timer ??= setTimeout(flush, BATCH_MS);
    });
    useRun.setState({ runId });
  } catch (error) {
    useRun.setState({ running: false, error: errorText(error) });
  }
}

export function cancelRun() {
  const { runId } = useRun.getState();
  if (runId !== null) void api.cancelRun(runId);
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

function onFound(event: FoundEvent) {
  switch (event.event) {
    case "saved":
      useRun.setState((state) => {
        const job = state.jobs[event.job - 1];
        if (state.runId !== event.run || !job || job.status.state !== "done") return {};
        // The found image takes its face's place in the job's files.
        const files = job.status.files
          .filter((file) => file.faceIndex !== event.file.faceIndex)
          .concat(event.file)
          .toSorted((a, b) => a.faceIndex - b.faceIndex);
        const jobs = state.jobs.slice();
        jobs[event.job - 1] = { ...job, status: { ...job.status, files } };
        return { jobs };
      });
      showSuccess(`Saved the larger copy of ${event.file.face}`);
      break;
    case "doubtful":
      useRun.setState((state) => ({ doubts: [...state.doubts, event] }));
      break;
    case "failed":
      showError("Couldn't save the image", event.message);
      break;
  }
}
