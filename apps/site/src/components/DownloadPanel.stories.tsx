import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";

import {
  emptyRelease,
  releaseV010,
  releaseWithEverything,
} from "@/downloads/__fixtures__/releases";

import { DownloadPanel } from "./DownloadPanel";

const meta = {
  component: DownloadPanel,
  args: {
    state: { status: "ready", release: releaseV010 },
    os: "windows",
    arch: "x64",
  },
} satisfies Meta<typeof DownloadPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

/** While GitHub is asked: placeholders the shape of the downloads, and a link
 * to GitHub's releases that works without scripts. */
export const Loading: Story = {
  args: { state: { status: "loading" } },
  play: async ({ canvas, canvasElement }) => {
    await expect(canvas.getByText(/Finding the latest release/)).toBeVisible();
    await expect(canvas.getByRole("link", { name: "All releases on GitHub" })).toBeVisible();
    await expect(canvasElement.querySelectorAll(".chakra-skeleton")).toHaveLength(3);
  },
};

export const Windows: Story = {
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("tab", { name: "Windows" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(canvas.getByRole("link", { name: "Download for Windows" })).toHaveAttribute(
      "href",
      expect.stringMatching(/_x64-setup\.exe$/),
    );
    await expect(canvas.getByRole("link", { name: "MSI package, 64-bit" })).toBeVisible();
    await expect(canvas.getByText("Windows may warn you the first time")).toBeVisible();
    await expect(canvas.getByText(/Version 0\.1\.0/)).toBeVisible();
  },
};

export const MacOS: Story = {
  args: { os: "macos", arch: null },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("link", { name: "Download for macOS" })).toHaveAttribute(
      "href",
      expect.stringMatching(/_universal\.dmg$/),
    );
    await expect(canvas.getByText(/Apple silicon and Intel/)).toBeVisible();
  },
};

export const Linux: Story = {
  args: { os: "linux", arch: "x64" },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("link", { name: "Download for Linux" })).toHaveAttribute(
      "href",
      expect.stringMatching(/\.AppImage$/),
    );
    await expect(
      canvas.getByRole("link", { name: "Debian and Ubuntu package, 64-bit" }),
    ).toBeVisible();
    await expect(canvas.getByText("Running the AppImage")).toBeVisible();
  },
};

/** A phone or an unknown system: the first system's tab, and any tab can be picked. */
export const UnknownSystem: Story = {
  args: { os: null, arch: null },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("tab", { name: "Windows" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.click(canvas.getByRole("tab", { name: "Linux" }));
    await expect(canvas.getByRole("link", { name: "Download for Linux" })).toBeVisible();
  },
};

/** Files v0.1.0 didn't have show up without changes to the page. */
export const NewKindsOfFile: Story = {
  args: { state: { status: "ready", release: releaseWithEverything }, os: "macos", arch: "x64" },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("link", { name: "Download for macOS" })).toHaveAttribute(
      "href",
      expect.stringMatching(/_x64\.dmg$/),
    );
    await expect(canvas.getByRole("link", { name: "Disk image, Apple silicon" })).toBeVisible();
    await expect(canvas.getByRole("heading", { name: "Command line tool" })).toBeVisible();
    await expect(canvas.getByRole("link", { name: "macOS, Apple silicon" })).toBeVisible();
    await expect(canvas.getByRole("heading", { name: "Other files" })).toBeVisible();
    await expect(canvas.getByRole("link", { name: "SHA256SUMS.txt" })).toBeVisible();
  },
};

export const EmptyRelease: Story = {
  args: { state: { status: "ready", release: emptyRelease } },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("This release has no downloads yet.")).toBeVisible();
    await expect(canvas.queryByRole("tablist")).toBeNull();
  },
};

export const LoadFailed: Story = {
  args: { state: { status: "error", message: "GitHub answered 403." } },
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Couldn't load the latest release from GitHub")).toBeVisible();
    await expect(canvas.getByRole("link", { name: "Go to the latest release" })).toHaveAttribute(
      "href",
      "https://github.com/AModSoul/mtg-art-downloader-rust/releases/latest",
    );
  },
};
