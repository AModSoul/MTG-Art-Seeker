import type { Meta, StoryObj } from "@storybook/react-vite";
import { open } from "@tauri-apps/plugin-dialog";
import { expect, mocked, screen, userEvent, waitFor } from "storybook/test";

import { defaultSettings } from "@/__fixtures__/settings";
import { api } from "@/api";
import type { ParsedList } from "@/bindings/ParsedList";
import { useInput, useRun, useSettings } from "@/store";

import { InputPanel } from "./InputPanel";

const LIST = "Lightning Bolt\nKarn, Scion of Urza (DOM) 1";

const meta = {
  component: InputPanel,
  decorators: [
    (Story) => (
      <div style={{ height: "640px", maxWidth: "400px" }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof InputPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Nothing to download yet, so the download button is disabled. */
export const Empty: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("tab", { name: "Card list" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(canvas.getByText("No cards yet")).toBeVisible();
    await expect(canvas.getByRole("button", { name: /download art/i })).toBeDisabled();
    await expect(canvas.getByText(defaultSettings.outputDir)).toBeVisible();
  },
};

/** Typing a list has the Rust parser read it, then the list can be downloaded. */
export const CardList: Story = {
  beforeEach: () => {
    mocked(api.parseCardList).mockImplementation(async (text): Promise<ParsedList> => ({
      cards: text
        .split("\n")
        .filter((line) => line.trim() && !line.startsWith("//"))
        .map((line) => ({ line, name: line, set: null, number: null })),
      skipped: text
        .split("\n")
        .filter((line) => line.startsWith("//"))
        .map((line) => ({ line, reason: "comment" })),
    }));
  },
  play: async ({ canvas }) => {
    await userEvent.type(canvas.getByRole("textbox"), `// Burn{enter}${LIST}`);
    await expect(await canvas.findByText("2 cards · 1 heading or comment skipped")).toBeVisible();
    await expect(useInput.getState().text).toBe(`// Burn\n${LIST}`);

    await userEvent.click(canvas.getByRole("button", { name: /download art/i }));
    await expect(api.startRun).toHaveBeenCalledWith(
      { kind: "list", text: `// Burn\n${LIST}`, folder: "" },
      defaultSettings,
      expect.any(Function),
    );
  },
};

/** "Clear" empties the list; with nothing in it, there's nothing to clear. */
export const ClearList: Story = {
  beforeEach: () => {
    useInput.setState({ text: LIST });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("textbox")).toHaveValue(LIST);
    await userEvent.click(canvas.getByRole("button", { name: "Clear" }));
    await expect(canvas.getByRole("textbox")).toHaveValue("");
    await expect(useInput.getState().text).toBe("");
    await expect(await canvas.findByText("No cards yet")).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Clear" })).not.toBeInTheDocument();
  },
};

/** "Open file…" reads the chosen list through the Rust side, and names its folder after the file. */
export const OpenListFile: Story = {
  beforeEach: () => {
    mocked(open).mockResolvedValue("/home/user/decks/burn.txt");
    mocked(api.readCardList).mockResolvedValue({ text: LIST, name: "burn" });
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: /open file/i }));
    await expect(api.readCardList).toHaveBeenCalledWith("/home/user/decks/burn.txt");
    await waitFor(() => expect(canvas.getByRole("textbox")).toHaveValue(LIST));
    await expect(useInput.getState().folder).toBe("burn");
  },
};

/** With "A folder for each card list" on, the list's folder can be named, and the run saves there. */
export const ListFolders: Story = {
  beforeEach: () => {
    useSettings.setState({ settings: { ...defaultSettings, listFolders: true } });
    useInput.setState({ text: LIST });
    mocked(api.parseCardList).mockResolvedValue({
      cards: [
        { line: "Lightning Bolt", name: "Lightning Bolt", set: null, number: null },
        {
          line: "Karn, Scion of Urza (DOM) 1",
          name: "Karn, Scion of Urza",
          set: "dom",
          number: "1",
        },
      ],
      skipped: [],
    });
  },
  play: async ({ canvas }) => {
    const folder = canvas.getByRole("textbox", { name: /folder for this list/i });
    await userEvent.type(folder, "Burn deck");
    await expect(useInput.getState().folder).toBe("Burn deck");

    const download = canvas.getByRole("button", { name: /download art/i });
    await waitFor(() => expect(download).toBeEnabled());
    await userEvent.click(download);
    await expect(api.startRun).toHaveBeenCalledWith(
      { kind: "list", text: LIST, folder: "Burn deck" },
      { ...defaultSettings, listFolders: true },
      expect.any(Function),
    );
  },
};

/** The folder field only shows while the setting is on. */
export const NoListFolders: Story = {
  play: async ({ canvas }) => {
    await expect(
      canvas.queryByRole("textbox", { name: /folder for this list/i }),
    ).not.toBeInTheDocument();
  },
};

/** A list that can't be read shows why. */
export const OpenListFileFails: Story = {
  beforeEach: () => {
    mocked(open).mockResolvedValue("/home/user/decks/burn.txt");
    mocked(api.readCardList).mockRejectedValue("The file isn't text");
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: /open file/i }));
    await waitFor(() => expect(screen.getByText("Couldn't open the list")).toBeVisible());
  },
};

/** A Scryfall search, with its match count. */
export const Search: Story = {
  beforeEach: () => {
    mocked(api.countSearch).mockResolvedValue(1234);
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("tab", { name: "Search" }));
    const query = canvas.getByRole("textbox", { name: "Scryfall search" });
    await userEvent.type(query, "set:mh2 t:creature{enter}");
    await expect(api.countSearch).toHaveBeenCalledWith("set:mh2 t:creature", defaultSettings);
    await expect(await canvas.findByText("1,234 cards match")).toBeVisible();

    await userEvent.click(canvas.getByRole("button", { name: /download art/i }));
    await expect(api.startRun).toHaveBeenCalledWith(
      { kind: "search", query: "set:mh2 t:creature" },
      defaultSettings,
      expect.any(Function),
    );
  },
};

/** Picking an EDHREC preset. */
export const Preset: Story = {
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("tab", { name: "Presets" }));
    await userEvent.click(await canvas.findByRole("radio", { name: "Saltiest 100" }));
    await expect(useInput.getState().presetId).toBe("edhrec-salt");

    await userEvent.click(canvas.getByRole("button", { name: /download art/i }));
    await expect(api.startRun).toHaveBeenCalledWith(
      { kind: "preset", id: "edhrec-salt" },
      defaultSettings,
      expect.any(Function),
    );
  },
};

/** While a run is going, the download button becomes Cancel. */
export const Running: Story = {
  beforeEach: () => {
    useRun.setState({ running: true, runId: 7 });
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: /cancel/i }));
    await expect(api.cancelRun).toHaveBeenCalledWith(7);
  },
};

/** Choosing another output folder saves it to the settings. */
export const ChooseOutputFolder: Story = {
  beforeEach: () => {
    mocked(open).mockResolvedValue("/home/user/Art");
  },
  play: async ({ canvas }) => {
    await userEvent.click(
      canvas.getByRole("button", { name: "Choose the folder art is saved to" }),
    );
    await waitFor(() => expect(useSettings.getState().settings?.outputDir).toBe("/home/user/Art"));
    await expect(canvas.getByText("/home/user/Art")).toBeVisible();
  },
};
