import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, userEvent, waitFor } from "storybook/test";

import { api } from "@/api";
import { useRun } from "@/store";

import { FoundArtDialog } from "./FoundArtDialog";

const meta = {
  component: FoundArtDialog,
} satisfies Meta<typeof FoundArtDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

/** No image waiting: nothing shows. */
export const Closed: Story = {
  play: async () => {
    await expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  },
};

/** An image that isn't this card's art: saved only if the user says so. */
export const Different: Story = {
  beforeEach: () => {
    useRun.setState({
      doubts: [{ event: "doubtful", token: 7, face: "Sol Ring", doubt: "different" }],
    });
  },
  play: async () => {
    const dialog = await screen.findByRole("alertdialog", {
      name: "This doesn't look like the art",
    });
    await waitFor(() => expect(dialog).toBeVisible());
    await expect(screen.getByText("It doesn't match Scryfall's art for Sol Ring.")).toBeVisible();

    await userEvent.click(screen.getByRole("button", { name: "Save anyway" }));
    await expect(api.confirmFoundArt).toHaveBeenCalledWith(7, true);
    await expect(useRun.getState().doubts).toEqual([]);
  },
};

/** A card scan, thrown away. */
export const CardScan: Story = {
  beforeEach: () => {
    useRun.setState({
      doubts: [{ event: "doubtful", token: 8, face: "Sol Ring", doubt: "cardScan" }],
    });
  },
  play: async () => {
    await waitFor(() =>
      expect(
        screen.getByRole("alertdialog", { name: "This looks like a whole card" }),
      ).toBeVisible(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Don't save" }));
    await expect(api.confirmFoundArt).toHaveBeenCalledWith(8, false);
  },
};

/** Several waiting: each is asked about in turn, oldest first. */
export const OneAtATime: Story = {
  beforeEach: () => {
    useRun.setState({
      doubts: [
        { event: "doubtful", token: 1, face: "Sol Ring", doubt: "smaller" },
        { event: "doubtful", token: 2, face: "Path to Exile", doubt: "different" },
      ],
    });
  },
  play: async () => {
    await waitFor(() =>
      expect(screen.getByRole("alertdialog", { name: "This copy isn't any larger" })).toBeVisible(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Don't save" }));
    await waitFor(() =>
      expect(
        screen.getByRole("alertdialog", { name: "This doesn't look like the art" }),
      ).toBeVisible(),
    );
    await expect(
      screen.getByText("It doesn't match Scryfall's art for Path to Exile."),
    ).toBeVisible();
    await expect(api.confirmFoundArt).toHaveBeenCalledWith(1, false);
  },
};
