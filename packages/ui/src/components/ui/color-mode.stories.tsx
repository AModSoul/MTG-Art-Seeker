import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { ColorModeButton } from "./color-mode";

// Storybook's theme switcher sets the colour mode here, not next-themes, so
// clicking the button doesn't change the page as it does in the apps.
const meta = {
  component: ColorModeButton,
} satisfies Meta<typeof ColorModeButton>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A sun or a moon for the mode in use, labelled for screen readers. */
export const Default: Story = {
  play: async ({ canvas }) => {
    await expect(await canvas.findByRole("button", { name: "Toggle color mode" })).toBeEnabled();
  },
};

export const Disabled: Story = {
  args: { disabled: true },
  play: async ({ canvas }) => {
    await expect(await canvas.findByRole("button", { name: "Toggle color mode" })).toBeDisabled();
  },
};
