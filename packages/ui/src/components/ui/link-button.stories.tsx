import type { Meta, StoryObj } from "@storybook/react-vite";
import { LuDownload } from "react-icons/lu";
import { expect } from "storybook/test";

import { LinkButton } from "./link-button";

const meta = {
  component: LinkButton,
  args: {
    href: "https://github.com/AModSoul/mtg-art-downloader-rust/releases/latest",
    children: "Go to the latest release",
  },
} satisfies Meta<typeof LinkButton>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A link, styled as a button: it navigates rather than acts. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    const link = canvas.getByRole("link", { name: "Go to the latest release" });
    await expect(link).toHaveAttribute("href", args.href);
  },
};

/** Takes the button's variants, sizes and palettes, and an icon beside its text. */
export const WithIcon: Story = {
  args: {
    colorPalette: "blue",
    size: "lg",
    children: (
      <>
        <LuDownload /> Download for Windows
      </>
    ),
  },
};

export const Outline: Story = { args: { variant: "outline", children: "View on GitHub" } };

/** With `asChild`, it styles a router's link instead of rendering its own. */
export const AsChild: Story = {
  args: {
    asChild: true,
    href: undefined,
    children: <a href="#download">Download</a>,
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("link", { name: "Download" })).toHaveAttribute(
      "href",
      "#download",
    );
  },
};
