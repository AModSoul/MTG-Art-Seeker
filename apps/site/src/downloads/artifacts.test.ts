import { describe, expect, it } from "vitest";

import release from "./__fixtures__/release-v0.1.0.json";
import {
  archFromName,
  archLabel,
  classify,
  formatSize,
  osFromName,
  recommend,
  ruleFor,
} from "./artifacts";
import { type ReleaseAsset, toRelease } from "./github";

const asset = (name: string, size = 1): ReleaseAsset => ({
  name,
  size,
  browser_download_url: `https://example.com/${name}`,
});

const names = (artifacts: { asset: ReleaseAsset }[]) => artifacts.map((a) => a.asset.name);

describe("classify, with v0.1.0's files", () => {
  const parsed = toRelease(release);
  if (!parsed) throw new Error("the fixture isn't a release");
  const { apps, cli, other } = classify(parsed.assets);

  it("offers each system's installers, recommended first", () => {
    expect(names(apps.windows)).toEqual([
      "MTG.Art.Downloader_0.1.0_x64-setup.exe",
      "MTG.Art.Downloader_0.1.0_x64_en-US.msi",
    ]);
    expect(names(apps.macos)).toEqual(["MTG.Art.Downloader_0.1.0_universal.dmg"]);
    expect(names(apps.linux)).toEqual([
      "MTG.Art.Downloader_0.1.0_amd64.AppImage",
      "MTG.Art.Downloader_0.1.0_amd64.deb",
      "MTG.Art.Downloader-0.1.0-1.x86_64.rpm",
    ]);
  });

  it("hides the updater's files and has nothing left over", () => {
    expect(cli).toEqual([]);
    expect(other).toEqual([]);
  });

  it("reads each file's architecture", () => {
    expect(apps.windows.map((a) => a.arch)).toEqual(["x64", "x64"]);
    expect(apps.macos.map((a) => a.arch)).toEqual(["universal"]);
    expect(apps.linux.map((a) => a.arch)).toEqual(["x64", "x64", "x64"]);
  });
});

describe("classify, with files a later release might ship", () => {
  const { apps, cli, other } = classify([
    asset("MTG.Art.Downloader_0.2.0_aarch64.dmg"),
    asset("MTG.Art.Downloader_0.2.0_x64.dmg"),
    asset("MTG.Art.Downloader_0.2.0_arm64-setup.exe"),
    asset("mtg-art-x86_64-pc-windows-msvc.zip"),
    asset("mtg-art-aarch64-apple-darwin.tar.gz"),
    asset("mtg-art-x86_64-unknown-linux-gnu.tar.gz"),
    asset("mtg-art-source.tar.gz"),
    asset("SHA256SUMS.txt"),
    asset("MTG.Art.Downloader_0.2.0_x64.portable.zip"),
  ]);

  it("puts new builds of known kinds on their system's tab", () => {
    expect(names(apps.macos)).toEqual([
      "MTG.Art.Downloader_0.2.0_aarch64.dmg",
      "MTG.Art.Downloader_0.2.0_x64.dmg",
    ]);
    expect(apps.macos.map((a) => a.arch)).toEqual(["arm64", "x64"]);
    expect(apps.windows.map((a) => a.arch)).toEqual(["arm64"]);
  });

  it("finds the command line tool's system in its target triple", () => {
    expect(cli.map((a) => [a.os, a.arch])).toEqual([
      ["windows", "x64"],
      ["macos", "arm64"],
      ["linux", "x64"],
      [null, null],
    ]);
  });

  it("lists files no rule knows, or whose system it can't tell, as other files", () => {
    expect(other.map((a) => a.name)).toEqual([
      "SHA256SUMS.txt",
      "MTG.Art.Downloader_0.2.0_x64.portable.zip",
    ]);
  });

  it("puts a command line archive with no system under the command line tool", () => {
    expect(cli.some((a) => a.asset.name === "mtg-art-source.tar.gz" && a.os === null)).toBe(true);
  });
});

describe("recommend", () => {
  const { apps } = classify([
    asset("App_1.0.0_aarch64.dmg"),
    asset("App_1.0.0_x64.dmg"),
    asset("App_1.0.0_x64-setup.exe"),
    asset("App_1.0.0_x64_en-US.msi"),
  ]);

  it("picks the visitor's architecture", () => {
    expect(recommend(apps.macos, "macos", "x64")?.arch).toBe("x64");
    expect(recommend(apps.macos, "macos", "arm64")?.arch).toBe("arm64");
  });

  it("assumes Apple silicon on a Mac that doesn't say", () => {
    expect(recommend(apps.macos, "macos", null)?.arch).toBe("arm64");
  });

  it("prefers the higher priority file of the same fit", () => {
    expect(recommend(apps.windows, "windows", "x64")?.rule.id).toBe("nsis");
  });

  it("falls back to an x64 build on ARM", () => {
    expect(recommend(apps.windows, "windows", "arm64")?.rule.id).toBe("nsis");
  });

  it("recommends nothing when there's nothing", () => {
    expect(recommend([], "linux", "x64")).toBeNull();
  });
});

describe("names", () => {
  it("reads architectures only as whole tokens", () => {
    expect(archFromName("App_1.0_x86_64.rpm")).toBe("x64");
    expect(archFromName("App-arm64.zip")).toBe("arm64");
    expect(archFromName("Max64Tool.zip")).toBeNull();
  });

  it("reads systems from target triples", () => {
    expect(osFromName("mtg-art-aarch64-unknown-linux-musl.tar.gz")).toBe("linux");
    expect(osFromName("mtg-art.zip")).toBeNull();
  });

  it("hides the updater's files before matching rules", () => {
    expect(ruleFor("App_universal.app.tar.gz")).toBe("hidden");
    expect(ruleFor("App_x64-setup.exe.sig")).toBe("hidden");
  });

  it("labels architectures for the system", () => {
    expect(archLabel("macos", "universal")).toBe("Apple silicon and Intel");
    expect(archLabel("windows", "x64")).toBe("64-bit");
    expect(archLabel("linux", null)).toBeNull();
  });

  it("formats sizes", () => {
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(7_192_836)).toBe("6.9 MB");
  });
});
