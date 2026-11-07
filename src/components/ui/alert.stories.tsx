import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { Alert } from "./alert";

const meta = {
  component: Alert,
  args: {
    title: "The download couldn't start",
    children: "EDHREC no longer has that list.",
  },
} satisfies Meta<typeof Alert>;

export default meta;
type Story = StoryObj<typeof meta>;

export const ErrorStatus: Story = {
  args: { status: "error" },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("The download couldn't start")).toBeVisible();
    await expect(canvas.getByText("EDHREC no longer has that list.")).toBeVisible();
  },
};

export const Warning: Story = { args: { status: "warning", title: "Some art is missing" } };

export const Success: Story = { args: { status: "success", title: "Saved 12 images" } };

export const Info: Story = { args: { status: "info", title: "Updates are checked at start-up" } };

/** Without a body, only the title shows. */
export const TitleOnly: Story = { args: { status: "info", children: undefined } };
