import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, mocked, screen, userEvent, waitFor } from "storybook/test";

import { api } from "@/api";

import { DiscordButton } from "./DiscordButton";

const meta = {
  component: DiscordButton,
} satisfies Meta<typeof DiscordButton>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Opens the Discord invite through the Rust side, which hands it to the browser. */
export const Default: Story = {
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Join the Proxy Community Discord" }));
    await expect(api.openDiscord).toHaveBeenCalledTimes(1);
  },
};

export const OpenFails: Story = {
  beforeEach: () => {
    mocked(api.openDiscord).mockRejectedValue("No browser found");
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Join the Proxy Community Discord" }));
    await waitFor(() => expect(screen.getByText("Couldn't open Discord")).toBeVisible());
  },
};
