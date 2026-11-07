# AGENTS.md

Guide for coding agents working in this repo. Read [README.md](README.md) for what the app does.

## Shape

A Rust core does all the work; two thin shells drive it.

- `crates/core` (`mtg-art-core`) — parsing, Scryfall, MTGPics, the run loop, settings. Nothing in it
  knows about Tauri.
- `crates/cli` (`mtg-art`) — prints the core's events.
- `src-tauri` — Tauri commands over the core. A run reports progress on a `Channel<RunEvent>` passed
  to `start_run`.
- `src` — React 19 + Chakra UI v3 + Zustand. It shows what the core reports and holds no logic of
  its own: no fetching, no parsing, no file writes.

## Commands

| Task                           | Command                                                                     |
| ------------------------------ | --------------------------------------------------------------------------- |
| Install                        | `npm install`                                                               |
| Desktop app                    | `npm run tauri-dev`                                                         |
| Rust tests                     | `cargo test --workspace`                                                    |
| Rust lint and format           | `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --all` |
| Front end typecheck and build  | `npm run build`                                                             |
| Front end lint and format      | `npm run lint` / `npm run format`                                           |
| Regenerate TypeScript bindings | `npm run bindings`                                                          |
| A real run, no window          | `cargo run -p mtg-art-cli -- cards.txt --out <dir>`                         |

The Tauri crate embeds `dist/`, so `npm run build` once before `cargo clippy --workspace` on a fresh
clone.

## Rules

- **Every request goes through `http::Http`.** It holds the rate limiters (Scryfall's per-endpoint
  limits, and a gap between MTGPics requests), retries, 429 handling and the User-Agent. One `Http`
  is shared by a whole run, and by the whole app, so concurrent jobs share one limit. Never call
  `reqwest` directly.
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
  it — it's configured in `.mcp.json`, `.vscode/mcp.json` and `.cursor/mcp.json`; keep the three in
  step.
- **Reach for a snippet before a compound component.** `src/components/ui` holds Chakra's snippets
  (`Field`, `Switch`, `Radio`, `NativeSelect*`, `SegmentedControl`, `Drawer*`, `EmptyState`,
  `Alert`, `Progress*`, `Tooltip`…), so a component writes `<Switch checked>Label</Switch>`, not the
  five parts behind it. Add a missing one with
  `npx @chakra-ui/cli snippet add <name> --tsx --outdir src/components/ui`, then convert it to
  React 19 style: `ref` as a prop, no `forwardRef`, no `"use client"`.
- **Errors go through `showError`** (`src/notify.ts`), and a select, segmented control or tab list
  over a string union narrows its value with `findOption` (`src/utils/options.ts`).
- **Types over casts** in TypeScript: narrow with a guard instead of `as`.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `feat(core): …`,
`fix(mtgpics): …`, `chore(ci): …`, in the imperative, lower case, no trailing period. The default
branch is `trunk`.
