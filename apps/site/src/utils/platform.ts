/**
 * Best-effort detection of the visitor's system and processor, to pick the download to recommend.
 * Takes a navigator-like object so it stays testable and never reads globals during prerender.
 */

import type { Arch, Os } from "@/downloads/artifacts";

/** The parts of `Navigator` read here. `userAgentData` is Chromium's, and not in the DOM types. */
export type NavigatorLike = {
  userAgent: string;
  maxTouchPoints?: number;
  userAgentData?: {
    platform: string;
    getHighEntropyValues?: (hints: string[]) => Promise<{ architecture?: string }>;
  };
};

export function detectOs(nav: NavigatorLike): Os | null {
  const platform = nav.userAgentData?.platform;
  if (platform) {
    if (platform === "Windows") return "windows";
    if (platform === "macOS") return "macos";
    if (platform === "Linux") return "linux";
    return null;
  }

  const ua = nav.userAgent;
  if (/Windows NT/.test(ua)) return "windows";
  // iPadOS in desktop mode sends a Mac user agent; touch support gives it away.
  if (/Macintosh/.test(ua)) return (nav.maxTouchPoints ?? 0) <= 1 ? "macos" : null;
  if (/Android|CrOS/.test(ua)) return null;
  if (/Linux|X11/.test(ua)) return "linux";
  return null;
}

/**
 * Chromium reports the processor through client hints. Other browsers only hint at it in the user
 * agent, and Safari and Firefox say "Intel Mac OS X" even on Apple silicon, so a Mac's user agent
 * is never trusted.
 */
export async function detectArch(nav: NavigatorLike): Promise<Arch | null> {
  const getHighEntropyValues = nav.userAgentData?.getHighEntropyValues;
  if (getHighEntropyValues) {
    try {
      const { architecture } = await getHighEntropyValues.call(nav.userAgentData, ["architecture"]);
      if (architecture === "arm") return "arm64";
      if (architecture === "x86") return "x64";
    } catch {
      // Fall through to the user agent.
    }
  }

  const ua = nav.userAgent;
  if (/Macintosh/.test(ua)) return null;
  if (/aarch64|arm64|ARM64/.test(ua)) return "arm64";
  if (/x86_64|x64|Win64|WOW64|amd64/i.test(ua)) return "x64";
  return null;
}
