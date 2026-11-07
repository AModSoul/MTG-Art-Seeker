import { create } from "zustand";
import { persist } from "zustand/middleware";

import { api, errorText } from "@/api";
import type { JobInfo } from "@/bindings/JobInfo";
import type { JobStatus } from "@/bindings/JobStatus";
import type { RunEvent } from "@/bindings/RunEvent";
import type { RunInput } from "@/bindings/RunInput";
import type { RunSummary } from "@/bindings/RunSummary";
import type { Settings } from "@/bindings/Settings";
import type { SkippedLine } from "@/bindings/SkippedLine";
import { toaster } from "@/components/ui/toaster";

// --- What the user is about to download, remembered between launches -------

export const INPUT_TABS = ["list", "search", "preset"] as const;
export type InputTab = (typeof INPUT_TABS)[number];

export function isInputTab(value: string): value is InputTab {
  return (INPUT_TABS as readonly string[]).includes(value);
}

interface InputState {
  tab: InputTab;
  text: string;
  query: string;
  presetId: string;
  setInput: (patch: Partial<Pick<InputState, "tab" | "text" | "query" | "presetId">>) => void;
}

export const useInput = create<InputState>()(
  persist(
    (set) => ({
      tab: "list",
      text: "",
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
      toaster.create({
        type: "error",
        title: "Couldn't read settings",
        description: errorText(error),
      });
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
      api.saveSettings(settings).catch((error: unknown) => {
        toaster.create({
          type: "error",
          title: "Couldn't save settings",
          description: errorText(error),
        });
      });
    }, 300);
  },
}));

// --- The current run, fed by its event channel -----------------------------

export type JobView = JobInfo & { status: JobStatus };

interface RunState {
  runId: number | null;
  running: boolean;
  jobs: JobView[];
  skipped: SkippedLine[];
  error: string | null;
  summary: RunSummary | null;
}

const idle: RunState = {
  runId: null,
  running: false,
  jobs: [],
  skipped: [],
  error: null,
  summary: null,
};

export const useRun = create<RunState>()(() => idle);

/** Applies a batch of events. Job ids count up from 1, so a job is at `id - 1`. */
function applyEvents(events: RunEvent[]) {
  useRun.setState((state) => {
    let { jobs, skipped, error, summary, running } = state;
    let copied = false;
    for (const event of events) {
      switch (event.event) {
        case "planned":
          jobs = event.jobs.map((job) => ({ ...job, status: { state: "queued" } }));
          copied = true;
          skipped = event.skipped;
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
    return { jobs, skipped, error, summary, running };
  });
}

/** How long events are gathered before the table updates, so a busy run
 * re-renders a few times a second rather than once per event. */
const BATCH_MS = 80;

export async function startRun(input: RunInput) {
  const { settings } = useSettings.getState();
  if (!settings || useRun.getState().running) return;
  useRun.setState({ ...idle, running: true });

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
