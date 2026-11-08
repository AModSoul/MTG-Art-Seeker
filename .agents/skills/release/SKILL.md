---
name: release
description: Cut a release of the MTG Art Seeker desktop app - the version bump PR, the release notes, the v* tag, the draft GitHub release the Release workflow builds, checking its installers and update manifest, and publishing it. Use whenever the user wants to release, ship, publish, tag or cut a new version of the app, bump the version, write release notes, or finish or fix a release that is still a draft or whose build failed, even if they do not say "release".
---

# Releasing the desktop app

A release is a GitHub release named **MTG Art Seeker v<version>**, holding the Windows, macOS
and Linux installers, their `.sig` files and `latest.json`, the signed manifest the in-app updater
reads. Pushing a `v*` tag makes `.github/workflows/release.yml` build all three platforms and attach
them to a draft. Publishing the draft is what ships it.

Hold on to these throughout:

- **Publishing is the release.** The updater reads
  `releases/latest/download/latest.json`, so the moment the draft is published every installed copy
  offers the update, and `site.yml` redeploys getmtg.art's download page. Pushing the tag is public
  too. Show the user the version and notes and get a yes before pushing the tag, and ask again
  before publishing. Everything before the tag push is local or a PR and can be redone freely.
- **The tag must match `tauri.conf.json`.** The workflow names the release from
  `apps/desktop/src-tauri/tauri.conf.json`'s version (`tagName: v__VERSION__`), not from the tag. A
  tag pushed on a commit that still has the old version builds into the old version's release,
  which is already published. Check before tagging, every time.
- **`releases/latest` is always the app's.** Never create a release by hand
  (`gh release create`); the workflow makes the draft. Nothing but the app ever gets a `v*` tag.
- **No pre-releases.** The workflow publishes with `prerelease: false`, and Windows' MSI bundler
  rejects versions like `0.3.0-rc1`. If the user wants one, say so; it needs workflow changes first.

The remote is `origin` and the default branch is `trunk`. Commit messages and PR titles follow
Conventional Commits.

`gh`'s own subcommands differ a lot between versions (2.4 has no `gh release edit`, and no
`--event` or `--branch` on `gh run list`), so the steps below use `gh api` and `--json`/`--jq`,
which work with any version. `{owner}/{repo}` is filled in by `gh api` itself. A draft has no tag
yet as far as the API's `releases/tags/` is concerned, so find a release by listing:

```bash
gh api 'repos/{owner}/{repo}/releases?per_page=20' --jq '.[] | select(.tag_name=="v<version>") | {id, name, draft}'
```

and use its `id` as `<release-id>` below.

## 1. Is there something to release?

```bash
git fetch origin --tags
git describe --tags --abbrev=0 origin/trunk          # the last release
git log --oneline <last-tag>..origin/trunk
gh run list --workflow ci.yml --limit 20 --json headBranch,status,conclusion --jq '[.[] | select(.headBranch=="trunk")][0]'   # must be completed, success
```

- If nothing since the last tag touches the app (`crates/`, `apps/desktop/`, `packages/ui/`), say
  so: site, docs, CI and agent changes don't need a release. Ask whether one is really wanted.
- If CI on trunk's head is red or still running, stop until it's green.
- Check the release list (`gh api 'repos/{owner}/{repo}/releases?per_page=5' --jq '.[] | {tag_name, draft}'`)
  for a leftover draft. If one exists for the version you're
  about to cut, this is a resumed release: skip to the step it got to.

Work from `origin/trunk`, not from whatever is checked out. If the working tree has uncommitted
changes, don't stash or touch them: do the release in a worktree
(`git worktree add ../mtg-art-release origin/trunk`) and work there.

## 2. Version

Pick it with the user if they didn't give one: `MAJOR.MINOR.PATCH`, a minor bump for new features,
a patch bump for fixes only. The tag is `v<version>`; check it doesn't exist yet
(`git ls-remote --tags origin v<version>`).

## 3. Release notes

There is no changelog file. The notes go in the release PR's body and then on the release. Write
them for someone who uses the app, not for a developer: what they can now do, what was fixed. No
commit subjects, crate names, refactors, CI, site, Storybook or agent changes. Work out what
changed from `git log <last-tag>..origin/trunk`, the merged PRs' bodies
(`gh pr view <n> --json body`) and the README.

Follow the last release's shape
(`gh api 'repos/{owner}/{repo}/releases?per_page=20' --jq '.[] | select(.tag_name=="<last-tag>") | .body'`):

```markdown
If you have <previous version> installed, the app offers this update when it starts. Otherwise, download the installer for your system below, or from [getmtg.art](https://getmtg.art/download).

## New

### <A headline feature>

<What it is, in a sentence or two.>

- <How to use it, naming controls in **bold** as the app labels them.>

### Also new

- <Smaller additions.>

## Fixed

- <What was wrong, as the user saw it, and that it no longer is.>
```

Drop a section that would be empty. Write the notes to a file in the scratchpad, and show them to
the user.

## 4. The version bump PR

On a branch `chore/release-<version>` from `origin/trunk`, set the version in:

- `apps/desktop/src-tauri/tauri.conf.json` — `"version"`, which names the release;
- `Cargo.toml` — `[workspace.package] version`, which all three crates use;
- `apps/desktop/package.json` — `"version"`.

Then `cargo update --workspace` to move the three crates' entries in `Cargo.lock` without building
anything. `git diff` should show only those version lines and, in `Cargo.lock`, only
`mtg-art-cli`, `mtg-art-core` and `mtg-art-downloader`. `git grep -n '<old version>'` finds what
was left: the site's fixtures of real past releases and sample text in stories stay as they are.

Commit as `chore: release <version>`. Once the user agrees to the version and notes, push the
branch and open the PR, its body shaped like the last one's
(`gh pr list --state merged --search "chore: release" --json number,body --jq '.[0].body'`):

- **Summary** — the three files bumped, and that `Cargo.lock` changes only the workspace's crates.
- **After merging** — the commands in step 5.
- The release notes inside `<details><summary>Draft release notes</summary>`.
- **Test plan**.

Wait for CI on the PR:

```bash
gh run watch $(gh run list --workflow ci.yml --limit 20 --json databaseId,headBranch --jq '[.[] | select(.headBranch=="chore/release-<version>")][0].databaseId') --exit-status
```

 Merging is the user's call: ask whether they
want you to merge it or will do it themselves.

## 5. Tag

After the PR is merged:

```bash
git switch trunk && git pull --ff-only origin trunk
jq -r .version apps/desktop/src-tauri/tauri.conf.json   # must print <version>
git log -1 --format=%s                                   # the release PR's merge
```

If the version doesn't match, stop: see the second rule above. Ask the user, then:

```bash
git tag v<version>
git push origin v<version>
```

Watch the build. It takes a while (macOS builds a universal binary), so watch it in the background:

```bash
gh run watch $(gh run list --workflow release.yml --limit 20 --json databaseId,headBranch,event --jq '[.[] | select(.headBranch=="v<version>" and .event=="push")][0].databaseId') --exit-status
```

The three platforms build independently (`fail-fast: false`). If one fails:

- A flaky step: `gh api -X POST 'repos/{owner}/{repo}/actions/runs/<run-id>/rerun-failed-jobs'`.
- Signing errors mean the `TAURI_SIGNING_PRIVATE_KEY` secrets are missing or wrong. Only the repo's
  owner can fix that; tell the user.
- A real build failure: fix it on trunk through a PR, then build just that platform into the same
  draft with `gh workflow run release.yml --ref trunk -f platform=<windows|macos|linux>`. This
  works because the draft is found by `tauri.conf.json`'s version. Don't move the tag unless the
  fix changes what the other platforms built; if it does, delete the draft and the tag and start
  step 5 again.

## 6. Check the draft

```bash
gh api 'repos/{owner}/{repo}/releases?per_page=20' --jq '.[] | select(.tag_name=="v<version>" or .tag_name=="<last-tag>") | {tag_name, name, draft, assets: [.assets[].name]}'
```

It should be a draft named `MTG Art Seeker v<version>`, with the same kinds of files as the last
release under the new version: `latest.json`; Windows `_x64-setup.exe` and `_x64_en-US.msi`; macOS
`_universal.dmg` and `_universal.app.tar.gz`; Linux `.AppImage`, `.deb` and `.rpm`; and a `.sig`
beside every one but the `.dmg`. Then the manifest:

```bash
gh api 'repos/{owner}/{repo}/releases/<release-id>' --jq '.assets[] | select(.name=="latest.json") | .id'
gh api 'repos/{owner}/{repo}/releases/assets/<asset-id>' -H 'Accept: application/octet-stream' > <scratchpad dir>/latest.json
jq '{version, platforms: (.platforms | keys)}' <scratchpad dir>/latest.json
```

`version` must be `<version>`, and `platforms` must cover `windows-x86_64`, `darwin-aarch64`,
`darwin-x86_64` and `linux-x86_64`. A platform missing from it gets no update.

If a file's name has changed from the last release, the download page lists it under "Other files"
until `apps/site/src/downloads/artifacts.ts` gets a rule for it. Tell the user; it doesn't block the
release.

Offer the user the chance to install one of the draft's installers before publishing: download it
the same way, by its asset id, into a directory of its own.

## 7. Notes and publish

```bash
gh api -X PATCH 'repos/{owner}/{repo}/releases/<release-id>' -F body=@<notes file> --jq .html_url
```

Show the user that URL (a draft's page is visible only to the repo's maintainers). Ask the user
before publishing, then:

```bash
gh api -X PATCH 'repos/{owner}/{repo}/releases/<release-id>' -F draft=false -f make_latest=true --jq '{draft, html_url}'
```

Then check it went out:

```bash
gh api repos/{owner}/{repo}/releases/latest --jq .tag_name     # v<version>
curl -sL https://github.com/AModSoul/mtg-art-downloader-rust/releases/latest/download/latest.json | jq -r .version
gh run watch $(gh run list --workflow site.yml --limit 20 --json databaseId,headBranch,event --jq '[.[] | select(.headBranch=="v<version>" and .event=="release")][0].databaseId') --exit-status
```

Report the release's URL and whether the site redeployed.

If a bad build went out, setting it back to a draft
(`gh api -X PATCH 'repos/{owner}/{repo}/releases/<release-id>' -F draft=true`) takes it back: `releases/latest`
falls back to the previous release and no more copies update to it. Copies that already updated
stay, so the fix ships as the next patch release, not by replacing files on this one.
