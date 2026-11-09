# AGENTS.md

Guide for coding agents working in this repo. Read [README.md](README.md) for what the app does.

## Shape

A pnpm workspace and a Cargo workspace in one repo. A Rust core does all the work; two thin shells
drive it. A marketing site sits beside them.

- `crates/core` (`mtg-art-core`) — parsing, Scryfall and its card data (`carddb`), MTGPics,
  MTGJSON's precon decks and their boxes' pictures (`decks`), the run loop, settings. Nothing in it
  knows about Tauri.
- `crates/cli` (`mtg-art`) — prints the core's events.
- `apps/desktop` (package `desktop`) — the desktop app.
  - `src-tauri` — Tauri commands over the core. A run reports progress on a `Channel<RunEvent>`
    passed to `start_run`.
  - `src` — React 19 + Chakra UI v3 + Zustand. It shows what the core reports and holds no logic of
    its own: no fetching, no parsing, no file writes.
- `packages/ui` (`@mtg-art/ui`) — the Chakra snippets, theme, provider and shared Storybook preview
  both apps use. Source only, no build step: import `@mtg-art/ui/<snippet>`, `@mtg-art/ui/theme`.
- `apps/site` (package `site`) — the marketing site at getmtg.art, on Cloudflare Pages: Vite, TanStack Router, and
  `vite-prerender-plugin`, which renders every page to static HTML after the build.

## Commands

| Task                           | Command                                                                     |
| ------------------------------ | --------------------------------------------------------------------------- |
| Task                           | Command                                                                     |
| ------------------------------ | --------------------------------------------------------------------------- |
| Install                        | `pnpm install` (also runs `agents:sync`; `corepack enable` once for pnpm)   |
| Desktop app                    | `pnpm tauri-dev`                                                            |
| Site                           | `pnpm site-dev` (port 3000); `pnpm --filter site build` prerenders it       |
| Storybooks                     | `pnpm storybook` (all three; VS Code starts them when the folder opens)     |
| Rust tests                     | `cargo test --workspace`                                                    |
| Rust lint and format           | `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all` |
| Front end tests                | `pnpm test` (unit tests, and every package's stories in headless Chromium)  |
| Front end typecheck and build  | `pnpm typecheck` / `pnpm build`                                             |
| Front end lint and format      | `pnpm lint` / `pnpm format` (oxlint and oxfmt, from the root)               |
| Chakra theme types             | `pnpm --filter @mtg-art/ui typegen` (the apps' dev, build and typecheck do) |
| Regenerate TypeScript bindings | `pnpm bindings`                                                             |
| A real run, no window          | `cargo run -p mtg-art-cli -- cards.txt --out <dir>`                         |
| Retake the app's screenshots   | `pnpm --filter desktop screenshots` (the `app-screenshots` skill)           |

Run one package's script with `pnpm --filter <desktop|site|@mtg-art/ui> <script>`. Shared dependency
versions live in the `catalog:` in `pnpm-workspace.yaml`; a package asks for one with `"catalog:"`,
so every package gets the same React, Chakra and Vite.

The Tauri crate embeds `apps/desktop/dist`, so `pnpm --filter desktop build` once before
`cargo clippy --workspace` on a fresh clone.

## Rules

- **Every request goes through `http::Http`.** It holds the rate limiters (Scryfall's per-endpoint
  limits, and a gap between MTGPics requests), retries, 429 handling and the User-Agent. One `Http`
  is shared by a whole run, and by the whole app, so concurrent jobs share one limit. Never call
  `reqwest` directly.
- **Look cards up in the card data before Scryfall.** `carddb` keeps Scryfall's `default_cards`
  bulk file in SQLite, refreshed daily; a run gets it as `Option<&CardDb>`. Ask Scryfall's API only
  for what it can't answer — searches, misspellings, cards newer than the copy, or no copy yet. A
  change to its schema bumps `SCHEMA_VERSION`, so existing copies are rebuilt. `history` logs
  where each printing's MTGPics art was found; both reach a run through `engine::Local`.
- **Wrong art is worse than Scryfall's art.** When MTGPics can't be matched with confidence — the
  page's title must name the card and its credit must match the artist — fall back to Scryfall.
  Never match on a name alone: one set can print a name many times.
- **The front end never passes URLs or file paths to download.** It sends text input and settings;
  the core builds every URL and path, and file names go through `naming::sanitize`.
- **The finder window is the user's to drive.** `apps/desktop/src-tauri/src/finder.rs` opens the
  page the core chose (`manual::site_url`: Google Lens on an art crop, a search for the artist, or
  the website MTGPics links for them) and then listens for downloads. It reads a page only when the
  user picks **Find the art on this page** from the window's menu, and then only the addresses and
  sizes of its images (`PAGE_IMAGES`); it never clicks, scrolls or navigates for the user, and reads
  nothing else. The only thing it adds to a page is its own toast in the corner (`finder::toast`),
  set as text in a shadow root, to say what came of an image. Images reach the core from
  `apps/desktop/src-tauri` (`manual::import`, `manual::fetch_page_images`), never from React: the
  main window is shown a page's images to choose from, and answers with an index. One `verify`
  doesn't accept is only saved if the user says so. Its pages get no capabilities: `capabilities/`
  names only `main`.
- **Art from anywhere but MTGPics and Scryfall goes through `verify`**, which checks it's Scryfall's
  art and larger. Tune its thresholds with `cargo run --release -p mtg-art-core --example verify`
  on real images kept outside the repo; the tests use generated ones.
- **Types the front end reads derive `TS` with `#[ts(export)]`.** After changing one, run
  `pnpm bindings` and commit `apps/desktop/src/bindings`. Don't edit those files by hand.
- **Tests don't touch the network.** Unit tests read saved pages in `crates/core/tests/fixtures`
  (MTGPics pages are ISO-8859-1); `crates/core/tests/run.rs` runs the engine against `wiremock`
  servers. Save a new real page as a fixture rather than hand-writing one where you can.
- **Chakra UI v3**, not v2: compound components (`Drawer.Root`, `Switch.Root`…) and
  `colorPalette`. Check an unfamiliar component's API with the `chakra-ui` MCP server before using
  it, and load the `chakra-ui-builder` skill (or `chakra-ui-refactor`) for UI work.
- **Reach for a snippet before a compound component.** `packages/ui/src/components/ui` holds
  Chakra's snippets (`Field`, `Switch`, `Radio`, `NativeSelect*`, `SegmentedControl`, `Drawer*`,
  `EmptyState`, `Alert`, `Progress*`, `Tooltip`…), shared by both apps, so a component writes
  `<Switch checked>Label</Switch>`, not the five parts behind it. oxlint enforces it: importing a
  component that has a snippet (or could have one) from `@chakra-ui/react` is an error outside
  `packages/ui/src` and `.storybook`. Before writing UI, check what's there with the
  `storybook-ui` MCP's `docs-list`. Add a missing snippet with
  `npx @chakra-ui/cli snippet add <name> --tsx --outdir packages/ui/src/components/ui`, convert it
  to React 19 style (`ref` as a prop, no `forwardRef`, no `"use client"`), import within the
  package by relative path, and give it a story so `docs-show` documents it.
- **Theme tokens, recipes and slot recipes live in `packages/ui/src/theme.ts`**, one system for both
  apps. Run `pnpm --filter @mtg-art/ui typegen` after changing it so their props are typed. Use
  semantic tokens (`fg.muted`, `bg.subtle`…) over raw colours so both colour modes work.
- **In the desktop app, errors go through `showError`** (`apps/desktop/src/notify.ts`), and a
  select, segmented control or tab list over a string union narrows its value with `findOption`
  (`apps/desktop/src/utils/options.ts`). Its dialogs and drawers come from
  `@/components/ui/dialog` and `@/components/ui/drawer`, which keep the header usable while one
  is open; oxlint bans the shared ones there.
- **Screenshots follow the UI.** `docs/screenshots`, which the README and the site show, are
  taken from real runs by `pnpm --filter desktop screenshots`. A change that alters what one
  shows (theme, colours, fonts, the panels, the run table, the settings drawer, wording) retakes
  it in the same change; the `app-screenshots` skill says how, and how to check each image.
- **The download page never names a file.** It fetches the latest release from GitHub and
  `apps/site/src/downloads/artifacts.ts` sorts its files with a table of rules. Files no rule knows
  are listed as "Other files", so a new kind of file shows up on its own; to give it a label and a
  place on its system's tab, add a rule there and a case to `artifacts.test.ts`. Never hard-code an
  asset name or version in a component.
- **The site never creates a GitHub release or a `v*` tag.** The app's updater and the download
  page read `releases/latest`, which must always be the app's. The site deploys from `trunk` on
  its own (`.github/workflows/site.yml`); the app ships by tag (`release.yml`).
- **Types over casts** in TypeScript: narrow with a guard instead of `as`.

## Agent config and MCP servers

`AGENTS.md`, `.agents/skills/` and `.agents/mcp.json` are the sources. `pnpm agents:sync` (run on
install, checkout and merge) generates `CLAUDE.md`, `.claude/skills/`, and the MCP configs for
Claude Code (`.mcp.json`), Cursor (`.cursor/mcp.json`) and VS Code (`.vscode/mcp.json`). Those
are gitignored: edit the sources, never the generated files. The Chakra skills come from
`npx skills add chakra-ui/chakra-ui` and are pinned in `skills-lock.json`.

- `chakra-ui` — Chakra's component docs, props and examples.
- `storybook-ui`, `storybook-desktop`, `storybook-site` — served by `@storybook/addon-mcp` at
  `http://localhost:6008/mcp` (`packages/ui`), `6009` (`apps/desktop`) and `6010` (`apps/site`), so
  only while that Storybook is running. Use the one for the package you're changing, and
  `storybook-ui` for the snippets.

When working on UI components:

- Use the `storybook` MCP tools before answering questions about or changing UI components.
- Never guess component props. Call `docs-list` to see documented components and `docs-show` for a
  component's props, and use only props that are documented or shown in example stories. Story
  names don't always match prop names. If a prop isn't documented, ask the user instead of
  assuming. `docs-show` prints imports from `'mtg-art-downloader'`; the real import is
  `@mtg-art/ui/<file>` for a snippet or `@/components/<Component>` for an app's own component.
- Call `get-storybook-story-instructions` before creating or updating stories.
- Run `test-run` to verify your changes.

## Storybook stories

Stories sit next to their component (`RunView.tsx` and `RunView.stories.tsx`) and every one runs
as a test under `pnpm test`. Each package has its own Storybook: `packages/ui` for the snippets
(port 6008), `apps/desktop` (6009) and `apps/site` (6010). The app Storybooks compose the library's
as a "UI library" ref, so they show its stories too while it runs. Shared preview setup (theme
switcher, Chakra provider, toaster, a11y settings) is `@mtg-art/ui/storybook`; each package's
`.storybook/preview.tsx` spreads `basePreview` and calls `resetUi()` in its `beforeEach`.

- Write or update a story when you create a component or a snippet, or add props, states or
  variants to one. Cover each state the component can reach (empty, loading, running, finished,
  error…), not every prop combination.
- Add a `play` function that checks the behaviour a story is about, with role- or label-based
  queries. Chakra overlays (drawers, tooltips, toasts) render in a portal: query them with
  `screen`, not `canvas`, and `waitFor` them to finish animating in.
- Don't wrap stories in a provider or add a `<Toaster />`; `.storybook/preview.tsx` does, and the
  site's also puts every story inside a router so router links work. Don't add
  `tags: ["autodocs"]` or a `title`; titles come from the file path.
- **There is no Tauri in Storybook.** The desktop preview mocks `src/api.ts` and every `@tauri-apps/*`
  module the app imports with `sb.mock`, and gives each a default answer. Override one in the
  story's `beforeEach` with `mocked(api.countSearch).mockResolvedValue(…)` and assert calls with
  `expect(api.startRun).toHaveBeenCalledWith(…)`. If a component starts importing another
  `@tauri-apps/*` module, add it to the preview's `sb.mock` list and defaults.
- The desktop's Zustand stores are reset before every story. Seed one in `beforeEach` with
  `useRun.setState({ … })`. Typed sample data lives in `apps/desktop/src/__fixtures__`; build it
  from the `src/bindings` types. The site's sample releases are in
  `apps/site/src/downloads/__fixtures__`, built on a real release saved from GitHub's API.

## Accessibility

Storybook's a11y addon runs axe on every story. Violations are reported in the a11y panel and by
the MCP's `test-run`, but don't fail tests (`a11y.test: "todo"` in `.storybook/preview.tsx`).

- Fix structural issues yourself: labels, roles, `aria-label` on icon-only buttons, alt text,
  table headers (`VisuallyHidden` for a column with no visible title), heading order.
- Ask before changing anything visual, such as colour contrast, sizes or focus styles: describe
  the issue and offer two or three options.
- Don't turn rules off to make a report go away.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `feat(core): …`,
`fix(mtgpics): …`, `feat(site): …`, `refactor(ui): …`, `chore(ci): …`, in the imperative, lower case, no trailing period. The default
branch is `trunk`.
