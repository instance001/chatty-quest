# `v0.1` Release Notes

Release date: `2026-06-11`

Release status: `accepted`

## What `v0.1` Ships

`v0.1` is the first honest playable slice of Chatty Quest.

It ships:

- a Rust desktop shell built with `egui/eframe`
- a branded launch flow with splash, engine beat, and setup screen
- external datapack discovery and validation
- deterministic new-game generation for `Property Siege Classic`
- reducer-owned movement, inspect, take, equip, use, attack, and wait flows
- chat-forward narration through a replaceable `MockNarrator`
- map, inventory, character, media, and diagnostics tabs
- JSON save/load under `runtime/saves/`

## Included Scenario

Playable datapack:

- `Property Siege Classic`

This pack is the `v0.1` proof slice for:

- bounded deterministic exploration
- inventory and equipment state
- combat and boss resolution
- objective completion
- event-aware media focus with fallbacks

## Validation Status

Automated validation:

- `cargo test` passes with `14` tests
- coverage includes datapack loading, run generation, reducer behavior, narrator boundaries, media focus, diagnostics, and save/load roundtrip

Manual validation:

- live desktop sweep completed successfully on `2026-06-11`
- scenario was confirmed playable, completable, saveable, and reloadable

Supporting records:

- [V0_1_ACCEPTANCE_AUDIT.md](V0_1_ACCEPTANCE_AUDIT.md)
- [V0_1_MANUAL_SWEEP.md](V0_1_MANUAL_SWEEP.md)
- [V0_1_ACCEPTANCE_TESTS.md](V0_1_ACCEPTANCE_TESTS.md)

Visual references:

- [README.md](../README.md)
- [`assets/ui/screenshots/`](../assets/ui/screenshots/)

Media credit:

- in-game media for this release was created with help from [instance001/chatty-art](https://github.com/instance001/chatty-art)

## Deliberate Non-Goals

`v0.1` does not attempt to ship:

- real LLM runtime integration
- multiplayer
- live Chatty ecosystem handoff
- advanced procedural generation
- broad natural-language understanding
- generalized RPG-scale progression systems

## What Comes Next

`v0.2` should deepen deterministic scenario expression without breaking truth ownership.

The current post-`v0.1` planning anchor is:

- [V0_2_MILESTONE_PLAN.md](V0_2_MILESTONE_PLAN.md)
