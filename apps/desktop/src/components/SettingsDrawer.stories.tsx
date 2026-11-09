import type { Meta, StoryObj } from "@storybook/react-vite";
import { open } from "@tauri-apps/plugin-dialog";
import { expect, mocked, screen, userEvent, waitFor, within } from "storybook/test";

import { cardData } from "@/__fixtures__/run";
import { chosenCardAnvil, defaultSettings, installedCardAnvil } from "@/__fixtures__/settings";
import { api } from "@/api";
import { useSettings } from "@/store";

import { SettingsDrawer } from "./SettingsDrawer";

const meta = {
  component: SettingsDrawer,
} satisfies Meta<typeof SettingsDrawer>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Opens the drawer; it renders in a portal, so queries go through `screen`. */
async function openDrawer(canvas: { getByRole: typeof screen.getByRole }) {
  await userEvent.click(canvas.getByRole("button", { name: "Settings" }));
  const drawer = await screen.findByRole("dialog", { name: "Settings" });
  // It slides in; wait for it to finish.
  await waitFor(() => expect(drawer).toBeVisible());
  return drawer;
}

/** One of the drawer's sections, by its legend, to query inside. */
function section(legend: string) {
  return within(screen.getByRole("group", { name: legend }));
}

/** The drawer shows the current settings. */
export const Default: Story = {
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await expect(screen.getByLabelText("File names")).toHaveValue(defaultSettings.naming);
    await expect(
      screen.getByText(/Karn, Scion of Urza \(Chase Stone\) \[DOM\] \{1\}\.jpg/),
    ).toBeVisible();
    await expect(
      screen.getByRole("checkbox", { name: "Replace files with the same name" }),
    ).toBeChecked();
    await expect(
      screen.getByRole("checkbox", { name: "A folder for each card type" }),
    ).not.toBeChecked();
  },
};

/** How current the local copy of Scryfall's card data is. */
export const CardDataCurrent: Story = {
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await waitFor(() =>
      expect(screen.getByText(/112,345 cards from Scryfall, published/)).toBeVisible(),
    );
  },
};

/** The first download of the card data is under way. */
export const CardDataDownloading: Story = {
  beforeEach: () => {
    mocked(api.cardDataStatus).mockResolvedValue({
      updatedAt: null,
      cards: 0,
      updating: true,
      error: null,
    });
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await waitFor(() => expect(screen.getByText(/Downloading Scryfall's card data/)).toBeVisible());
  },
};

/** An update that failed says why; the older data stays in use. */
export const CardDataUpdateFailed: Story = {
  beforeEach: () => {
    mocked(api.cardDataStatus).mockResolvedValue({
      ...cardData,
      error: "Scryfall's bulk data is unavailable",
    });
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await waitFor(() =>
      expect(
        screen.getByText("The last update failed: Scryfall's bulk data is unavailable"),
      ).toBeVisible(),
    );
  },
};

/** Settings load before the app shows them; until then the button is disabled. */
export const NotLoaded: Story = {
  beforeEach: () => {
    useSettings.setState({ settings: null });
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("button", { name: "Settings" })).toBeDisabled();
  },
};

/** Each change is saved once typing or clicking pauses. */
export const ChangeSettings: Story = {
  play: async ({ canvas }) => {
    await openDrawer(canvas);

    await userEvent.click(screen.getByRole("checkbox", { name: "A folder for each card type" }));
    await userEvent.click(
      screen.getByRole("checkbox", { name: "Search for larger copies of MTGPics' art too" }),
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Where art comes from" }),
      "Scryfall only",
    );
    await userEvent.click(screen.getByRole("radio", { name: "Oldest" }));

    const naming = screen.getByLabelText("File names");
    await userEvent.clear(naming);
    await userEvent.type(naming, "NAME - SET");
    await expect(
      screen.getByText("For example: Karn, Scion of Urza - DOM.jpg", { exact: false }),
    ).toBeVisible();

    await waitFor(() =>
      expect(api.saveSettings).toHaveBeenLastCalledWith({
        ...defaultSettings,
        typeFolders: true,
        searchMtgpicsArt: true,
        sources: "scryfallOnly",
        printing: "oldest",
        naming: "NAME - SET",
      }),
    );
  },
};

/** "Choose…" asks for a folder and saves it. */
export const ChooseOutputFolder: Story = {
  beforeEach: () => {
    mocked(open).mockResolvedValue("/home/user/Art");
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await userEvent.click(section("Files").getByRole("button", { name: /choose/i }));
    await waitFor(() => expect(screen.getByLabelText("Save art to")).toHaveValue("/home/user/Art"));
    await waitFor(() =>
      expect(api.saveSettings).toHaveBeenCalledWith({
        ...defaultSettings,
        outputDir: "/home/user/Art",
      }),
    );
  },
};

/** A save the Rust side rejects is reported. */
export const SaveFails: Story = {
  beforeEach: () => {
    mocked(api.saveSettings).mockRejectedValue("Permission denied");
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await userEvent.click(screen.getByRole("checkbox", { name: "Skip full-art printings" }));
    await waitFor(() => expect(screen.getByText("Couldn't save settings")).toBeVisible());
  },
};

/** Card Anvil, found where its installer put it. */
export const CardAnvilFound: Story = {
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await waitFor(() =>
      expect(screen.getByLabelText("Card Anvil app")).toHaveValue(installedCardAnvil.path),
    );
    await expect(api.cardAnvilStatus).toHaveBeenCalledWith(null);
    await expect(screen.getByText(/Found automatically/)).toBeVisible();
    // Nothing chosen, so nothing to go back from.
    await expect(
      screen.queryByRole("button", { name: "Use the installed one" }),
    ).not.toBeInTheDocument();
    await expect(
      screen.getByRole("checkbox", { name: "Send each download to Card Anvil" }),
    ).not.toBeChecked();
  },
};

/** Card Anvil isn't installed: the field says so, and links to its download page. */
export const CardAnvilNotFound: Story = {
  beforeEach: () => {
    mocked(api.cardAnvilStatus).mockResolvedValue({ chosen: false, chosenMissing: false });
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    const app = screen.getByLabelText("Card Anvil app");
    await waitFor(() => expect(app).toHaveAttribute("placeholder", "Not found"));
    await expect(app).toHaveValue("");
    await userEvent.click(screen.getByRole("button", { name: "Get Card Anvil" }));
    await expect(api.openCardAnvilSite).toHaveBeenCalled();
  },
};

/** "Choose…" picks another copy of Card Anvil, such as a development build, and saves it. */
export const ChooseCardAnvil: Story = {
  beforeEach: () => {
    mocked(open).mockResolvedValue(chosenCardAnvil.path ?? null);
    mocked(api.cardAnvilStatus).mockImplementation(async (path) =>
      path === null ? installedCardAnvil : chosenCardAnvil,
    );
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await userEvent.click(section("Card Anvil").getByRole("button", { name: /choose/i }));
    await expect(open).toHaveBeenCalledWith(expect.objectContaining({ directory: false }));
    await waitFor(() =>
      expect(screen.getByLabelText("Card Anvil app")).toHaveValue(chosenCardAnvil.path),
    );
    await expect(api.cardAnvilStatus).toHaveBeenLastCalledWith(chosenCardAnvil.path);
    await expect(screen.getByRole("button", { name: "Use the installed one" })).toBeVisible();
    await waitFor(() =>
      expect(api.saveSettings).toHaveBeenCalledWith({
        ...defaultSettings,
        cardAnvilPath: chosenCardAnvil.path,
      }),
    );
  },
};

/** "Use the installed one" forgets the copy chosen, and finds the installed one again. */
export const ResetCardAnvil: Story = {
  beforeEach: () => {
    useSettings.setState({
      settings: { ...defaultSettings, cardAnvilPath: chosenCardAnvil.path },
    });
    mocked(api.cardAnvilStatus).mockImplementation(async (path) =>
      path === null ? installedCardAnvil : chosenCardAnvil,
    );
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    const app = screen.getByLabelText("Card Anvil app");
    await waitFor(() => expect(app).toHaveValue(chosenCardAnvil.path));
    await userEvent.click(screen.getByRole("button", { name: "Use the installed one" }));
    await waitFor(() => expect(app).toHaveValue(installedCardAnvil.path));
    await expect(
      screen.queryByRole("button", { name: "Use the installed one" }),
    ).not.toBeInTheDocument();
    await waitFor(() => expect(api.saveSettings).toHaveBeenLastCalledWith(defaultSettings));
  },
};

/** The copy chosen is gone: the installed one is used, and the drawer says why. */
export const ChosenCardAnvilGone: Story = {
  beforeEach: () => {
    useSettings.setState({
      settings: { ...defaultSettings, cardAnvilPath: "/home/user/old/card-anvil" },
    });
    mocked(api.cardAnvilStatus).mockResolvedValue({ ...installedCardAnvil, chosenMissing: true });
  },
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await waitFor(() =>
      expect(screen.getByText(/The copy you chose isn't there any more/)).toBeVisible(),
    );
    await expect(screen.getByLabelText("Card Anvil app")).toHaveValue(installedCardAnvil.path);
    await expect(screen.getByRole("button", { name: "Use the installed one" })).toBeVisible();
  },
};

/** Each download can be sent to Card Anvil as it finishes. */
export const ToggleAutoSend: Story = {
  play: async ({ canvas }) => {
    await openDrawer(canvas);
    await userEvent.click(
      screen.getByRole("checkbox", { name: "Send each download to Card Anvil" }),
    );
    await waitFor(() =>
      expect(api.saveSettings).toHaveBeenLastCalledWith({
        ...defaultSettings,
        autoSendToCardAnvil: true,
      }),
    );
  },
};
