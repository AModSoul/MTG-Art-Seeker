/**
 * Sorts a release's files into what the download page offers. This is the only place that knows
 * about kinds of file: the page never names one.
 *
 * Every file is either hidden (the updater's files), matched by a rule, or listed under "Other
 * files". A new kind of file is offered the day a release ships it, and adding a rule for it
 * promotes it to its system's tab with a proper label.
 */

import type { ReleaseAsset } from "./github";

export type Os = "windows" | "macos" | "linux";
export type Arch = "x64" | "arm64" | "universal";
/** The desktop app, or the `mtg-art` command line tool. */
export type Kind = "app" | "cli";

export const OSES: readonly Os[] = ["windows", "macos", "linux"];

export const OS_LABEL: Record<Os, string> = {
  windows: "Windows",
  macos: "macOS",
  linux: "Linux",
};

export type Rule = {
  id: string;
  kind: Kind;
  /** The system the file is for, or "from-name" to read it from the file name. */
  os: Os | "from-name";
  label: string;
  test: RegExp;
  /** Which file a system's tab recommends: the highest that suits the visitor. */
  priority: number;
  note?: string;
};

export const RULES: readonly Rule[] = [
  {
    id: "nsis",
    kind: "app",
    os: "windows",
    label: "Installer",
    test: /-setup\.exe$/i,
    priority: 100,
  },
  {
    id: "msi",
    kind: "app",
    os: "windows",
    label: "MSI package",
    test: /\.msi$/i,
    priority: 90,
    note: "For installing across managed machines.",
  },
  {
    id: "dmg",
    kind: "app",
    os: "macos",
    label: "Disk image",
    test: /\.dmg$/i,
    priority: 100,
  },
  {
    id: "appimage",
    kind: "app",
    os: "linux",
    label: "AppImage",
    test: /\.AppImage$/i,
    priority: 100,
    note: "Runs on most distributions without installing.",
  },
  {
    id: "deb",
    kind: "app",
    os: "linux",
    label: "Debian and Ubuntu package",
    test: /\.deb$/i,
    priority: 90,
  },
  {
    id: "rpm",
    kind: "app",
    os: "linux",
    label: "Fedora and openSUSE package",
    test: /\.rpm$/i,
    priority: 80,
  },
  {
    id: "cli",
    kind: "cli",
    os: "from-name",
    label: "Command line tool",
    test: /^mtg-art[-_].*\.(tar\.gz|tgz|zip)$/i,
    priority: 50,
  },
];

/**
 * Files no visitor needs: the updater's manifest, its signatures, and the macOS update bundle.
 * Checked before the rules, so `.app.tar.gz` never looks like an archive to download.
 */
export const HIDDEN: readonly RegExp[] = [/\.sig$/i, /^latest\.json$/i, /\.app\.tar\.gz$/i];

export type Artifact = {
  asset: ReleaseAsset;
  rule: Rule;
  /** `null` when a `from-name` rule's file doesn't say. */
  os: Os | null;
  /** `null` when the file name doesn't say. */
  arch: Arch | null;
};

export type Classified = {
  /** The desktop app's files for each system, recommended first. */
  apps: Record<Os, Artifact[]>;
  /** The command line tool's files. */
  cli: Artifact[];
  /** Files no rule knows, in the release's order. */
  other: ReleaseAsset[];
};

/** A token in a file name: the text between separators, so `x64` doesn't match inside a word. */
const token = (pattern: string) =>
  new RegExp(`(?:^|[._\\-\\s])(?:${pattern})(?=[._\\-\\s]|$)`, "i");

const ARCH_TOKENS: [Arch, RegExp][] = [
  ["universal", token("universal")],
  ["arm64", token("aarch64|arm64")],
  ["x64", token("x86_64|x64|amd64")],
];

const OS_TOKENS: [Os, RegExp][] = [
  ["windows", /pc-windows|windows|win64|win32/i],
  ["macos", /apple-darwin|darwin|macos|osx/i],
  ["linux", /linux/i],
];

export function archFromName(name: string): Arch | null {
  return ARCH_TOKENS.find(([, pattern]) => pattern.test(name))?.[0] ?? null;
}

export function osFromName(name: string): Os | null {
  return OS_TOKENS.find(([, pattern]) => pattern.test(name))?.[0] ?? null;
}

/** The rule for a file, `"hidden"` for the updater's files, or `null` for one no rule knows. */
export function ruleFor(name: string): Rule | "hidden" | null {
  if (HIDDEN.some((pattern) => pattern.test(name))) return "hidden";
  return RULES.find((rule) => rule.test.test(name)) ?? null;
}

export function classify(assets: readonly ReleaseAsset[]): Classified {
  const apps: Record<Os, Artifact[]> = { windows: [], macos: [], linux: [] };
  const cli: Artifact[] = [];
  const other: ReleaseAsset[] = [];

  for (const asset of assets) {
    const rule = ruleFor(asset.name);
    if (rule === "hidden") continue;
    if (!rule) {
      other.push(asset);
      continue;
    }
    const os = rule.os === "from-name" ? osFromName(asset.name) : rule.os;
    const artifact = { asset, rule, os, arch: archFromName(asset.name) };
    if (rule.kind === "cli") cli.push(artifact);
    else if (os) apps[os].push(artifact);
    else other.push(asset);
  }

  for (const os of OSES) apps[os].sort(byPriority);
  cli.sort((a, b) => OSES.indexOf(a.os ?? "linux") - OSES.indexOf(b.os ?? "linux"));
  return { apps, cli, other };
}

const byPriority = (a: Artifact, b: Artifact) => b.rule.priority - a.rule.priority;

/**
 * The architecture to assume when the browser doesn't say: most Macs sold since 2020 are Apple
 * silicon, and most Windows and Linux desktops are x64.
 */
export const DEFAULT_ARCH: Record<Os, Arch> = { windows: "x64", macos: "arm64", linux: "x64" };

/** How well a file's architecture suits the visitor's: 2 runs natively, 1 might, 0 won't. */
function archFit(artifact: Arch | null, visitor: Arch): number {
  if (artifact === visitor || artifact === "universal") return 2;
  if (artifact === null) return 1;
  // Windows and macOS on ARM run x64 builds; nothing runs an ARM build on x64.
  return artifact === "x64" ? 1 : 0;
}

/**
 * The file to recommend from one system's files: the best architecture fit for the visitor, then
 * the highest priority.
 */
export function recommend(
  artifacts: readonly Artifact[],
  os: Os,
  visitorArch: Arch | null,
): Artifact | null {
  const arch = visitorArch ?? DEFAULT_ARCH[os];
  let best: Artifact | null = null;
  let bestFit = -1;
  for (const artifact of artifacts) {
    const fit = archFit(artifact.arch, arch);
    if (fit > bestFit || (fit === bestFit && best && artifact.rule.priority > best.rule.priority)) {
      best = artifact;
      bestFit = fit;
    }
  }
  return best;
}

/** How a file's architecture reads on its system's tab. */
export function archLabel(os: Os | null, arch: Arch | null): string | null {
  switch (arch) {
    case null:
      return null;
    case "universal":
      return os === "macos" ? "Apple silicon and Intel" : "All processors";
    case "arm64":
      return os === "macos" ? "Apple silicon" : "ARM64";
    case "x64":
      return os === "macos" ? "Intel" : "64-bit";
  }
}

const SIZE = new Intl.NumberFormat("en", { maximumFractionDigits: 1 });

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${SIZE.format(bytes / 1024)} KB`;
  return `${SIZE.format(bytes / 1024 ** 2)} MB`;
}
