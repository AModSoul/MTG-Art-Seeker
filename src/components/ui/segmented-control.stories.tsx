import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent } from "storybook/test";

import { SegmentedControl } from "./segmented-control";

const meta = {
  component: SegmentedControl,
  args: {
    size: "sm",
    defaultValue: "all",
    items: [
      { value: "all", label: "All 7" },
      { value: "failed", label: "Failed 2" },
      { value: "scryfall", label: "Scryfall 4" },
      { value: "mtgpics", label: "MTGPics 1", disabled: true },
    ],
    onValueChange: fn(),
  },
} satisfies Meta<typeof SegmentedControl>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each item is a radio; a disabled one can't be chosen. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    await userEvent.click(canvas.getByRole("radio", { name: "Failed 2" }));
    await expect(args.onValueChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ value: "failed" }),
    );
    await expect(canvas.getByRole("radio", { name: "MTGPics 1" })).toBeDisabled();
  },
};

/** `items` also takes plain strings. */
export const StringItems: Story = {
  args: { items: ["Newest", "Oldest"], defaultValue: "Newest" },
};
