// Takes the README's and the site's screenshots of the app (docs/screenshots) from real runs.
//
//   pnpm --filter desktop screenshots [--record] [scene...]
//
// A run is recorded once by `crates/core/examples/record.rs` into target/screenshots, really
// downloading the art; `--record` records again. Then the app's front end is served with
// `api.ts` and `tauri.ts` in place of Tauri, fed the recording, and each scene is shot in
// headless Chromium at 1200 × 800 and 150%, as on a Windows laptop. Name scenes to take only
// those.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

import react from "@vitejs/plugin-react";
import { chromium, type Page } from "playwright";
import { createServer, type Plugin } from "vite";

const here = import.meta.dirname;
const desktop = path.resolve(here, "..");
const root = path.resolve(desktop, "../..");
const recordings = path.join(root, "target/screenshots");
const output = path.join(root, "docs/screenshots");

type Scheme = "light" | "dark";

interface Scene {
  /** The file is `<name>-<scheme>.png`. */
  name: string;
  recording: "list" | "deck";
  schemes: Scheme[];
  /** What to do once the run has finished, before the shot. */
  act?: (page: Page) => Promise<void>;
}

const SCENES: Scene[] = [
  { name: "results", recording: "list", schemes: ["light", "dark"] },
  { name: "decks", recording: "deck", schemes: ["light", "dark"] },
  {
    name: "find-copy",
    recording: "list",
    schemes: ["dark"],
    act: async (page) => {
      await page
        .getByRole("button", { name: "Search Google Lens for this art, for Sol Ring" })
        .hover();
      await page.getByText("Search Google Lens for this art", { exact: true }).waitFor();
    },
  },
  {
    name: "sources",
    recording: "list",
    schemes: ["dark"],
    act: async (page) => {
      await page
        .getByRole("row", { name: /Karn, Scion of Urza/ })
        .click({ button: "right", position: { x: 128, y: 45 } });
      await page.getByText("Show in folder").waitFor();
    },
  },
  {
    name: "settings",
    recording: "list",
    schemes: ["dark"],
    act: async (page) => {
      await page.getByRole("button", { name: "Settings" }).click();
      await page.getByRole("dialog", { name: "Settings" }).waitFor();
      // The drawer opens with its first field focused and selected, and its close button under
      // the pointer; neither is what it looks like at rest.
      await page.evaluate(() => {
        const field = document.activeElement;
        if (field instanceof HTMLInputElement) field.setSelectionRange(0, 0);
        if (field instanceof HTMLElement) field.blur();
      });
      await page.mouse.move(0, 799);
    },
  },
];

/** What each recording is a run of. The deck is found as the picker would find it. */
const RECORD_ARGS: Record<Scene["recording"], string[]> = {
  list: ["list", path.join(here, "cards.txt")],
  deck: ["deck", "set:eoc", "WorldShaper_EOC", "World Shaper"],
};

const args = process.argv.slice(2);
const rerecord = args.includes("--record");
const names = args.filter((arg) => !arg.startsWith("--"));
const unknown = names.filter((name) => !SCENES.some((scene) => scene.name === name));
if (unknown.length > 0) {
  throw new Error(
    `No scene ${unknown.join(", ")}; there are ${SCENES.map((s) => s.name).join(", ")}`,
  );
}
const scenes = SCENES.filter((scene) => names.length === 0 || names.includes(scene.name));

// Record each run the scenes need, unless it already has been.
const recordingFile = (name: Scene["recording"]) => path.join(recordings, `${name}.json`);
for (const name of new Set(scenes.map((scene) => scene.recording))) {
  if (!rerecord && existsSync(recordingFile(name))) continue;
  console.log(`Recording a ${name} run…`);
  const cargo = spawnSync(
    "cargo",
    [
      "run",
      "--release",
      "--quiet",
      "-p",
      "mtg-art-core",
      "--example",
      "record",
      "--",
      ...RECORD_ARGS[name],
      path.join(recordings, name),
      recordingFile(name),
    ],
    { cwd: root, stdio: "inherit" },
  );
  if (cargo.status !== 0) throw new Error(`Recording the ${name} run failed`);
}
const recorded = new Map(
  [...new Set(scenes.map((scene) => scene.recording))].map((name) => [
    name,
    readFileSync(recordingFile(name), "utf8"),
  ]),
);

// The app, with the stand-ins swapped in for the modules that reach Tauri.
const swaps: Record<string, string> = {
  "@/api": path.join(here, "api.ts"),
  "@tauri-apps/api/core": path.join(here, "tauri.ts"),
  "@tauri-apps/plugin-dialog": path.join(here, "tauri.ts"),
  "@tauri-apps/plugin-process": path.join(here, "tauri.ts"),
  "@tauri-apps/plugin-updater": path.join(here, "tauri.ts"),
};
const harness: Plugin = {
  name: "screenshots",
  enforce: "pre",
  resolveId: (source) => swaps[source],
  transformIndexHtml: (html) => html.replace("/src/main.tsx", "/screenshots/main.tsx"),
};
// The saved art is read off the disk: the recordings' folder, and the decks' boxes' pictures,
// which are kept where the app keeps them.
const boxDirs = [...recorded.values()].flatMap((text) => {
  const { boxes }: { boxes?: Record<string, string | null> } = JSON.parse(text);
  return Object.values(boxes ?? {}).flatMap((box) => (box ? [path.dirname(box)] : []));
});
const server = await createServer({
  configFile: false,
  root: desktop,
  logLevel: "warn",
  plugins: [harness, react()],
  resolve: { alias: [{ find: /^@\/(?!api$)/, replacement: `${path.join(desktop, "src")}/` }] },
  server: { port: 1430, fs: { allow: [root, ...boxDirs] } },
});
await server.listen();
const url = server.resolvedUrls?.local[0];
if (!url) throw new Error("The dev server didn't say where it's listening");

const browser = await chromium.launch();
try {
  for (const scene of scenes) {
    for (const scheme of scene.schemes) {
      const page = await browser.newPage({
        viewport: { width: 1200, height: 800 },
        deviceScaleFactor: 1.5,
        colorScheme: scheme,
      });
      page.on("pageerror", (error) => console.error(`${scene.name}: ${error.message}`));
      await page.addInitScript(
        (text) => {
          window.screenshotRecording = JSON.parse(text);
        },
        recorded.get(scene.recording) ?? "",
      );
      await page.goto(url);
      await page.getByRole("button", { name: "Download art" }).click();
      await page.getByText("Finished", { exact: true }).waitFor();
      // Every image on screen has loaded; those below are lazy, and never will.
      await page.waitForFunction(() =>
        [...document.images]
          .filter((image) => {
            const box = image.getBoundingClientRect();
            return box.width > 0 && box.bottom > 0 && box.top < window.innerHeight;
          })
          .every((image) => image.complete && image.naturalWidth > 0),
      );
      // Out of the way, so nothing shows as hovered unless the scene hovers it.
      await page.mouse.move(0, 799);
      await scene.act?.(page);
      // Let tooltips, menus and drawers finish animating in.
      await page.waitForTimeout(600);
      const file = path.join(output, `${scene.name}-${scheme}.png`);
      await page.screenshot({ path: file });
      console.log(path.relative(root, file));
      await page.close();
    }
  }
} finally {
  await browser.close();
  await server.close();
}
