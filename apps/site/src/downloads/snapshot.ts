/**
 * The latest release as it was when the site was built. The prerender fetches it and writes it
 * into the download page's <head>, so the page shows its downloads before (or without) asking
 * GitHub again.
 */

import { type Release, toRelease } from "./github";

export const SNAPSHOT_ELEMENT_ID = "release-snapshot";

/** Set by the prerender while it renders the download page. */
let prerendered: Release | null = null;

export function setPrerenderedRelease(release: Release | null) {
  prerendered = release;
}

/** The release the page was built with, if it was. */
export function readReleaseSnapshot(): Release | null {
  if (prerendered) return prerendered;
  if (typeof document === "undefined") return null;
  const text = document.getElementById(SNAPSHOT_ELEMENT_ID)?.textContent;
  if (!text) return null;
  try {
    return toRelease(JSON.parse(text));
  } catch {
    return null;
  }
}

/** JSON that's safe inside a <script> element: no `<` can close it. */
export function snapshotJson(release: Release): string {
  return JSON.stringify(release).replace(/</g, "\\u003c");
}
