import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, mocked, screen, userEvent, waitFor, within } from "storybook/test";

import { minimizeWindow, windowFrame } from "@/appWindow";

import { AppHeader } from "./AppHeader";

const meta = {
  component: AppHeader,
} satisfies Meta<typeof AppHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

/** On Windows and Linux: the app's name and buttons, the window's own buttons at the end, and
 * the bar to drag the window by. */
export const Windows: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("heading", { level: 1, name: "MTG Art Seeker" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Settings" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Minimize" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Maximize" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Close" })).toBeVisible();
    // Anywhere on it but its buttons.
    await expect(canvas.getByRole("banner")).toHaveAttribute("data-tauri-drag-region", "deep");
  },
};

/** On macOS the native title bar stays, its traffic lights over the start of this bar: no
 * window buttons of the app's own, and the name only for screen readers. */
export const MacOS: Story = {
  beforeEach: () => {
    mocked(windowFrame).mockReturnValue("mac");
  },
  play: async ({ canvas }) => {
    const heading = canvas.getByRole("heading", { level: 1, name: "MTG Art Seeker" });
    // Clipped to nothing, out of the traffic lights' way, but still read out.
    const name = within(heading).getByText("MTG Art Seeker", { exact: true });
    await expect(name.getBoundingClientRect().width).toBeLessThanOrEqual(1);
    await expect(canvas.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    await expect(canvas.getByRole("banner")).toHaveAttribute("data-tauri-drag-region");
  },
};

/** With the settings drawer open, which opens below the bar, the window's buttons still take a
 * click, and using the bar leaves the drawer open. The drawer is modal, so it hides the rest of
 * the page, the bar too, from screen readers, and keeps the keyboard to itself: these buttons
 * are for the pointer then, as a native title bar's are. */
export const WithSettingsOpen: Story = {
  play: async ({ canvas }) => {
    await userEvent.click(canvas.getByRole("button", { name: "Settings" }));
    await waitFor(() => expect(screen.getByRole("dialog", { name: "Settings" })).toBeVisible());
    // A click on anything the drawer blocks (`pointer-events: none`) would fail here.
    await userEvent.click(canvas.getByRole("button", { name: "Minimize", hidden: true }));
    await expect(minimizeWindow).toHaveBeenCalledTimes(1);
    // Pressing the bar, as to drag the window, isn't a click outside the drawer either.
    await userEvent.click(canvas.getByRole("heading", { level: 1, hidden: true }));
    await new Promise((resolve) => setTimeout(resolve, 500));
    await expect(screen.getByRole("dialog", { name: "Settings" })).toBeVisible();

    // The drawer starts where the bar ends.
    const bar = canvas.getByRole("banner", { hidden: true }).getBoundingClientRect();
    const drawer = screen.getByRole("dialog", { name: "Settings" }).getBoundingClientRect();
    await expect(Math.round(drawer.top)).toBeGreaterThanOrEqual(Math.round(bar.bottom));
  },
};

/** The settings button closes the drawer it opened, rather than starting to close it as a click
 * outside it and then opening it again. */
export const SettingsButtonCloses: Story = {
  play: async ({ canvas }) => {
    const button = canvas.getByRole("button", { name: "Settings" });
    await userEvent.click(button);
    await waitFor(() => expect(screen.getByRole("dialog", { name: "Settings" })).toBeVisible());

    // Pressed as a person does, the button down a moment before it's let go: the drawer saw the
    // press as one outside it and started to close, and the click then opened it again.
    // One session for both halves, so letting go makes the click.
    const user = userEvent.setup();
    await user.pointer({ keys: "[MouseLeft>]", target: button });
    await new Promise((resolve) => setTimeout(resolve, 100));
    await user.pointer({ keys: "[/MouseLeft]", target: button });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Settings" })).not.toBeInTheDocument(),
    );
    // And it stays closed once the drawer has slid away.
    await new Promise((resolve) => setTimeout(resolve, 500));
    await expect(screen.queryByRole("dialog", { name: "Settings" })).not.toBeInTheDocument();
  },
};

/** Outside Tauri there's no window to drive: no window buttons, and nothing to drag. */
export const Browser: Story = {
  beforeEach: () => {
    mocked(windowFrame).mockReturnValue("browser");
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("heading", { level: 1, name: "MTG Art Seeker" })).toBeVisible();
    await expect(canvas.queryByRole("button", { name: "Minimize" })).not.toBeInTheDocument();
    await expect(canvas.getByRole("banner")).not.toHaveAttribute("data-tauri-drag-region");
    await userEvent.click(canvas.getByRole("button", { name: "Settings" }));
  },
};
