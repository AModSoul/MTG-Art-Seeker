import { relaunch } from "@tauri-apps/plugin-process";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, screen, userEvent, waitFor } from "storybook/test";

import { UpdateOffer } from "./UpdateButton";

// `UpdateButton` asks GitHub only in a release build, so the stories show the
// offer it makes once a newer release is found.
const meta = {
  component: UpdateOffer,
  args: {
    update: { version: "0.3.0", downloadAndInstall: fn(async () => {}) },
  },
} satisfies Meta<typeof UpdateOffer>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Installs the update, then restarts the app on it. */
export const Default: Story = {
  play: async ({ canvas, args }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Update to 0.3.0" }));
    await expect(args.update.downloadAndInstall).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(relaunch).toHaveBeenCalledTimes(1));
  },
};

/** A failed install says why, and the button can be tried again. */
export const InstallFails: Story = {
  args: {
    update: {
      version: "0.3.0",
      downloadAndInstall: fn(async () => {
        throw new Error("The signature didn't match");
      }),
    },
  },
  play: async ({ canvas }) => {
    const button = canvas.getByRole("button", { name: "Update to 0.3.0" });
    await userEvent.click(button);
    await waitFor(() => expect(screen.getByText("The update failed")).toBeVisible());
    await expect(button).toBeEnabled();
    await expect(relaunch).not.toHaveBeenCalled();
  },
};
