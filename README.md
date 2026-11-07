# MTG Art Downloader

Download Magic: The Gathering card art in bulk. Paste a deck list, run a Scryfall search, or pick an
EDHREC top list, and the app saves each card's art — from [MTGPics](https://www.mtgpics.com) where
it has it, since its art is larger and uncropped, and from [Scryfall](https://scryfall.com)
otherwise.

A desktop app for Windows, macOS and Linux, and a command line tool, both built on one Rust core.
It's a rewrite of [MTG Art Downloader](https://github.com/MrTeferi/MTG-Art-Downloader) by Mr Teferi,
by way of [Alex Taxiera's fork](https://github.com/alex-taxiera/mtg-art-downloader).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/results-dark.png" />
  <img alt="The app after downloading a deck list: each card with a thumbnail of its art, where the art came from, and the artist" src="docs/screenshots/results-light.png" />
</picture>

## Install

Download the installer for your system from the
[latest release](https://github.com/AModSoul/mtg-art-downloader-rust/releases/latest). The app
checks for new versions when it starts and offers to update itself.

## Using it

**Card list.** One card per line, in the forms deck sites export:

```
Damnation
4 Lightning Bolt
4x Karn, Scion of Urza (DOM) 1
1 Sol Ring (C21) 263 *F*
```

Counts, foil and etched markers (`*F*`, `*E*`), and Archidekt categories and tags are ignored.
Section headings (`Commander`, `Sideboard`, `Creatures (12)`) and comments (`#`, `//`) are skipped.
A line with a set and collector number gets that exact printing; a line with only a name gets its
newest printing, or its oldest, or every printing's art — see the settings. A misspelt name is
matched to the closest card.

**Search.** Anything [Scryfall's search](https://scryfall.com/docs/syntax) takes —
`set:mh2 t:creature pow>=3` — and every matching card is downloaded. **Count matches** shows how
many first.

**Presets.** EDHREC's lists of the most-played Commander cards.

Files are named from a template, `NAME (ARTIST) [SET] {NUMBER}` by default, and by default go in
`mtgpics/` and `scryfall/` folders, with a folder for each layout: `MDFC Front` and `MDFC Back`,
`Adventure`, `Saga`, `Basic`. A double-faced card is saved as one image per face. When a run ends,
**Retry failed** runs the cards that failed again, and **Copy failed** copies them as a list.

### Settings

<img alt="The settings drawer" src="docs/screenshots/settings-dark.png" width="700" />

| Setting                   | Default                        |                                                    |
| ------------------------- | ------------------------------ | -------------------------------------------------- |
| Save art to               | `Pictures/MTG Art`             |                                                    |
| File names                | `NAME (ARTIST) [SET] {NUMBER}` |                                                    |
| Where art comes from      | MTGPics, then Scryfall         | or MTGPics only, or Scryfall only                  |
| Scryfall image            | Art crop                       | or the full card scan, as a PNG                    |
| Printing                  | Newest                         | for lines with only a name                         |
| Every printing's art      | Off                            | for lines with only a name                         |
| One printing per artwork  | On                             | skip reprints that reuse an art                    |
| Skip full-art printings   | Off                            |                                                    |
| Include extras            | On                             | Un-cards, oversized and championship cards         |
| Folders                   | Source and layout              | a folder per card type can be added                |
| Replace files of one name | On                             | otherwise the new file is numbered: `Name (1).jpg` |

The settings live in `settings.json` in the app's config folder
(`%APPDATA%\io.github.amodsoul.mtg-art-downloader` on Windows,
`~/Library/Application Support/io.github.amodsoul.mtg-art-downloader` on macOS,
`~/.config/io.github.amodsoul.mtg-art-downloader` on Linux), which the command line tool reads too.

### Command line

```sh
mtg-art cards.txt                       # a card list file
mtg-art --search "set:mh2 t:creature"   # a Scryfall search
mtg-art --preset edhrec-top-year        # a preset; --presets lists them
mtg-art cards.txt --out ./art           # somewhere other than the settings' folder
```

Build it with `cargo build --release -p mtg-art-cli`; the binary is `target/release/mtg-art`.

## How it finds art

Cards are looked up on Scryfall — lines with a set 75 at a time — within
[Scryfall's rate limits](https://scryfall.com/docs/api/rate-limits), which every request goes
through, however many cards are in flight.

MTGPics has no API, so its pages are read. A card is looked for at the page its set code and
collector number give (`dom001`), which must name the card and credit its artist. When it doesn't —
MTGPics numbers some sets its own way — the set's checklist is read, once per run, to find the
card's page by number, or by name and artist. Promos are found in MTGPics' promo lists by name and
artist. Whatever MTGPics lacks, including a double-faced card's back when MTGPics only has its
front, comes from Scryfall.

## Development

You need [Rust](https://rustup.rs), [Node 24](https://nodejs.org), and
[Tauri's system dependencies](https://v2.tauri.app/start/prerequisites/).

```sh
npm install
npm run tauri-dev      # the desktop app, reloading as you edit
cargo test --workspace # the core's tests; none reach the network
```

| Path           | What's in it                                                                       |
| -------------- | ---------------------------------------------------------------------------------- |
| `crates/core`  | Everything that finds and saves art: list parsing, Scryfall, MTGPics, the run loop |
| `crates/cli`   | The `mtg-art` command line tool                                                    |
| `src-tauri`    | The desktop app's Tauri commands, over the core                                    |
| `src`          | The desktop app's interface: React and Chakra UI, showing what the core reports    |
| `src/bindings` | TypeScript types generated from the core's Rust types                              |

After changing a type the interface uses, regenerate its TypeScript with `npm run bindings`. CI
fails if they're out of date.

### Releasing

1. Bump the version in `src-tauri/tauri.conf.json`, `Cargo.toml` and `package.json`.
2. Push a tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. The Release workflow builds the installers and attaches them to a draft release. Publish it, and
   installed apps offer the update.

Update bundles are signed. The workflow needs two repository secrets: `TAURI_SIGNING_PRIVATE_KEY`,
the private key whose public half is in `tauri.conf.json`, and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
(empty if the key has none). Losing the private key means installed apps can't update to a build
signed with a new one, so keep a copy somewhere safe.

The app icon is a placeholder. To replace it, run `npx tauri icon path/to/icon.png` with a square
PNG or SVG of at least 1024 pixels.

## License

[GPL-3.0](LICENSE), as the original is.
