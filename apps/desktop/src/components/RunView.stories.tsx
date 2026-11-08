import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, mocked, screen, userEvent, waitFor } from "storybook/test";

import {
  boltFile,
  finishedJobs,
  finishedSummary,
  foundBoltFile,
  karnFile,
  mixedJobs,
  skippedLines,
} from "@/__fixtures__/run";
import { defaultSettings } from "@/__fixtures__/settings";
import { api } from "@/api";
import type { KnownSource } from "@/bindings/KnownSource";
import { useRun, useSettings } from "@/store";

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

/** A card that got Scryfall's art has a button for each place to look for a
 * larger copy: Google Lens, and the artist on ArtStation, DeviantArt and Pixiv. */
export const FindLargerCopy: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    // MTGPics' art has no such buttons.
    await expect(
      canvas.queryByRole("button", { name: /Google Lens.*Karn, Scion of Urza/ }),
    ).not.toBeInTheDocument();

    await userEvent.click(
      canvas.getByRole("button", { name: "Search Google Lens for this art, for Lightning Bolt" }),
    );
    await expect(api.findLargerCopy).toHaveBeenLastCalledWith(
      3,
      2,
      0,
      "lens",
      expect.any(Function),
    );
    await userEvent.click(
      canvas.getByRole("button", {
        name: "Look for Christopher Moeller on ArtStation, for Lightning Bolt",
      }),
    );
    await expect(api.findLargerCopy).toHaveBeenLastCalledWith(
      3,
      2,
      0,
      "artstation",
      expect.any(Function),
    );
    await expect(
      canvas.getByRole("button", { name: /Look for Christopher Moeller on DeviantArt/ }),
    ).toBeVisible();
    await expect(
      canvas.getByRole("button", { name: /Look for Christopher Moeller on Pixiv/ }),
    ).toBeVisible();
    await expect(
      canvas.getByRole("button", { name: /Look for Christopher Moeller on Instagram/ }),
    ).toBeVisible();
    await userEvent.click(
      canvas.getByRole("button", {
        name: "Open Christopher Moeller's website, for Lightning Bolt",
      }),
    );
    await expect(api.findLargerCopy).toHaveBeenLastCalledWith(
      3,
      2,
      0,
      "website",
      expect.any(Function),
    );
  },
};

/** With the setting on, MTGPics' art has the search buttons too. */
export const FindLargerCopyOfMtgpicsArt: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    useSettings.setState({ settings: { ...defaultSettings, searchMtgpicsArt: true } });
  },
  play: async ({ canvas }) => {
    await userEvent.click(
      canvas.getByRole("button", {
        name: "Look for Chase Stone on ArtStation, for Karn, Scion of Urza",
      }),
    );
    await expect(api.findLargerCopy).toHaveBeenLastCalledWith(
      3,
      1,
      0,
      "artstation",
      expect.any(Function),
    );
  },
};

/** An image the user saves in the Lens window takes the place of Scryfall's in its row. */
export const SavesAFoundCopy: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.findLargerCopy).mockImplementation(async (run, job, _face, _site, onEvent) => {
      onEvent({ event: "saved", run, job, file: foundBoltFile, replaced: boltFile.path });
    });
  },
  play: async ({ canvas }) => {
    await userEvent.click(
      canvas.getByRole("button", { name: "Search Google Lens for this art, for Lightning Bolt" }),
    );
    await waitFor(() =>
      expect(screen.getByText("Saved the larger copy of Lightning Bolt")).toBeVisible(),
    );
    await expect(canvas.getByText("2400 × 1752")).toBeVisible();
    await userEvent.click(canvas.getByRole("radio", { name: "Found 1" }));
    await expect(canvas.getByText("Lightning Bolt")).toBeVisible();
  },
};

/** Asked to find the art on a page, the app found no image there large enough. */
export const NothingOnThePage: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.findLargerCopy).mockImplementation(async (_run, _job, _face, _site, onEvent) => {
      onEvent({ event: "noImages", face: "Lightning Bolt" });
    });
  },
  play: async ({ canvas }) => {
    await userEvent.click(
      canvas.getByRole("button", { name: "Search Google Lens for this art, for Lightning Bolt" }),
    );
    await waitFor(() =>
      expect(screen.getByText("No image on this page is large enough")).toBeVisible(),
    );
    await expect(
      screen.getByText("None could be a larger copy of Lightning Bolt's art."),
    ).toBeVisible();
  },
};

/** Asked to find the art on a page, the app has its images to choose from. */
export const ImagesOnThePage: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.findLargerCopy).mockImplementation(async (_run, _job, _face, _site, onEvent) => {
      onEvent({ event: "checking", face: "Lightning Bolt", page: true });
      onEvent({
        event: "choose",
        token: 9,
        face: "Lightning Bolt",
        images: [
          {
            path: "C:/cache/page-1.jpg",
            size: { width: 2400, height: 1752 },
            doubt: null,
            host: "cdna.artstation.com",
          },
        ],
      });
    });
  },
  play: async ({ canvas }) => {
    await userEvent.click(
      canvas.getByRole("button", { name: "Search Google Lens for this art, for Lightning Bolt" }),
    );
    await waitFor(() => expect(useRun.getState().pageImages?.token).toBe(9));
    // The dialog takes over from the spinner.
    await waitFor(() =>
      expect(screen.queryByText("Downloading the images on the page…")).not.toBeInTheDocument(),
    );
  },
};

/** Found art is badged as such, and links to the page it came from. */
export const FoundArt: Story = {
  beforeEach: () => {
    useRun.setState({
      runId: 3,
      jobs: finishedJobs.map((job) =>
        job.id === 2
          ? { ...job, status: { state: "done", files: [foundBoltFile], missing: [] } }
          : job,
      ),
      summary: finishedSummary,
    });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Found")).toBeVisible();
    await expect(
      canvas.queryByRole("button", { name: /Google Lens.*Lightning Bolt/ }),
    ).not.toBeInTheDocument();

    await userEvent.click(
      canvas.getByRole("button", { name: "Open the page Lightning Bolt's art came from" }),
    );
    await expect(api.openFoundPage).toHaveBeenCalledWith(3, 2, 0);
  },
};

/** Where Lightning Bolt's art is known to be, the found copy saved now. */
const boltSources: KnownSource[] = [
  {
    source: "mtgpics",
    copy: null,
    site: null,
    host: null,
    size: { width: 1600, height: 1168 },
    current: false,
  },
  {
    source: "scryfall",
    copy: null,
    site: null,
    host: null,
    size: { width: 626, height: 457 },
    current: false,
  },
  {
    source: "web",
    copy: 7,
    site: "lens",
    host: "artstation.com",
    size: { width: 2400, height: 1752 },
    current: true,
  },
  {
    source: "web",
    copy: 8,
    site: "instagram",
    host: "instagram.com",
    size: { width: 1440, height: 1051 },
    current: false,
  },
];

/** A found copy replaced the art by mistake: a right-click anywhere on the row
 * offers the other places its art is known to be, and saves it from one. */
export const SaveArtFromAnotherSource: Story = {
  beforeEach: () => {
    useRun.setState({
      runId: 3,
      jobs: finishedJobs.map((job) =>
        job.id === 2
          ? { ...job, status: { state: "done", files: [foundBoltFile], missing: [] } }
          : job,
      ),
      summary: finishedSummary,
    });
    mocked(api.artSources).mockResolvedValue(boltSources);
    mocked(api.saveArtFrom).mockResolvedValue({
      ...boltFile,
      path: boltFile.path.replace("Scryfall", "MTGPics"),
      source: "mtgpics",
      size: { width: 1600, height: 1168 },
    });
  },
  play: async ({ canvas }) => {
    // The resolution cell, away from the file's own line.
    await userEvent.pointer({ keys: "[MouseRight]", target: canvas.getByText("2400 × 1752") });
    await expect(api.artSources).toHaveBeenCalledWith(3, 2, 0);
    // Each with its size.
    const mtgpics = await screen.findByRole("menuitem", { name: "MTGPics · 1600 × 1168" });
    await waitFor(() => expect(mtgpics).toBeVisible());
    await waitFor(() =>
      expect(screen.getByRole("menuitem", { name: "Scryfall · 626 × 457" })).toBeVisible(),
    );
    // Each found copy, with its site, size and where it was found from; the
    // one saved now can't be chosen again.
    await expect(
      screen.getByRole("menuitem", { name: /artstation\.com · 2400 × 1752 \(saved now\)/ }),
    ).toHaveAttribute("aria-disabled", "true");
    const instagram = screen.getByRole("menuitem", { name: /instagram\.com · 1440 × 1051/ });
    await expect(instagram).toHaveTextContent("Found from Instagram");
    await expect(instagram).not.toHaveAttribute("aria-disabled");

    await userEvent.click(mtgpics);
    await expect(api.saveArtFrom).toHaveBeenCalledWith(3, 2, 0, "mtgpics", null);
    await waitFor(() =>
      expect(screen.getByText("Saved Lightning Bolt's art from MTGPics")).toBeVisible(),
    );
    await expect(canvas.getByText("1600 × 1168")).toBeVisible();
    await expect(canvas.queryByText("Found")).not.toBeInTheDocument();
  },
};

/** A two-faced card's row offers each face's sources, under the face's name. */
export const SourcesForEachFace: Story = {
  beforeEach: () => {
    const front = { ...boltFile, face: "Delver of Secrets", artist: "Matt Stewart" };
    const back = {
      ...front,
      face: "Insectile Aberration",
      faceIndex: 1,
      path: `${boltFile.path}-back`,
    };
    useRun.setState({
      runId: 3,
      jobs: [
        {
          id: 1,
          label: "Delver of Secrets",
          line: "Delver of Secrets",
          status: { state: "done", files: [front, back], missing: [] },
        },
      ],
      summary: finishedSummary,
    });
    mocked(api.artSources).mockResolvedValue([
      { source: "scryfall", copy: null, site: null, host: null, size: null, current: true },
    ]);
  },
  play: async ({ canvas, canvasElement }) => {
    // Each face has its own thumbnail and a line twice as tall as one card's.
    await expect(canvasElement.querySelectorAll("img")).toHaveLength(2);
    const sizes = canvas.getAllByTitle("Width × height, in pixels");
    await expect(sizes).toHaveLength(2);
    await expect(sizes[0]?.getBoundingClientRect().height).toBe(48);
    await userEvent.pointer({ keys: "[MouseRight]", target: canvas.getByText("Done") });
    await waitFor(() =>
      expect(screen.getByText("Insectile Aberration: use the art from")).toBeVisible(),
    );
    await expect(screen.getByText("Delver of Secrets: use the art from")).toBeVisible();
    await expect(api.artSources).toHaveBeenCalledWith(3, 1, 0);
    await expect(api.artSources).toHaveBeenCalledWith(3, 1, 1);
    await expect(screen.getAllByRole("menuitem", { name: "Show in folder" })).toHaveLength(2);
  },
};

/** The search can't open: the error shows as a toast. */
export const FindLargerCopyFails: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.findLargerCopy).mockRejectedValue("Scryfall has no art crop of Lightning Bolt");
  },
  play: async ({ canvas }) => {
    await userEvent.click(
      canvas.getByRole("button", { name: "Search Google Lens for this art, for Lightning Bolt" }),
    );
    await waitFor(() => expect(screen.getByText("Couldn't open the search")).toBeVisible());
  },
};
