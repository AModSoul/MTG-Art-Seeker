// Runs in Node after `vite build` (vite-prerender-plugin), once per page: it renders the page to
// HTML and reports the same-site links it found, which are prerendered in turn.

import createCache from "@emotion/cache";
import { CacheProvider } from "@emotion/react";
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { renderToString } from "react-dom/server";
import type { PrerenderArguments, PrerenderResult } from "vite-prerender-plugin";

import { documentTitleFromMatches } from "./document-title";
import { createAppRouter } from "./router";
import { APP_NAME, BASE_PATH } from "./site-config";

/** vite-prerender-plugin's head element, which it doesn't export. */
type HeadElement = { type: string; props: Record<string, string> };

/** The plugin names pages from the site's root; the router and the HTML include the base path. */
const withBase = (url: string) => BASE_PATH + url.replace(/^\//, "");

/** A route's `<meta>` entry as attributes, which are all strings. */
function stringProps(entry: object): Record<string, string> {
  return Object.fromEntries(
    Object.entries(entry).filter((pair): pair is [string, string] => typeof pair[1] === "string"),
  );
}

function withoutBase(href: string): string | null {
  const { pathname, origin } = new URL(href, "http://localhost");
  if (origin !== "http://localhost" || !pathname.startsWith(BASE_PATH)) return null;
  return `/${pathname.slice(BASE_PATH.length)}`;
}

export async function prerender({ url }: PrerenderArguments): Promise<PrerenderResult> {
  const router = createAppRouter({
    history: createMemoryHistory({ initialEntries: [withBase(url)] }),
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
  const links = new Set(
    parseLinks(html).flatMap((href) => {
      const page = withoutBase(href);
      return page ? [page] : [];
    }),
  );

  const meta = router.state.matches.flatMap((match) => match.meta ?? []);
  const elements = new Set<HeadElement>(
    meta.flatMap((entry) =>
      // The title goes in head.title; every other entry is a <meta> tag.
      entry && !("title" in entry) ? [{ type: "meta", props: stringProps(entry) }] : [],
    ),
  );

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
