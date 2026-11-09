import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, mocked, screen, userEvent, waitFor, within } from "storybook/test";

import {
  boltFile,
  delverFile,
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
import { type JobView, useRun, useSettings } from "@/store";

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

/** A run has started but the Rust side hasn't planned its jobs yet: the table
 * holds placeholder rows, so nothing moves when the jobs arrive. */
export const Planning: Story = {
  beforeEach: () => {
    useRun.setState({ running: true, runId: 1 });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Getting the list ready…")).toBeVisible();
    await expect(canvas.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    await expect(canvas.getByRole("table")).toHaveAttribute("aria-busy", "true");
    await expect(canvas.getAllByRole("row")).toHaveLength(7);
    await expect(canvas.getByRole("radio", { name: "All 0" })).toBeDisabled();
  },
};

/** Mid-run, with a job in every state. Failed jobs can't be retried until the run ends. */
export const Running: Story = {
  beforeEach: () => {
    useRun.setState({ running: true, runId: 1, jobs: mixedJobs, skipped: skippedLines });
  },
  play: async ({ canvas, canvasElement }) => {
    await expect(canvas.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "4");
    await expect(canvas.getByText("4 of 7 cards")).toBeVisible();
    await expect(canvas.getByText(/Skipped 2 lines that aren't cards/)).toBeVisible();
    await expect(canvas.queryByRole("button", { name: /retry failed/i })).not.toBeInTheDocument();
    // Every status, the longest with its spinner, fits inside its column's padding.
    const badges = canvasElement.querySelectorAll("td .chakra-badge");
    await expect(badges).toHaveLength(mixedJobs.length);
    for (const badge of badges) {
      const cell = badge.closest("td")?.getBoundingClientRect();
      await expect(badge.getBoundingClientRect().right).toBeLessThanOrEqual((cell?.right ?? 0) - 8);
    }
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
    await expect(canvas.queryByRole("row", { name: /Lightning Bolt/ })).not.toBeInTheDocument();

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

/** Where each saved image came from, and its size in pixels, or a dash when
 * its header couldn't be read. */
export const ImageColumn: Story = {
  beforeEach: () => {
    useRun.setState({ jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("columnheader", { name: "Image" })).toBeVisible();
    await expect(canvas.getAllByText("MTGPics")).toHaveLength(1);
    await expect(canvas.getAllByText("Scryfall")).toHaveLength(5);
    await expect(canvas.getByText("1920 × 1408")).toBeVisible();
    await expect(canvas.getByText("626 × 457")).toBeVisible();
    // Sol Ring, Damnation and Counterspell finish with an image of unknown size.
    await expect(canvas.getAllByText("—")).toHaveLength(3);
  },
};

/** Clicking a saved file or its thumbnail shows it in its folder, and so does
 * Enter on either; "Open folder" opens the output folder. */
export const RevealFiles: Story = {
  beforeEach: () => {
    useRun.setState({ jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    // Stacked on two lines, the name has a space more either side of the dot.
    const karn = /^Karn, Scion of Urza\s+·\s+Chase Stone$/;
    await userEvent.click(canvas.getByRole("button", { name: karn }));
    await expect(api.revealFile).toHaveBeenCalledWith(karnFile.path);

    mocked(api.revealFile).mockClear();
    await userEvent.click(
      canvas.getByRole("button", { name: "Show Karn, Scion of Urza's art in its folder" }),
    );
    await expect(api.revealFile).toHaveBeenCalledWith(karnFile.path);

    mocked(api.revealFile).mockClear();
    canvas.getByRole("button", { name: karn }).focus();
    await userEvent.keyboard("{Enter}");
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
    await expect(canvas.getByRole("button", { name: /^Lightning Bolt · / })).toBeVisible();
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

/** Found art is named as such, and links to the page it came from. */
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
          images: 2,
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
    // Each face has its own thumbnail, on a line as tall as it.
    await expect(canvasElement.querySelectorAll("img")).toHaveLength(2);
    // A size, or a dash for one unknown, on a line of its own per face.
    const sizes = canvas.getAllByText(/^(\d+ × \d+|—)$/);
    await expect(sizes).toHaveLength(2);
    await expect(sizes[0]?.parentElement?.parentElement?.getBoundingClientRect().height).toBe(48);
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

/** Karn before its art is saved. */
const karnJob: JobView = {
  id: 1,
  label: "Karn, Scion of Urza (DOM) 1",
  line: "",
  images: 1,
  status: { state: "queued" },
};

/** Delver of Secrets, with its two faces, as its art is being saved. */
const delverJob: JobView = {
  id: 2,
  label: "Delver of Secrets (ISD) 51",
  line: "",
  images: 2,
  status: { state: "downloading", images: 2 },
};

/** How many lines of text an element takes up. */
function lineCount(element: HTMLElement): number {
  return Math.round(element.offsetHeight / parseFloat(getComputedStyle(element).lineHeight));
}

/** With room, every saved file's card and artist share a line. */
export const OneLineEach: Story = {
  decorators: [
    (Story) => (
      <div style={{ width: "1200px" }}>
        <Story />
      </div>
    ),
  ],
  beforeEach: () => {
    useRun.setState({ jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    const buttons = canvas.getAllByRole("button", { name: / · / });
    await expect(buttons.length).toBeGreaterThan(1);
    for (const button of buttons) {
      await waitFor(() => expect(lineCount(button)).toBe(1));
    }
  },
};

/** A card with the longest name there is, as a list names it. */
const LONG_LABEL = "Avacyn's Collar, the Symbol of Her Church (SLD) 1234";

/** Its saved art: a long card name by a short artist credit. */
const longFaceFile = { ...boltFile, face: "Avacyn's Collar, the Symbol of Her Church" };

/** A short card name by an artist credit as long. */
const longArtistFile = {
  ...boltFile,
  path: `${boltFile.path}.2`,
  artist: "Christopher Moeller & Rebecca Guay-Mitchell",
};

/** The longest card name there is, and an artist credit as long, in a narrow
 * window: every row, even one whose names are short, puts its card and artist on
 * a line each, the Saved column keeps room for each name, and the table scrolls
 * sideways rather than cut either short. The Card column keeps room for its
 * longest name on two lines. */
export const LongNames: Story = {
  decorators: [
    (Story) => (
      <div style={{ width: "600px" }}>
        <Story />
      </div>
    ),
  ],
  beforeEach: () => {
    useRun.setState({
      runId: 1,
      jobs: [
        {
          ...karnJob,
          label: LONG_LABEL,
          status: { state: "done", files: [longFaceFile], missing: [] },
        },
        { ...delverJob, status: { state: "done", files: [longArtistFile], missing: [] } },
      ],
    });
  },
  play: async ({ canvas }) => {
    // The card's name: two lines, none cut off below them.
    const cell = canvas.getByRole("cell", { name: LONG_LABEL });
    const label = within(cell).getByRole("paragraph");
    await waitFor(() => expect(lineCount(label)).toBe(2));
    await expect(label.scrollHeight).toBeLessThanOrEqual(label.clientHeight);

    // Each saved file's card and artist: a line each, neither cut short.
    for (const file of [longFaceFile, longArtistFile]) {
      const button = canvas.getByRole("button", { name: new RegExp(`^${file.face}\\s+·`) });
      await waitFor(() => expect(lineCount(button)).toBe(2));
      for (const text of [file.face, file.artist]) {
        const line = within(button).getByText(text, { exact: true });
        await expect(line.scrollWidth).toBeLessThanOrEqual(line.clientWidth);
      }
    }
  },
};

/** A row is as tall as it will be once its art is saved: a line per image the
 * job says it will save, held by placeholders until each arrives. */
export const RowsKeepTheirHeight: Story = {
  beforeEach: () => {
    useRun.setState({ running: true, runId: 1, jobs: [karnJob, delverJob] });
  },
  play: async ({ canvas }) => {
    const heights = () =>
      [/Karn/, /Delver/].map(
        (name) => canvas.getByRole("row", { name }).getBoundingClientRect().height,
      );
    const before = heights();
    // Two faces make a row taller than one.
    await expect(before[1]).toBeGreaterThan(before[0] ?? 0);

    // Karn is saved, and Delver's back is missing.
    useRun.setState({
      jobs: [
        { ...karnJob, status: { state: "done", files: [karnFile], missing: [] } },
        {
          ...delverJob,
          status: { state: "done", files: [delverFile], missing: ["Insectile Aberration"] },
        },
      ],
    });
    await waitFor(() =>
      expect(canvas.getByText("No art found for Insectile Aberration")).toBeVisible(),
    );
    await expect(heights()).toEqual(before);
  },
};

/** The button that sends the run's art to Card Anvil. */
const sendButton = (canvas: { getByRole: typeof screen.getByRole }) =>
  canvas.getByRole("button", { name: "Send to Card Anvil" });

/** "Send to Card Anvil" hands the run's art over, and waits while Card Anvil takes it. */
export const SendToCardAnvil: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
  },
  play: async ({ canvas }) => {
    let deliver: (() => void) | undefined;
    mocked(api.sendToCardAnvil).mockReturnValue(
      new Promise((resolve) => {
        deliver = () => resolve({ outcome: "delivered", images: 6 });
      }),
    );
    await userEvent.click(sendButton(canvas));
    await expect(api.sendToCardAnvil).toHaveBeenCalledWith(3, null, defaultSettings);
    // One send at a time.
    await waitFor(() => expect(screen.getByText("Sending the art to Card Anvil…")).toBeVisible());
    await expect(sendButton(canvas)).toBeDisabled();

    deliver?.();
    await waitFor(() => expect(screen.getByText("Sent 6 images to Card Anvil")).toBeVisible());
    await expect(screen.getByText("Card Anvil will ask where to put them.")).toBeVisible();
    await waitFor(() => expect(sendButton(canvas)).toBeEnabled());
  },
};

/** Card Anvil isn't installed: the toast says so, and offers its download page. */
export const CardAnvilNotFound: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.sendToCardAnvil).mockResolvedValue({ outcome: "notFound" });
  },
  play: async ({ canvas }) => {
    await userEvent.click(sendButton(canvas));
    await waitFor(() => expect(screen.getByText("Card Anvil isn't installed")).toBeVisible());
    await expect(screen.getByText("Install it, or choose where it is in Settings.")).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "Get Card Anvil" }));
    await expect(api.openCardAnvilSite).toHaveBeenCalled();
  },
};

/** Card Anvil started but didn't take the art: it's too old to know how. */
export const CardAnvilNotTaken: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.sendToCardAnvil).mockResolvedValue({ outcome: "notTaken", images: 6 });
  },
  play: async ({ canvas }) => {
    await userEvent.click(sendButton(canvas));
    await waitFor(() => expect(screen.getByText("Card Anvil didn't take the art")).toBeVisible());
    await expect(
      screen.getByText("This version is too old. Update it, then send again."),
    ).toBeVisible();
    await expect(screen.getByRole("button", { name: "Get Card Anvil" })).toBeVisible();
  },
};

/** More cards than Card Anvil takes at once: the toast says to send a few at a time. */
export const TooManyForCardAnvil: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.sendToCardAnvil).mockResolvedValue({ outcome: "tooMany", images: 1204, max: 1000 });
  },
  play: async ({ canvas }) => {
    await userEvent.click(sendButton(canvas));
    await waitFor(() => expect(screen.getByText("Too many images for Card Anvil")).toBeVisible());
    await expect(screen.getByText(/that's 1,204 images/)).toBeVisible();
    await expect(screen.getByText(/right-click a card/)).toBeVisible();
  },
};

/** Card Anvil couldn't be started: the error shows as a toast. */
export const SendToCardAnvilFails: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.sendToCardAnvil).mockRejectedValue("Couldn't start Card Anvil: access denied");
  },
  play: async ({ canvas }) => {
    await userEvent.click(sendButton(canvas));
    await waitFor(() =>
      expect(screen.getByText("Couldn't send the art to Card Anvil")).toBeVisible(),
    );
    await expect(screen.getByText("Couldn't start Card Anvil: access denied")).toBeVisible();
    await waitFor(() => expect(sendButton(canvas)).toBeEnabled());
  },
};

/** A row's right-click menu sends that card's art alone. */
export const SendARowToCardAnvil: Story = {
  beforeEach: () => {
    useRun.setState({ runId: 3, jobs: finishedJobs, summary: finishedSummary });
    mocked(api.sendToCardAnvil).mockResolvedValue({ outcome: "delivered", images: 1 });
  },
  play: async ({ canvas }) => {
    await userEvent.pointer({ keys: "[MouseRight]", target: canvas.getByText("626 × 457") });
    const send = await screen.findByRole("menuitem", { name: "Send to Card Anvil" });
    await waitFor(() => expect(send).toBeVisible());
    await userEvent.click(send);
    await expect(api.sendToCardAnvil).toHaveBeenCalledWith(3, [2], defaultSettings);
    await waitFor(() => expect(screen.getByText("Sent 1 image to Card Anvil")).toBeVisible());
    await expect(screen.getByText("Card Anvil will ask where to put it.")).toBeVisible();
  },
};

/** Nothing is sent while the run is still going, or before it has saved any art. */
export const SendToCardAnvilAfterTheRun: Story = {
  beforeEach: () => {
    useRun.setState({ running: true, runId: 1, jobs: mixedJobs });
  },
  play: async ({ canvas }) => {
    await expect(sendButton(canvas)).toBeDisabled();

    useRun.setState({
      running: false,
      jobs: mixedJobs.map((job) => ({ ...job, status: { state: "cancelled" } })),
    });
    await waitFor(() => expect(canvas.getAllByText("Cancelled").length).toBeGreaterThan(1));
    await expect(sendButton(canvas)).toBeDisabled();
  },
};
