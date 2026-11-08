import { Box, Button, Icon } from "@chakra-ui/react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { LuImage } from "react-icons/lu";
import { expect } from "storybook/test";

import { Card } from "./card";

const meta = {
  component: Card,
  args: {
    maxW: "sm",
    title: "Larger, uncropped art",
    description: "Art comes from MTGPics where it has it, and from Scryfall otherwise.",
  },
} satisfies Meta<typeof Card>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Its title is an `h3` unless `titleProps` says otherwise. */
export const Default: Story = {
  play: async ({ canvas }) => {
    await expect(
      canvas.getByRole("heading", { level: 3, name: "Larger, uncropped art" }),
    ).toBeVisible();
  },
};

/** An icon above the title, as the site's features have. */
export const WithIcon: Story = {
  args: {
    variant: "subtle",
    icon: <Icon as={LuImage} boxSize="6" color="emerald.fg" aria-hidden />,
    titleProps: { as: "h2" },
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("heading", { level: 2 })).toBeVisible();
  },
};

/** A picture flush with its edges, and actions in the footer. */
export const WithMediaAndFooter: Story = {
  args: {
    size: "sm",
    title: "1920 × 1408",
    description: "cdna.artstation.com",
    media: <Box h="40" bg="bg.emphasized" />,
    footer: <Button size="sm">Save this one</Button>,
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("button", { name: "Save this one" })).toBeVisible();
  },
};
