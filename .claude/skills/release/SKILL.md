---
name: release
description: >-
  Cut a versioned release of RailGraph (the Tauri desktop app): bump the version,
  generate a user-facing changelog, tag to trigger the multi-platform signed
  build, verify the draft release + updater artifacts, and publish. Claude drives
  the read-only checks and all generation; the releaser runs git, the tag, and the
  publish. Use when preparing or cutting a release, e.g. "prepare the v2.1.0
  release", "let's cut a release", "release RailGraph".
---

# Releasing RailGraph

A phased loop for shipping a RailGraph release. RailGraph is a Tauri desktop app
distributed as GitHub Releases with auto-update via `tauri-plugin-updater`. This
skill is the *how*.

## Who runs what

The releaser owns anything that touches git, the tag, or the published release;
Claude owns the read-only checks and all generation. The goal is for Claude to
drive as much as possible while the irreversible steps keep an explicit human OK.

| What | Who |
| --- | --- |
| Read-only checks (CI status, run monitoring, asset/updater verification) | **Claude** (`gh`) |
| Propose the version bump (SemVer from commits since the last tag) | **Claude** |
| Edit `tauri.conf.json` + crate versions for the bump | **Claude** (you review the diff) |
| Generate the user-facing changelog (release notes + `CHANGELOG.md`) | **Claude** |
| Draft the release PR body and the release notes | **Claude** |
| `git`: commit, push, open/merge the release PR | **You** — gate |
| `git tag` + `git push <tag>` (triggers the whole release build) | **You** — gate |
| Publish the draft release (makes it *latest* → ships auto-update) | **You** — gate |

## Key facts about how RailGraph releases work

Read these once — they are the sharp edges.

- **Desktop releases are cut from the `2.0` branch, not `main`.** `main` is
  still the legacy web app with its own AWS deploy pipeline; the desktop app,
  its CI, and this release pipeline live on `2.0`. Everything below that says
  "the release branch" means `2.0` (update this skill when 2.0 becomes main).
- **Version source of truth is `src-tauri/tauri.conf.json` → `version`.** The
  three crate versions (root `Cargo.toml`, `core/Cargo.toml`,
  `src-tauri/Cargo.toml`) are kept in sync with it; `src-tauri`'s
  `CARGO_PKG_VERSION` drives the Sentry release name (`railgraph@<version>`).
- **A tagged release is created as a DRAFT and is NOT auto-published.** A green
  build is *not* a shipped release. Someone writes the changelog and clicks
  publish.
- **Publishing as *latest* is what ships the update.** The updater endpoint is
  `releases/latest/download/latest.json`. Publishing the draft as a full
  (non-prerelease) release is what repoints it. A prerelease does not become
  "latest".
- **The tag triggers `release.yml`**, which creates the draft, then builds +
  signs four targets via `tauri-action`: macOS arm64/x86 (Apple cert +
  notarization + updater signing), Linux x86, Windows x64 (unsigned installer;
  SmartScreen will warn). Sentry symbol-upload steps are token-gated and
  best-effort — they never fail the build.
- **Tags are annotated, not GPG-signed** — no signing-key ceremony.
- **Gate the actual commit being released.** CI (`ci.yml`, `test.yml`) runs on
  pushes/PRs to `2.0`; confirm the released SHA itself has a green run.

Repo: `SuperManifolds/rail_graph`. Expected assets (~16): `latest.json`; macOS
`RailGraph_aarch64.dmg` / `RailGraph_x64.dmg` plus updater bundles
`RailGraph_{aarch64,x64}.app.tar.gz(.sig)`; Windows `RailGraph_x64_setup.exe(.sig)`
and `RailGraph_x64.msi(.sig)`; Linux `RailGraph_amd64.AppImage(.sig)`,
`RailGraph_amd64.deb(.sig)`, `RailGraph_x86_64.rpm(.sig)`. Note the **`.dmg`s
carry no `.sig`** — the macOS updater signs the `.app.tar.gz`, not the disk
image. (Confirm exact names against the previous release rather than trusting
this list verbatim — the `.sig` set follows `tauri-action`'s
`assetNamePattern`.)

## The loop

Work one phase at a time: Claude proposes/generates, you run the state-changing
commands, you both confirm, then move on. Keep the releaser's load light —
**surface the minimum to make the current decision, one decision at a time, and
don't narrate read-only checks that passed.** Give the full clickable URL for any
PR / release / run you name.

**Always show where we are.** At the start of every phase print one line:

`Phase N/8 · <name> · ~<total> remaining`

| Phase | Rough time |
| --- | --- |
| 0 · Preflight & version | ~10 min |
| 1 · Version bump PR | ~10 min + CI |
| 2 · Changelog | ~15 min |
| 3 · Tag & trigger | ~5 min |
| 4 · Monitor the build | ~15 min |
| 5 · Verify the draft | ~10 min |
| 6 · Publish | ~5 min |
| 7 · Post-publish verify | ~5 min |
| 8 · Improve the skill | ~10 min |

(Refine the Phase 4 time anchor after the first real release — no full matrix
has run yet.)

### Phase 0: Preflight & version

- **Claude** confirms the ground is solid, reporting only what needs a decision:
  - **The local git state is clean before touching anything.** Assess up front,
    because a messy tree derails the version-bump PR. Verify and reset to a
    known-good base:

    ```sh
    git fetch origin 2.0 --tags
    git status --porcelain          # want empty
    git rev-parse HEAD origin/2.0   # local 2.0 should match origin
    git branch --list 'release*'    # delete any stale release branches first
    ```

    If dirty: stash/discard, `git checkout 2.0 && git reset --hard origin/2.0`,
    and delete stale `release*` branches — *then* start.
  - **Apple Developer agreements are current.** A lapsed agreement fails the
    macOS build at *notarization* with `HTTP 403: A required agreement is
    missing or has expired` — after the build, not at tag time. It needs no code
    change, only accepting the agreement at <https://developer.apple.com/account>
    (and App Store Connect → Agreements). Flag it to the releaser to confirm
    before tagging.
  - `2.0` is current locally and nothing release-blocking is unmerged. If a PR
    *should* ship, flag it — it must be merged before tagging.
  - **CI is green on the real shipping commit:**

    ```sh
    gh run list --repo SuperManifolds/rail_graph --workflow ci.yml --branch 2.0 --limit 5 \
      --json headSha,status,conclusion,displayTitle \
      --jq '.[] | "\(.headSha[0:8]) \(.status)/\(.conclusion) \(.displayTitle)"'
    git rev-parse origin/2.0   # HEAD must match a green run above
    ```

- **Claude proposes the version, you confirm.** Per [SemVer](https://semver.org/):
  **minor** if anything since the last tag is a user-facing feature, **patch**
  if it's only fixes. Derive it from the commit prefixes:

  ```sh
  git describe --tags --abbrev=0                          # last release tag
  git log --no-merges <last-tag>..origin/2.0 --format='%s' | cat
  ```

  Any `feat`/breaking change → minor. Refactor/chore/ci/perf-only with fixes →
  patch. Propose `$VERSION` with the one or two changes that drove the call.

  **Present the version as a structured choice (`AskUserQuestion`), not a prose
  question** — one option per candidate bump, each labelled with what drove it,
  so the releaser picks rather than free-types. Same for any other either/or
  gate in this loop.

### Phase 1: Version bump PR

The bump ships as its own small PR so CI runs on it before the tag.

- **Claude** bumps the version everywhere it lives, to one value:
  - `src-tauri/tauri.conf.json` → `"version"` (the app version + updater version).
  - Root `Cargo.toml`, `core/Cargo.toml`, `src-tauri/Cargo.toml`
    `[package] version`.
  - Run `cargo check` so `Cargo.lock` picks up the new versions.
- **You** open the PR on a branch (`release/v2.1.0`), base `2.0`, title
  `chore: prepare v2.1.0 release`. Let CI go green, then merge.
- Note the merge commit — the tag goes on it.

### Phase 2: Changelog

The changelog is **hand-written and user-facing** — no generator. Users care
about features, fixes, and "it's faster now", not refactors or internals.

- **Claude** builds it from the commits since the last release:

  ```sh
  git log --no-merges <last-tag>..origin/2.0 --format='%h %s' | cat
  ```

- **Translate, don't transcribe.** Group into **New Features / Improvements /
  Bug Fixes / Performance**. Each line is what a user would notice. Rules:
  - **Exclude** internal-only work: `refactor`, `chore`, `ci`, dependency bumps,
    test-only changes, and anything a user can't see. If a refactor made things
    faster, it becomes a Performance line, not a refactor line.
  - **Don't re-list** what a previous release already shipped — diff against the
    last release's notes (`gh release view <last-tag> --json body`).
  - Plain language, no jargon (no "msgpack", "signal", internal identifiers).
  - Match RailGraph's product vocabulary (lines, stations, views, schedules,
    the timetable graph, infrastructure) from the app and README.
- **Format** — mirror the previous release's notes so the style stays
  consistent:
  - One `## <Section>` per group; a feature-heavy release reads best with
    **themed** sections a user recognizes. Group by what the reader cares
    about, not by commit type.
  - Each entry is a bullet: a short **bold lead-in**, an em dash, then one plain
    sentence — what changed and why it matters. No `(#PR)` refs, no commit
    hashes, no author handles.
  - Scannable: a handful of bullets per section, most impactful first.
- **Also prepend the same content to `CHANGELOG.md`** (this repo keeps one; the
  entry ships with the bump PR or a follow-up commit on `2.0`).
- **You** approve the wording. This becomes the release body in Phase 6; hold it.

### Phase 3: Tag & trigger

> **Gate:** the bump PR is merged, CI is green on the merge commit, and you know
> its SHA.

- **You** tag the merge commit by its explicit SHA (never `HEAD`/`origin/2.0` —
  the branch may move) and push:

  ```sh
  git fetch origin
  git tag -a v2.1.0 <merge-sha> -m "RailGraph v2.1.0"
  git push origin v2.1.0        # triggers release.yml
  ```

### Phase 4: Monitor the build

- **Claude** watches the tag's `release.yml` run to a terminal state — prefer
  the `watch-ci` skill (fast-fail per job) over `gh run watch` (blocks until the
  whole run ends and can exit 0 on a transient `HTTP 502`). After any watcher
  returns, re-confirm the real terminal state before acting:

  ```sh
  gh run list --repo SuperManifolds/rail_graph --workflow release.yml --limit 5 \
    --json databaseId,headBranch,status,conclusion \
    --jq '.[] | select(.headBranch=="v2.1.0") | "\(.databaseId) \(.status)/\(.conclusion)"'
  gh run view <run-id> --repo SuperManifolds/rail_graph --json status,conclusion \
    --jq '"\(.status)/\(.conclusion)"'   # want completed/success
  ```

  Flag any failed job, especially a signing/notarization failure. If a platform
  fails to build, the tag can be re-pushed after a fix (see *Fixing a broken
  release*). A partial matrix means missing assets — don't publish.

### Phase 5: Verify the draft

The build made a **draft** release. Confirm it's whole before publishing.

- **Claude** confirms every expected asset is present (all four platforms +
  their installers + `.sig` files + `latest.json`):

  ```sh
  gh release view v2.1.0 --repo SuperManifolds/rail_graph --json isDraft,assets \
    --jq '{draft: .isDraft, assets: [.assets[].name]}'
  ```

- **Claude** confirms `latest.json` names the new version and all platforms:

  ```sh
  gh release download v2.1.0 --repo SuperManifolds/rail_graph --pattern latest.json -O - \
    | jq '{version, platforms: (.platforms|keys)}'
  ```

  `version` must equal the release version and each platform must have a
  non-empty `signature`. Anything missing → do not publish; investigate.

### Phase 6: Publish

> **Gate:** Phase 5 clean — all assets present, `latest.json` correct.

- **You** replace the draft's auto-generated notes with the Phase 2 changelog and
  publish it as a full, latest (non-prerelease) release. Publishing as *latest*
  is what repoints the updater endpoint.

  ```sh
  gh release edit v2.1.0 --repo SuperManifolds/rail_graph \
    --notes-file <changelog.md> --draft=false --latest --prerelease=false
  ```

  (Editing the body in the GitHub UI and clicking "Publish release" is
  equivalent — just ensure "Set as the latest release" is ticked and
  "pre-release" is not.)

### Phase 7: Post-publish verify

- **Claude** confirms the release is live and *latest*, and that the updater
  will serve it:

  ```sh
  gh release view --repo SuperManifolds/rail_graph --json tagName,isDraft,isPrerelease \
    --jq '"latest=\(.tagName) draft=\(.isDraft) prerelease=\(.isPrerelease)"'
  curl -sL https://github.com/SuperManifolds/rail_graph/releases/latest/download/latest.json | jq .version
  ```

  `latest` must be the new tag, `draft=false`, `prerelease=false`, and the
  updater `latest.json` `version` must equal the release.
- An in-app auto-update from the previous version is the real end-to-end proof
  if a machine on the old version is handy.

### Phase 8: Improve the skill

Leave the skill better than you found it. For each rough spot, broken command,
or "what now?" during the release, name the concrete edit that would have
prevented it, and refine the time anchors with what this run actually took.
Propose the edits; the releaser approves; land them in a separate post-release
PR.

## Fixing a broken release

First decide whether the failure was **the code/config** or **something external
and transient** — the recovery differs.

**Transient / external failure (no code change needed)** — e.g. notarization
`403` from a lapsed Apple agreement, a flaky network step. Fix the external
cause, then **re-run only the failed jobs on the same run — do NOT re-tag:**

```sh
gh run rerun <run-id> --failed --repo SuperManifolds/rail_graph
```

This re-runs just the failed matrix jobs, reusing the successful builds and the
same draft release. Re-verify (Phase 5), publish (Phase 6).

**Code/config failure (needs a source change)** — re-tag after landing the fix:

1. Land the fix on `2.0`; confirm CI green on the fix commit.
2. Re-tag the fix commit by explicit SHA and force-push to re-run the build:

   ```sh
   git tag -f -a v2.1.0 <fix-sha> -m "RailGraph v2.1.0"
   git push -f origin v2.1.0
   ```

   The re-run recreates the draft and rebuilds/re-signs. Re-verify (Phase 5)
   and publish (Phase 6).

Prefer either recovery only before the release is published. Once users may have
auto-updated, ship a new patch version instead of moving a published tag.
