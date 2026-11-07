# AGENTS.md

Guide for coding agents working in this repo. Read [README.md](README.md) for what the app does.

## Shape

A Rust core does all the work; two thin shells drive it.

- `crates/core` (`mtg-art-core`) — parsing, Scryfall and its card data (`carddb`), MTGPics, the run
  loop, settings. Nothing in it knows about Tauri.
- `crates/cli` (`mtg-art`) — prints the core's events.
- `src-tauri` — Tauri commands over the core. A run reports progress on a `Channel<RunEvent>` passed
  to `start_run`.
- `src` — React 19 + Chakra UI v3 + Zustand. It shows what the core reports and holds no logic of
  its own: no fetching, no parsing, no file writes.

## Commands

| Task                           | Command                                                                     |
| ------------------------------ | --------------------------------------------------------------------------- |
| Install                        | `npm install` (also runs `typegen` and `agents:sync`)                       |
| Desktop app                    | `npm run tauri-dev`                                                         |
| Storybook                      | `npm run storybook` (port 6008; VS Code starts it when the folder opens)    |
| Rust tests                     | `cargo test --workspace`                                                    |
| Rust lint and format           | `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all` |
| Front end tests                | `npm test` (unit tests, and every story in headless Chromium)               |
| Front end typecheck and build  | `npm run typecheck` / `npm run build`                                       |
| Front end lint and format      | `npm run lint` / `npm run format` (oxlint and oxfmt)                        |
| Chakra theme types             | `npm run typegen`                                                           |
| Regenerate TypeScript bindings | `npm run bindings`                                                          |
| A real run, no window          | `cargo run -p mtg-art-cli -- cards.txt --out <dir>`                         |

The Tauri crate embeds `dist/`, so `npm run build` once before `cargo clippy --workspace` on a fresh
clone.

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
- **Types the front end reads derive `TS` with `#[ts(export)]`.** After changing one, run
  `npm run bindings` and commit `src/bindings`. Don't edit those files by hand.
- **Tests don't touch the network.** Unit tests read saved pages in `crates/core/tests/fixtures`
  (MTGPics pages are ISO-8859-1); `crates/core/tests/run.rs` runs the engine against `wiremock`
  servers. Save a new real page as a fixture rather than hand-writing one where you can.
- **Chakra UI v3**, not v2: compound components (`Drawer.Root`, `Switch.Root`…) and
  `colorPalette`. Check an unfamiliar component's API with the `chakra-ui` MCP server before using
  it, and load the `chakra-ui-builder` skill (or `chakra-ui-refactor`) for UI work.
- **Reach for a snippet before a compound component.** `src/components/ui` holds Chakra's snippets
  (`Field`, `Switch`, `Radio`, `NativeSelect*`, `SegmentedControl`, `Drawer*`, `EmptyState`,
  `Alert`, `Progress*`, `Tooltip`…), so a component writes `<Switch checked>Label</Switch>`, not the
  five parts behind it. oxlint enforces it: importing a component that has a snippet (or could
  have one) from `@chakra-ui/react` is an error outside `src/components/ui`, `src/theme.ts` and
  `.storybook`. Before writing UI, check what's there with the Storybook MCP's `docs-list`. Add a
  missing snippet with `npx @chakra-ui/cli snippet add <name> --tsx --outdir src/components/ui`,
  convert it to React 19 style (`ref` as a prop, no `forwardRef`, no `"use client"`), and give it a
  story so `docs-show` documents it.
- **Theme tokens, recipes and slot recipes live in `src/theme.ts`.** Run `npm run typegen` after
  changing it so their props are typed. Use semantic tokens (`fg.muted`, `bg.subtle`…) over raw
  colours so both colour modes work.
- **Errors go through `showError`** (`src/notify.ts`), and a select, segmented control or tab list
  over a string union narrows its value with `findOption` (`src/utils/options.ts`).
- **Types over casts** in TypeScript: narrow with a guard instead of `as`.

## Agent config and MCP servers

`AGENTS.md`, `.agents/skills/` and `.agents/mcp.json` are the sources. `npm run agents:sync` (run on
install, checkout and merge) generates `CLAUDE.md`, `.claude/skills/`, and the MCP configs for
Claude Code (`.mcp.json`), Cursor (`.cursor/mcp.json`) and VS Code (`.vscode/mcp.json`). Those
are gitignored: edit the sources, never the generated files. The Chakra skills come from
`npx skills add chakra-ui/chakra-ui` and are pinned in `skills-lock.json`.

- `chakra-ui` — Chakra's component docs, props and examples.
- `storybook` — served by `@storybook/addon-mcp` at `http://localhost:6008/mcp`, so only while
  Storybook is running.

When working on UI components:

- Use the `storybook` MCP tools before answering questions about or changing UI components.
- Never guess component props. Call `docs-list` to see documented components and `docs-show` for a
  component's props, and use only props that are documented or shown in example stories. Story
  names don't always match prop names. If a prop isn't documented, ask the user instead of
  assuming. `docs-show` prints imports from `'mtg-art-downloader'`; the real import is
  `@/components/ui/<file>` or `@/components/<Component>`.
- Call `get-storybook-story-instructions` before creating or updating stories.
- Run `test-run` to verify your changes.

## Storybook stories

Stories sit next to their component (`RunView.tsx` and `RunView.stories.tsx`) and every one runs
as a test under `npm test`.

- Write or update a story when you create a component or a snippet, or add props, states or
  variants to one. Cover each state the component can reach (empty, loading, running, finished,
  error…), not every prop combination.
- Add a `play` function that checks the behaviour a story is about, with role- or label-based
  queries. Chakra overlays (drawers, tooltips, toasts) render in a portal: query them with
  `screen`, not `canvas`, and `waitFor` them to finish animating in.
- Don't wrap stories in a provider or add a `<Toaster />`; `.storybook/preview.tsx` does. Don't
  add `tags: ["autodocs"]` or a `title`; titles come from the file path.
- **There is no Tauri in Storybook.** The preview mocks `src/api.ts` and every `@tauri-apps/*`
  module the app imports with `sb.mock`, and gives each a default answer. Override one in the
  story's `beforeEach` with `mocked(api.countSearch).mockResolvedValue(…)` and assert calls with
  `expect(api.startRun).toHaveBeenCalledWith(…)`. If a component starts importing another
  `@tauri-apps/*` module, add it to the preview's `sb.mock` list and defaults.
- The Zustand stores are reset before every story. Seed one in `beforeEach` with
  `useRun.setState({ … })`. Typed sample data lives in `src/__fixtures__`; build it from the
  `src/bindings` types.

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
`fix(mtgpics): …`, `chore(ci): …`, in the imperative, lower case, no trailing period. The default
branch is `trunk`.
