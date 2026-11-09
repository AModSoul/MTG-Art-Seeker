import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";

import { Header } from "./Header";

const meta = {
  component: Header,
} satisfies Meta<typeof Header>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvas }) => {
    const nav = canvas.getByRole("navigation", { name: "Main" });
    await expect(nav).toBeVisible();
    await expect(canvas.getByRole("link", { name: "Home" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(canvas.getByRole("link", { name: "Download" })).not.toHaveAttribute(
      "aria-current",
    );
    await expect(canvas.getByRole("link", { name: "Source code on GitHub" })).toHaveAttribute(
      "href",
      "https://github.com/AModSoul/MTG-Art-Seeker",
    );
  },
};
