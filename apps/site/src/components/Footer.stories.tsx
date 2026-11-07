import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { Footer } from "./Footer";

const meta = {
  component: Footer,
} satisfies Meta<typeof Footer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvas }) => {
    for (const name of ["GitHub", "Releases", "Discord"]) {
      await expect(canvas.getByRole("link", { name })).toHaveAttribute("target", "_blank");
    }
  },
};
