import type { Release, ReleaseAsset } from "../github";
import { toRelease } from "../github";
import v010 from "./release-v0.1.0.json";

const parsed = toRelease(v010);
if (!parsed) throw new Error("release-v0.1.0.json isn't a release");

/** The real v0.1.0 release: Windows, a universal macOS build and Linux packages. */
export const releaseV010: Release = parsed;

const asset = (name: string, size: number): ReleaseAsset => ({
  name,
  size,
  browser_download_url: `https://github.com/AModSoul/mtg-art-downloader-rust/releases/download/v0.2.0/${name}`,
});

/** A release with files v0.1.0 didn't have: per-architecture Macs, the CLI, and unknown files. */
export const releaseWithEverything: Release = {
  tag_name: "v0.2.0",
  name: "MTG Art Downloader v0.2.0",
  published_at: "2026-12-01T12:00:00Z",
  html_url: "https://github.com/AModSoul/mtg-art-downloader-rust/releases/tag/v0.2.0",
  assets: [
    asset("latest.json", 7_132),
    asset("MTG.Art.Downloader_0.2.0_x64-setup.exe", 6_201_344),
    asset("MTG.Art.Downloader_0.2.0_x64-setup.exe.sig", 420),
    asset("MTG.Art.Downloader_0.2.0_arm64-setup.exe", 5_902_112),
    asset("MTG.Art.Downloader_0.2.0_aarch64.dmg", 7_801_002),
    asset("MTG.Art.Downloader_0.2.0_x64.dmg", 8_012_441),
    asset("MTG.Art.Downloader_0.2.0_amd64.AppImage", 79_210_112),
    asset("MTG.Art.Downloader_0.2.0_amd64.deb", 7_402_884),
    asset("mtg-art-x86_64-pc-windows-msvc.zip", 4_120_330),
    asset("mtg-art-aarch64-apple-darwin.tar.gz", 3_901_204),
    asset("mtg-art-x86_64-unknown-linux-gnu.tar.gz", 4_330_112),
    asset("SHA256SUMS.txt", 1_024),
  ],
};

/** A published release whose files haven't been attached. */
export const emptyRelease: Release = {
  ...releaseWithEverything,
  assets: [asset("latest.json", 7_132)],
};
