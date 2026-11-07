import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { Screenshot } from "./Screenshot";

const meta = {
  component: Screenshot,
} satisfies Meta<typeof Screenshot>;

export default meta;
type Story = StoryObj<typeof meta>;

/** One image per colour mode; only the current mode's is shown. */
export const Light: Story = {
  globals: { theme: "light" },
  play: async ({ canvas }) => {
    const [light, dark] = canvas.getAllByRole("img", { hidden: true });
    await expect(light).toBeVisible();
    await expect(dark).not.toBeVisible();
  },
};

export const Dark: Story = {
  globals: { theme: "dark" },
  play: async ({ canvas }) => {
    const [light, dark] = canvas.getAllByRole("img", { hidden: true });
    await expect(light).not.toBeVisible();
    await expect(dark).toBeVisible();
  },
};
