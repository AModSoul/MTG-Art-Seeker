/**
 * The latest published release of the app, from GitHub's REST API. The API allows cross-origin
 * requests, so the browser asks it directly; the prerender asks it at build time.
 */

import { GITHUB_OWNER, GITHUB_REPO } from "@/site-config";

export type ReleaseAsset = {
  name: string;
  /** Bytes. */
  size: number;
  browser_download_url: string;
};

export type Release = {
  tag_name: string;
  name: string | null;
  published_at: string | null;
  html_url: string;
  assets: ReleaseAsset[];
};

export const LATEST_RELEASE_API_URL = `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest`;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

const isStringOrNull = (value: unknown): value is string | null =>
  typeof value === "string" || value === null;

function toAsset(value: unknown): ReleaseAsset | null {
  if (!isRecord(value)) return null;
  const { name, size, browser_download_url } = value;
  if (typeof name !== "string" || typeof size !== "number") return null;
  if (typeof browser_download_url !== "string") return null;
  return { name, size, browser_download_url };
}

/**
 * The fields the download page reads from a release, or `null` when `value` isn't one. Keeps only
 * those fields, so a release serialised into the page carries nothing else.
 */
export function toRelease(value: unknown): Release | null {
  if (!isRecord(value)) return null;
  const { tag_name, name, published_at, html_url, assets } = value;
  if (typeof tag_name !== "string" || typeof html_url !== "string") return null;
  if (!isStringOrNull(name) || !isStringOrNull(published_at)) return null;
  if (!Array.isArray(assets)) return null;
  return {
    tag_name,
    name,
    published_at,
    html_url,
    assets: assets.flatMap((asset) => {
      const parsed = toAsset(asset);
      return parsed ? [parsed] : [];
    }),
  };
}

/** Rejects when GitHub can't be reached or answers with something other than a release. */
export async function fetchLatestRelease(
  options: { signal?: AbortSignal; token?: string } = {},
): Promise<Release> {
  const response = await fetch(LATEST_RELEASE_API_URL, {
    signal: options.signal,
    headers: {
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      ...(options.token ? { Authorization: `Bearer ${options.token}` } : {}),
    },
  });
  if (response.status === 404) throw new Error("There's no published release yet.");
  if (!response.ok) throw new Error(`GitHub answered ${response.status}.`);
  const release = toRelease(await response.json());
  if (!release) throw new Error("GitHub's answer wasn't a release.");
  return release;
}
