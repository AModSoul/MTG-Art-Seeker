---
name: app-screenshots
description: >
  Take or retake the desktop app's screenshots in docs/screenshots, which the README and the
  marketing site's front page show. Use whenever the user asks to take, update, retake or refresh
  screenshots of the app, the README's or the site's images, or to add a new screenshot; and
  after any change that alters what a screenshot shows (theme, colours, fonts, the input panel,
  the run table, the deck picker, the settings drawer, wording on screen), even if the user
  doesn't mention screenshots.
---

# The app's screenshots

`docs/screenshots/*.png` are taken from real runs, not drawn: a run really downloads the art,
and the app's own front end shows it in headless Chromium. One command takes them all:

```sh
pnpm --filter desktop screenshots              # every scene
pnpm --filter desktop screenshots decks        # only the scenes named
pnpm --filter desktop screenshots --record     # download the runs again first
```

They're the app window's default size, 1340 × 800, at 150%, as on a Windows laptop, so
2010 × 1200 files, and the settings say art is saved to `C:\MTG Art`.

## How it works

1. **Recording.** `crates/core/examples/record.rs` does a run through `mtg-art-core` (so through
   `http::Http`, like the app) and writes what the app would have been told:
   `target/screenshots/<list|deck>.json` (settings, the run's events, each face's sources, the
   deck search and box pictures), with the art in `target/screenshots/<list|deck>/art`. A
   recording is made the first time a scene needs it and kept; `--record` makes it again. It
   uses Scryfall's card data where the app keeps it, without updating it; with no copy, every
   card is asked of Scryfall, which is slower but works. The art is artists' work: it stays in
   `target/`, never in the repo.
2. **Serving.** `apps/desktop/screenshots/shoot.ts` serves the desktop front end with Vite, with
   `screenshots/api.ts` in place of `src/api.ts` (it answers from the recording) and
   `screenshots/tauri.ts` in place of the `@tauri-apps/*` modules (saved art is read off the disk
   through `/@fs/`). `screenshots/main.tsx` enters the recording's list or deck before the app
   starts.
3. **Shooting.** For each scene and colour mode it clicks **Download art**, waits for
   **Finished** and the visible thumbnails, does the scene's action (hover, right-click, open the
   drawer), parks the mouse out of the way and saves `docs/screenshots/<scene>-<mode>.png`.

## The scenes

| File                   | Shows                                                    | Used by                                       |
| ---------------------- | -------------------------------------------------------- | --------------------------------------------- |
| `results-{light,dark}` | The 10-card list in `screenshots/cards.txt`, downloaded  | README's top; the site's front page           |
| `decks-{light,dark}`   | Decks tab, `set:eoc`, World Shaper picked and downloaded | README, "Using it"                            |
| `find-copy-dark`       | Sol Ring's Google Lens button's tooltip                  | README, "Using it"                            |
| `sources-dark`         | Karn's row right-clicked: where its art is known to be   | README, "Using it"                            |
| `settings-dark`        | The settings drawer                                      | README, "Settings"                            |

`SCENES` in `shoot.ts` is the list; `RECORD_ARGS` is what each recording is a run of.

## Retaking them

1. Run `pnpm --filter desktop screenshots`. Add `--record` when the core reports something new
   or differently (a changed binding, new fields in an event), when MTGPics or Scryfall have
   changed what a card finds, or when a shot shows something the recording predates.
2. **Look at every image you took** with the Read tool before calling it done, against the one
   in git (`git show HEAD:docs/screenshots/<file> > /tmp/old.png`). Look for: art that didn't
   load, a hovered or focused control that shouldn't be, text cut off, a palette or font that
   isn't the theme's (a half-applied change, or another edit landing mid-run), and the change
   you made actually showing.
3. If a shot's content changed, check its alt text in `README.md` still describes it, and
   `apps/site/src/components/Screenshot.tsx` for the site's.
4. Commit the images with the change that made them differ, or on their own as
   `docs: retake the app's screenshots`.

When a scene fails, the error names the step: a selector that no longer matches means the UI's
labels changed, so update the scene's action in `shoot.ts`. `pageerror` lines are the app's own
errors, often a recording older than the bindings: retake with `--record`.

## Adding a scene

1. Add it to `SCENES` in `shoot.ts`: a `name`, the `recording` it uses, its colour `schemes`,
   and an `act` that gets the app into the state to show, with role- or label-based locators
   as in the Storybook stories. End with nothing hovered or focused that the picture isn't
   about.
2. If it needs data no recording has, record it in `record.rs`, add it to `Recording` in
   `screenshots/api.ts`, and answer from it there. `api.ts` is typed as the real `api`, so
   `pnpm typecheck` says when the app asks for something it doesn't answer.
3. Take it, look at it, and put it in the README with alt text that says what it shows.
