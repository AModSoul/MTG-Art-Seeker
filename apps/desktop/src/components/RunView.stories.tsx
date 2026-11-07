import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, mocked, screen, userEvent, waitFor } from "storybook/test";

import {
  finishedJobs,
  finishedSummary,
  karnFile,
  mixedJobs,
  skippedLines,
} from "@/__fixtures__/run";
import { defaultSettings } from "@/__fixtures__/settings";
import { api } from "@/api";
import { useRun } from "@/store";

import { RunView } from "./RunView";

const meta = {
  component: RunView,
  decorators: [
    (Story) => (
      <div style={{ height: "640px" }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof RunView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Before the first download. */
export const Empty: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Nothing downloaded yet")).toBeVisible();
  },
};

/** A run has started but the Rust side hasn't planned its jobs yet. */
export const Planning: Story = {
  beforeEach: () => {
    useRun.setState({ running: true, runId: 1 });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Getting the list ready…")).toBeVisible();
  },
};

/** Mid-run, with a job in every state. Failed jobs can't be retried until the run ends. */
export const Running: Story = {
  beforeEach: () => {
    useRun.setState({ running: true, runId: 1, jobs: mixedJobs, skipped: skippedLines });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "4");
    await expect(canvas.getByText("4 of 7 cards")).toBeVisible();
    await expect(canvas.getByText(/Skipped 2 lines that aren't cards/)).toBeVisible();
    await expect(canvas.queryByRole("button", { name: /retry failed/i })).not.toBeInTheDocument();
  },
};

/** A finished run: filters narrow the table, and failed cards can be retried. */
export const Finished: Story = {
  beforeEach: () => {
    useRun.setState({ jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("7 of 7 cards in 12.4s")).toBeVisible();

    await userEvent.click(canvas.getByRole("radio", { name: "Failed 2" }));
    await expect(canvas.getByText("Not A Real Card")).toBeVisible();
    await expect(canvas.getByText("No art found for Insectile Aberration")).toBeVisible();
    await expect(canvas.queryByText("Lightning Bolt")).not.toBeInTheDocument();

    await userEvent.click(canvas.getByRole("radio", { name: "MTGPics 1" }));
    await expect(canvas.getByText("Karn, Scion of Urza (DOM) 1")).toBeVisible();
    await expect(canvas.queryByText("Not A Real Card")).not.toBeInTheDocument();

    await userEvent.click(canvas.getByRole("button", { name: /retry failed/i }));
    await expect(api.startRun).toHaveBeenCalledWith(
      { kind: "list", text: "Delver of Secrets\nNot A Real Card" },
      defaultSettings,
      expect.any(Function),
    );
  },
};

/** Each saved image's size in pixels, or a dash when its header couldn't be read. */
export const Resolution: Story = {
  beforeEach: () => {
    useRun.setState({ jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("columnheader", { name: "Resolution" })).toBeVisible();
    await expect(canvas.getByText("1920 × 1408")).toBeVisible();
    await expect(canvas.getByText("626 × 457")).toBeVisible();
    // Sol Ring, Damnation and Counterspell finish with an image of unknown size.
    await expect(canvas.getAllByText("—")).toHaveLength(3);
  },
};

/** Clicking a saved file shows it in its folder; "Open folder" opens the output folder. */
export const RevealFiles: Story = {
  beforeEach: () => {
    useRun.setState({ jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByText("Karn, Scion of Urza · Chase Stone"));
    await expect(api.revealFile).toHaveBeenCalledWith(karnFile.path);

    await userEvent.click(canvas.getByRole("button", { name: /open folder/i }));
    await expect(api.openFolder).toHaveBeenCalledWith(defaultSettings.outputDir);
  },
};

/** A list saved in its own folder: "Open folder" opens that folder, and a retry saves there too. */
export const ListFolder: Story = {
  beforeEach: () => {
    useRun.setState({
      jobs: finishedJobs,
      summary: finishedSummary,
      folder: `${defaultSettings.outputDir}/Burn deck`,
      listFolder: "Burn deck",
    });
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: /open folder/i }));
    await expect(api.openFolder).toHaveBeenCalledWith(`${defaultSettings.outputDir}/Burn deck`);

    await userEvent.click(canvas.getByRole("button", { name: /retry failed/i }));
    await expect(api.startRun).toHaveBeenCalledWith(
      { kind: "list", text: "Delver of Secrets\nNot A Real Card", folder: "Burn deck" },
      defaultSettings,
      expect.any(Function),
    );
  },
};

/** A Tauri command that fails shows its error as a toast. */
export const OpenFolderFails: Story = {
  beforeEach: () => {
    useRun.setState({ jobs: finishedJobs, summary: finishedSummary });
    mocked(api.openFolder).mockRejectedValue("No such folder");
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: /open folder/i }));
    // Toasts render in a portal, outside the canvas.
    await waitFor(() => expect(screen.getByText("Couldn't open the folder")).toBeVisible());
    await expect(screen.getByText("No such folder")).toBeInTheDocument();
  },
};

/** The run couldn't start, so there are no jobs to show. */
export const StartFailed: Story = {
  beforeEach: () => {
    useRun.setState({ error: "EDHREC no longer has the list “Top 100, past month”" });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("The download couldn't start")).toBeVisible();
  },
};

/** A cancelled run says so instead of "Finished". */
export const Cancelled: Story = {
  beforeEach: () => {
    useRun.setState({
      jobs: mixedJobs.map((job) =>
        job.status.state === "done" ? job : { ...job, status: { state: "cancelled" } },
      ),
      summary: { ...finishedSummary, done: 3, partial: 0, failed: 0, cancelled: 4 },
    });
  },
  play: async ({ canvas }) => {
    await expect(canvas.queryByText("Finished")).not.toBeInTheDocument();
    // The progress label plus one badge per cancelled job.
    await expect(canvas.getAllByText("Cancelled")).toHaveLength(5);
  },
};
