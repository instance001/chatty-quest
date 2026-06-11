# `v0.1` GitHub Release Plan

## Purpose

This is the short GitHub-side runbook for turning the current repository state into the first tagged `v0.1` release.

Use this when we are ready to move from:

- committed and pushed baseline

to:

- tagged and publicly framed `v0.1`

## Current Baseline

Current import commit:

- `0cf3c42` - `Initial v0.1 release-ready import`

Current branch:

- `main`

## Recommended Release Shape

Tag:

- `v0.1.0`

Release title:

- `Chatty Quest v0.1.0`

Suggested release description anchors:

- first accepted playable slice of Chatty Quest
- deterministic desktop adventure shell built on the RD Engine
- includes `Property Siege Classic`
- ships splash/menu shell, map, inventory, character, diagnostics, media focus, and save/load
- validated by automated tests plus completed manual sweep

## Suggested Tag Flow

From the local repo:

1. confirm `git status` is clean
2. confirm `cargo test` is green
3. create annotated tag:

```powershell
git tag -a v0.1.0 -m "Chatty Quest v0.1.0"
```

4. push the tag:

```powershell
git push origin v0.1.0
```

## Suggested GitHub Release Body

Short summary:

- First accepted playable `v0.1` release of Chatty Quest.

Highlights:

- deterministic run generation and reducer-owned game truth
- playable `Property Siege Classic` datapack
- chat-forward `MockNarrator` presentation seam
- map, inventory, character, diagnostics, and media panels
- JSON save/load
- branded splash and setup flow

Verification:

- `cargo test`
- manual sweep completed successfully on `2026-06-11`

Docs:

- [V0_1_RELEASE_NOTES.md](V0_1_RELEASE_NOTES.md)
- [V0_1_ACCEPTANCE_AUDIT.md](V0_1_ACCEPTANCE_AUDIT.md)
- [V0_1_MANUAL_SWEEP.md](V0_1_MANUAL_SWEEP.md)

## After Release

Once `v0.1.0` is tagged:

- treat `main` as the public `v0.1` baseline
- begin `v0.2` work from that known-good state
- keep future release notes additive rather than rewriting `v0.1` history
