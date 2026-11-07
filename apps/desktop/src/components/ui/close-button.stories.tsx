import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent } from "storybook/test";

import { CloseButton } from "./close-button";

const meta = {
  component: CloseButton,
  args: { onClick: fn() },
} satisfies Meta<typeof CloseButton>;

export default meta;
type Story = StoryObj<typeof meta>;

/** An X icon button, labelled "Close" for screen readers. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Close" }));
    await expect(args.onClick).toHaveBeenCalledTimes(1);
  },
};

export const Small: Story = { args: { size: "sm" } };
