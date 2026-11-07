import { createRouter, type RouterHistory } from "@tanstack/react-router";

import { routeTree } from "./routeTree.gen";

/** The router both the browser and the prerender use; the prerender passes a memory history. */
export function createAppRouter(options: { history?: RouterHistory; isServer?: boolean } = {}) {
  return createRouter({
    routeTree,
    scrollRestoration: !options.isServer,
    defaultPreload: options.isServer ? false : "intent",
    defaultPreloadStaleTime: 0,
    ...options,
  });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
