// Runs in Node after `vite build` (vite-prerender-plugin), once per page: it renders the page to
// HTML and reports the links it found, which are prerendered in turn.

import createCache from "@emotion/cache";
import { CacheProvider } from "@emotion/react";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { renderToString } from "react-dom/server";
import type { PrerenderArguments, PrerenderResult } from "vite-prerender-plugin";

import { documentTitleFromMatches } from "./document-title";
import { fetchLatestRelease, type Release } from "./downloads/github";
import { SNAPSHOT_ELEMENT_ID, setPrerenderedRelease, snapshotJson } from "./downloads/snapshot";
import { createAppRouter } from "./router";
import { APP_NAME } from "./site-config";

/** vite-prerender-plugin's head element, which it doesn't export. */
type HeadElement = { type: string; props: Record<string, string>; children?: string };

/** Pages that show the latest release, and so carry a snapshot of it. */
const RELEASE_PAGES = new Set(["/download"]);

let release: Promise<Release | null> | undefined;

/**
 * The latest release, fetched once per build. A build that can't reach GitHub still succeeds: the
 * page then loads the release in the browser. CI passes GITHUB_TOKEN for a higher rate limit.
 */
function latestRelease(): Promise<Release | null> {
  release ??= fetchLatestRelease({ token: process.env.GITHUB_TOKEN }).catch((error: unknown) => {
    console.warn(`Prerendering without a release snapshot: ${String(error)}`);
    return null;
  });
  return release;
}

/** A route's `<meta>` entry as attributes, which are all strings. */
function stringProps(entry: object): Record<string, string> {
  return Object.fromEntries(
    Object.entries(entry).filter((pair): pair is [string, string] => typeof pair[1] === "string"),
  );
}

export async function prerender({ url }: PrerenderArguments): Promise<PrerenderResult> {
  const snapshot = RELEASE_PAGES.has(url) ? await latestRelease() : null;
  setPrerenderedRelease(snapshot);

  const router = createAppRouter({
    history: createMemoryHistory({ initialEntries: [url] }),
    isServer: true,
  });
  // The server render path never mounts the router's Transitioner, so nothing matches until the
  // router has loaded.
  await router.load();

  // The browser's default Emotion key, so the client's styles take over the prerendered ones.
  const cache = createCache({ key: "css", prepend: true });
  const html = renderToString(
    <CacheProvider value={cache}>
      <RouterProvider router={router} />
    </CacheProvider>,
  );

  const { parseLinks } = await import("vite-prerender-plugin/parse");
  // The plugin skips links to other sites itself.
  const links = new Set(parseLinks(html));

  const meta = router.state.matches.flatMap((match) => match.meta ?? []);
  const elements = new Set<HeadElement>(
    meta.flatMap((entry) =>
      // The title goes in head.title; every other entry is a <meta> tag.
      entry && !("title" in entry) ? [{ type: "meta", props: stringProps(entry) }] : [],
    ),
  );
  if (snapshot) {
    elements.add({
      type: "script",
      props: { type: "application/json", id: SNAPSHOT_ELEMENT_ID },
      children: snapshotJson(snapshot),
    });
  }
  setPrerenderedRelease(null);

  return {
    html,
    links,
    head: {
      lang: "en",
      title: documentTitleFromMatches(router.state.matches) ?? APP_NAME,
      elements,
    },
  };
}
