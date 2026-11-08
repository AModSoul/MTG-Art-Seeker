import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, mocked, userEvent, waitFor } from "storybook/test";

import { abzanArmor, allDecks, eocDecks, worldShaper } from "@/__fixtures__/decks";
import { api } from "@/api";
import type { DeckSummary } from "@/bindings/DeckSummary";
import { useInput } from "@/store";

import { DECK_PAGE, DeckPicker } from "./DeckPicker";

const meta = {
  component: DeckPicker,
  decorators: [
    (Story) => (
      <div style={{ height: "640px", maxWidth: "360px", display: "flex", flexDirection: "column" }}>
        <Story />
      </div>
    ),
  ],
  beforeEach: () => {
    // Decks with a known box show a picture of it; the others, an icon.
    mocked(api.deckBox).mockImplementation(async (fileName) =>
      allDecks.decks.some((deck) => deck.fileName === fileName && deck.hasBox)
        ? `/boxes/${fileName}.png`
        : null,
    );
  },
} satisfies Meta<typeof DeckPicker>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Every deck, newest first, with its box and its set under its name. */
export const AllDecks: Story = {
  play: async ({ canvas }) => {
    await expect(await canvas.findByText("4 decks")).toBeVisible();
    await expect(api.searchDecks).toHaveBeenCalledWith("", DECK_PAGE);
    await expect(canvas.getByText("Abzan Armor")).toBeVisible();
    await expect(canvas.getByText("Tarkir: Dragonstorm Commander")).toBeVisible();
    await expect(await canvas.findByAltText("The box of Abzan Armor")).toBeVisible();
    await expect(canvas.queryByAltText("The box of Arzakon's Minions")).not.toBeInTheDocument();
    await expect(canvas.queryByRole("button", { name: "Show more" })).not.toBeInTheDocument();
  },
};

/** Typing a search asks the core for the decks it matches. */
export const SearchBySet: Story = {
  beforeEach: () => {
    mocked(api.searchDecks).mockImplementation(async (query) =>
      query === "set:eoc" ? eocDecks : allDecks,
    );
  },
  play: async ({ canvas }) => {
    await userEvent.type(
      canvas.getByRole("textbox", { name: "Search the precon decks" }),
      "set:eoc",
    );
    await expect(await canvas.findByText("2 decks")).toBeVisible();
    await expect(api.searchDecks).toHaveBeenLastCalledWith("set:eoc", DECK_PAGE);
    await expect(canvas.getByText("World Shaper")).toBeVisible();
    await expect(canvas.queryByText("Abzan Armor")).not.toBeInTheDocument();
    await expect(useInput.getState().deckQuery).toBe("set:eoc");
  },
};

/** Clicking a deck picks it; the line under the list says which, and unpicks it. */
export const PickADeck: Story = {
  play: async ({ canvas }) => {
    const tile = await canvas.findByRole("button", { name: /Abzan Armor/ });
    await expect(tile).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(tile);
    await expect(tile).toHaveAttribute("aria-pressed", "true");
    await expect(useInput.getState().deck).toEqual({
      fileName: abzanArmor.fileName,
      name: abzanArmor.name,
      code: abzanArmor.code,
      setName: abzanArmor.setName,
    });
    await expect(canvas.getByText("Picked")).toBeVisible();

    await userEvent.click(canvas.getByRole("button", { name: "Unpick the deck" }));
    await expect(tile).toHaveAttribute("aria-pressed", "false");
    await expect(useInput.getState().deck).toBeNull();
  },
};

/** A deck picked before stays picked when a search hides it. */
export const PickedButHidden: Story = {
  beforeEach: () => {
    mocked(api.searchDecks).mockResolvedValue(eocDecks);
    useInput.setState({ deckQuery: "set:eoc", deck: abzanArmor });
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText("2 decks")).toBeVisible();
    await expect(canvas.getByText("Picked")).toBeVisible();
    await expect(canvas.getByTitle("Abzan Armor · Tarkir: Dragonstorm Commander")).toBeVisible();
  },
};

/** More decks than are shown: "Show more" asks for the next lot. */
export const ShowMore: Story = {
  beforeEach: () => {
    const many: DeckSummary[] = Array.from({ length: 100 }, (_, index) => ({
      ...worldShaper,
      fileName: `Deck${index}_SLD`,
      name: `Secret Lair Drop ${index + 1}`,
      code: "SLD",
      setName: "Secret Lair Drop",
      type: "Secret Lair Drop",
      hasBox: false,
    }));
    mocked(api.searchDecks).mockImplementation(async (_query, limit) => ({
      decks: many.slice(0, limit),
      total: many.length,
    }));
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText("100 decks")).toBeVisible();
    await expect(canvas.queryByText(`Secret Lair Drop ${DECK_PAGE + 1}`)).not.toBeInTheDocument();
    await userEvent.click(canvas.getByRole("button", { name: "Show more" }));
    // Asked a moment after the click, as after typing.
    await waitFor(() => expect(api.searchDecks).toHaveBeenLastCalledWith("", DECK_PAGE * 2));
    await expect(await canvas.findByText(`Secret Lair Drop ${DECK_PAGE + 1}`)).toBeInTheDocument();
  },
};

/** While MTGJSON's list is fetched the first time. */
export const Loading: Story = {
  beforeEach: () => {
    mocked(api.searchDecks).mockReturnValue(new Promise(() => {}));
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Getting the list of decks from MTGJSON…")).toBeVisible();
  },
};

/** A search the core can't read says why. */
export const BadSearch: Story = {
  beforeEach: () => {
    mocked(api.searchDecks).mockRejectedValue(
      "Unknown filter \u201ccolor:\u201d. Try set:, type:, name:, year: or date:",
    );
    useInput.setState({ deckQuery: "color:g" });
  },
  play: async ({ canvas }) => {
    await expect(await canvas.findByText(/Unknown filter/)).toBeVisible();
  },
};

/** A search nothing matches. */
export const NoMatches: Story = {
  beforeEach: () => {
    mocked(api.searchDecks).mockResolvedValue({ decks: [], total: 0 });
    useInput.setState({ deckQuery: "set:zzz" });
  },
  play: async ({ canvas }) => {
    await waitFor(() => expect(canvas.getByText("No decks match")).toBeVisible());
  },
};
