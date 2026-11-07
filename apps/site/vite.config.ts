/// <reference types="vitest/config" />
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, URL } from "node:url";

import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig, type Plugin } from "vite";
import { vitePrerenderPlugin } from "vite-prerender-plugin";

import { BASE_PATH } from "./src/site-config";

const dirname = path.dirname(fileURLToPath(import.meta.url));
const PRERENDER_SCRIPT = path.join(dirname, "src/prerender.tsx");

/**
 * The directory of `pkg`'s `package.json`, resolved from `from`'s. pnpm only links a package's
 * own dependencies next to it, so Emotion's internals resolve from `@emotion/react`.
 */
function packageDir(pkg: string, from = dirname) {
  const require = createRequire(path.join(from, "package.json"));
  return path.dirname(require.resolve(`${pkg}/package.json`));
}

const emotionReact = packageDir("@emotion/react");
const routerCore = packageDir("@tanstack/router-core", packageDir("@tanstack/react-router"));

/**
 * Drops the chunks only the prerender used (it and `react-dom/server`, `parse`), once it has run:
 * nothing the pages load reaches them.
 */
function dropPrerenderChunks(): Plugin {
  return {
    name: "drop-prerender-chunks",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const chunks = Object.values(bundle).filter((output) => output.type === "chunk");
      const reachable = new Set<string>();
      const visit = (fileName: string) => {
        if (reachable.has(fileName)) return;
        reachable.add(fileName);
        const chunk = bundle[fileName];
        if (chunk?.type !== "chunk") return;
        [...chunk.imports, ...chunk.dynamicImports].forEach(visit);
      };
      chunks
        .filter((chunk) => chunk.isEntry && chunk.facadeModuleId !== PRERENDER_SCRIPT)
        .forEach((chunk) => visit(chunk.fileName));
      for (const chunk of chunks) {
        if (!reachable.has(chunk.fileName)) delete bundle[chunk.fileName];
      }
    },
  };
}

/**
 * The prerender imports the built bundle into Node, which can leave file handles open so the
 * process never exits although the build is done and written.
 */
function exitAfterBuild(): Plugin {
  return {
    name: "exit-after-build",
    apply: "build",
    enforce: "post",
    closeBundle() {
      if (this.meta.watchMode) return;
      setImmediate(() => process.exit(0));
    },
  };
}

export default defineConfig(({ command }) => ({
  base: BASE_PATH,
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      // Prerendering runs the bundle in Node, so these resolve to the builds that work without a
      // DOM. The browser builds of Emotion touch `document` at import time, wait for
      // `useInsertionEffect` (which `renderToString` never runs, so the HTML had class names but
      // no <style> tags), and hard-code `isBrowser`.
      "@emotion/cache": path.join(packageDir("@emotion/cache"), "dist/emotion-cache.esm.js"),
      "@emotion/use-insertion-effect-with-fallbacks": path.join(
        packageDir("@emotion/use-insertion-effect-with-fallbacks", emotionReact),
        "dist/emotion-use-insertion-effect-with-fallbacks.esm.js",
      ),
      "@emotion/utils": path.join(
        packageDir("@emotion/utils", emotionReact),
        "dist/emotion-utils.esm.js",
      ),
      "@emotion/react": path.join(emotionReact, "dist/emotion-react.esm.js"),
      // The client build exports `isServer = false`, which overrides the router's own `isServer`
      // and leaves Suspense boundaries (empty HTML) in the prerender. The development build
      // exports `undefined`, so each router's option applies: false in the browser, true in Node.
      "@tanstack/router-core/isServer": path.join(routerCore, "dist/esm/isServer/development.js"),
    },
  },
  plugins: [
    tanstackRouter({
      target: "react",
      // Route modules load eagerly so `renderToString` has every component synchronously.
      autoCodeSplitting: false,
    }),
    react(),
    // Storybook's config drops these from its own build. The prerender script is given here
    // rather than as a <script prerender> tag, which would bundle it into the pages' JavaScript.
    ...(command === "build"
      ? [
          ...vitePrerenderPlugin({
            renderTarget: "#root",
            prerenderScript: PRERENDER_SCRIPT,
          }),
          dropPrerenderChunks(),
          exitAfterBuild(),
        ]
      : []),
  ],
  build: {
    target: "es2022",
  },
  test: {
    projects: [
      {
        extends: true,
        test: { name: "unit", environment: "node", include: ["src/**/*.test.ts"] },
      },
      {
        // Every story is a test: it must render, its play function must pass,
        // and the a11y addon reports what axe finds.
        extends: true,
        plugins: [
          storybookTest({
            configDir: fileURLToPath(new URL("./.storybook", import.meta.url)),
            storybookUrl: "http://localhost:6010",
          }),
        ],
        test: {
          name: "storybook",
          browser: {
            enabled: true,
            headless: true,
            provider: playwright({}),
            instances: [{ browser: "chromium" }],
          },
        },
      },
    ],
  },
}));
