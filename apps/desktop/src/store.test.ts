import { beforeEach, describe, expect, it, vi } from "vitest";

import { finishedSummary } from "@/__fixtures__/run";
import { defaultSettings } from "@/__fixtures__/settings";
import { api } from "@/api";
import type { RunEvent } from "@/bindings/RunEvent";
import type { RunSummary } from "@/bindings/RunSummary";
import type { Settings } from "@/bindings/Settings";
import { startRun, useCardAnvil, useRun, useSettings } from "@/store";

// No Tauri here: the commands the store calls, and the modules that reach Tauri, are stand-ins.
vi.mock("@/api", () => ({
  api: {
    startRun: vi.fn<() => void>(),
    sendToCardAnvil: vi.fn<() => void>(),
    openCardAnvilSite: vi.fn<() => void>(),
  },
}));
vi.mock("@/appWindow", () => ({ windowFrame: () => "custom" }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn<() => void>() }));

const startRunMock = vi.mocked(api.startRun);
const sendMock = vi.mocked(api.sendToCardAnvil);

/** Starts a run with `settings` as run 7, and returns what sends its events. */
async function started(settings: Settings): Promise<(event: RunEvent) => void> {
  let send: ((event: RunEvent) => void) | undefined;
  startRunMock.mockImplementation(async (_input, _settings, onEvent) => {
    send = onEvent;
    return 7;
  });
  useSettings.setState({ settings });
  await startRun({ kind: "list", text: "Karn, Scion of Urza" });
  if (!send) throw new Error("the run wasn't started");
  return send;
}

/** Lets the store's promises settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

const autoSend: Settings = { ...defaultSettings, autoSendToCardAnvil: true };

describe("sending a download to Card Anvil when it finishes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sendMock.mockResolvedValue({ outcome: "delivered", images: 5 });
    useRun.setState(useRun.getInitialState(), true);
    useCardAnvil.setState(useCardAnvil.getInitialState(), true);
  });

  it("sends the run's art once it finishes", async () => {
    const send = await started(autoSend);
    send({ event: "job", id: 1, status: { state: "resolving" } });
    await settle();
    expect(sendMock).not.toHaveBeenCalled();

    send({ event: "finished", summary: finishedSummary });
    await vi.waitFor(() => expect(sendMock).toHaveBeenCalledWith(7, null, autoSend));
    await vi.waitFor(() => expect(useCardAnvil.getState().sending).toBe(false));
  });

  it("sends a run that finishes before its id arrives", async () => {
    let resolveId: ((id: number) => void) | undefined;
    startRunMock.mockImplementation((_input, _settings, onEvent) => {
      onEvent({ event: "finished", summary: finishedSummary });
      return new Promise((resolve) => {
        resolveId = resolve;
      });
    });
    useSettings.setState({ settings: autoSend });
    const running = startRun({ kind: "list", text: "Karn, Scion of Urza" });
    await settle();
    expect(sendMock).not.toHaveBeenCalled();
    resolveId?.(9);
    await running;
    await vi.waitFor(() => expect(sendMock).toHaveBeenCalledWith(9, null, autoSend));
  });

  const unsent: [string, Settings, RunSummary][] = [
    ["was cancelled", autoSend, { ...finishedSummary, cancelled: 2 }],
    [
      "saved no art",
      autoSend,
      { ...finishedSummary, done: 0, partial: 0, mtgpicsFiles: 0, scryfallFiles: 0, webFiles: 0 },
    ],
    ["finished with the setting off", defaultSettings, finishedSummary],
  ];

  it.each(unsent)("doesn't send a run that %s", async (_, settings, summary) => {
    const send = await started(settings);
    send({ event: "finished", summary });
    await settle();
    expect(sendMock).not.toHaveBeenCalled();
  });
});
