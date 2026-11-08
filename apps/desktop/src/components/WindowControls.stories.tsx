import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, mocked, screen, userEvent, waitFor } from "storybook/test";

import {
  closeWindow,
  isWindowMaximized,
  minimizeWindow,
  onWindowResized,
  toggleMaximizeWindow,
} from "@/appWindow";

import { WindowControls } from "./WindowControls";

const meta = {
  component: WindowControls,
  decorators: [
    (Story) => (
      <div style={{ display: "flex", height: "40px" }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof WindowControls>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Each button minimizes, maximizes or closes the window. */
export const Default: Story = {
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Minimize" }));
    await expect(minimizeWindow).toHaveBeenCalledTimes(1);
    await userEvent.click(canvas.getByRole("button", { name: "Maximize" }));
    await expect(toggleMaximizeWindow).toHaveBeenCalledTimes(1);
    await userEvent.click(canvas.getByRole("button", { name: "Close" }));
    await expect(closeWindow).toHaveBeenCalledTimes(1);
  },
};

/** A maximized window's middle button restores it, and follows the window when something
 * else, such as snapping it to the screen's edge, changes it. */
export const Maximized: Story = {
  beforeEach: () => {
    mocked(isWindowMaximized).mockResolvedValue(true);
  },
  play: async ({ canvas }) => {
    await waitFor(() => expect(canvas.getByRole("button", { name: "Restore" })).toBeVisible());

    // The window is restored by something other than the button.
    const resized = mocked(onWindowResized).mock.calls[0]?.[0];
    mocked(isWindowMaximized).mockResolvedValue(false);
    resized?.();
    await waitFor(() => expect(canvas.getByRole("button", { name: "Maximize" })).toBeVisible());
  },
};

/** A window that won't close says so. */
export const CloseFails: Story = {
  beforeEach: () => {
    mocked(closeWindow).mockRejectedValue("Not allowed");
  },
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.getByText("Couldn't close the window")).toBeVisible());
  },
};
