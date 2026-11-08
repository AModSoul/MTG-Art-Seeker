import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, screen, userEvent, waitFor } from "storybook/test";

import { api } from "@/api";
import type { PageImages } from "@/store";
import { useRun } from "@/store";

import { PageImagesDialog } from "./PageImagesDialog";

const meta = {
  component: PageImagesDialog,
} satisfies Meta<typeof PageImagesDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

/** An Instagram post's images: the art, a card scan of it, and the account's
 * banner. */
const instagramPost: PageImages = {
  event: "choose",
  token: 4,
  face: "Counterspell",
  images: [
    {
      path: "C:/cache/finder-downloads/page/page-1.jpg",
      size: { width: 1440, height: 1054 },
      doubt: null,
      host: "scontent.cdninstagram.com",
    },
    {
      path: "C:/cache/finder-downloads/page/page-2.jpg",
      size: { width: 745, height: 1040 },
      doubt: "cardScan",
      host: "scontent.cdninstagram.com",
    },
    {
      path: "C:/cache/finder-downloads/page/page-3.webp",
      size: { width: 1500, height: 500 },
      doubt: "different",
      host: "static.cdninstagram.com",
    },
  ],
};

/** Nothing to choose from: nothing shows. */
export const Closed: Story = {
  play: async () => {
    await expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  },
};

/** Each image with its size and what the check said; the chosen one is saved. */
export const ChooseTheArt: Story = {
  beforeEach: () => {
    useRun.setState({ pageImages: instagramPost });
  },
  play: async () => {
    const dialog = await screen.findByRole("dialog", { name: "Choose Counterspell's art" });
    await waitFor(() => expect(dialog).toBeVisible());
    await expect(screen.getByText("1440 × 1054")).toBeVisible();
    await expect(screen.getByText("The art")).toBeVisible();
    await expect(screen.getByText("Card scan")).toBeVisible();
    await expect(screen.getByText("Other image")).toBeVisible();

    await userEvent.click(
      screen.getByRole("button", {
        name: "Save this one: 745 × 1040, from scontent.cdninstagram.com",
      }),
    );
    await expect(api.choosePageImage).toHaveBeenCalledWith(4, 1);
    await expect(useRun.getState().pageImages).toBeNull();
  },
};

/** Cancelled: none is saved, and the images are let go. */
export const Cancelled: Story = {
  beforeEach: () => {
    useRun.setState({ pageImages: instagramPost });
  },
  play: async () => {
    await waitFor(() =>
      expect(screen.getByRole("dialog", { name: "Choose Counterspell's art" })).toBeVisible(),
    );
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await expect(api.choosePageImage).toHaveBeenCalledWith(4, null);
    // Gone once it has animated out, which takes longer while a whole suite runs.
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument(), {
      timeout: 3000,
    });
  },
};
