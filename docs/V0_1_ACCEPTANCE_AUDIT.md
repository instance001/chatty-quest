# `v0.1` Acceptance Audit

Date audited: `2026-07-28`

This audit compares the current repository state against [`docs/V0_1_ACCEPTANCE_TESTS.md`](docs/V0_1_ACCEPTANCE_TESTS.md).

## Summary

Current status: `v0.1 accepted`

Current branch note:

- the branch now includes post-`v0.1` siege-depth work
- this audit entry has been refreshed to reflect that current branch state without changing the historical fact that `v0.1` itself was accepted on `2026-06-11`

Meaning:

- the deterministic core is now covered by automated tests
- the content pack is loading cleanly
- the UI shell and save/load paths are present in code
- the automated acceptance checks are green
- the live manual sweep has been completed successfully

Automated evidence:

- `cargo test` passes with `105` passing tests
- `cargo clippy --all-targets --all-features -- -D warnings` passes
- tests cover datapack discovery, command boundary parsing, engine/scenario boundary guarding, multi-gate locked progression, broken-open gate state, barricade validation, finale-security rule validation, location/item/enemy flavor hook validation, two-room barricade pressure behavior, noise escalation, noise attractor retargeting, max-noise enemy spawning, spawned-enemy movement blockers and hazard attacks, threat forecasting, utility-effect item behavior, utility/security truth surfacing, objective-condition feedback, objective-condition name surfacing, sidebar objective-progress surfacing, reducer behavior, post-win epilogue hooks, epilogue hook UI surfacing, bounded rolling-summary support memory, dry handoff snapshot packaging, narrator boundaries, narrator context packaging, media focus, diagnostics, UI-derived recommendation behavior, and save/load roundtrip

## Acceptance Grid

### Launch And Menu

Status: `pass`

Evidence:

- setup screen exists in [src/app.rs](../src/app.rs#L138) and [src/ui/views.rs](../src/ui/views.rs#L95)
- branded splash-to-setup launch flow exists before the playable shell
- menu exposes new game, load game, and datapack selection
- successful generate/load paths route into the active run shell in [src/app.rs](../src/app.rs#L176)

Manual confirmation:

- local desktop launch, splash flow, and menu navigation passed in live testing

### Datapack Selection

Status: `pass`

Evidence:

- datapack discovery is external-file driven in [src/data/datapacks.rs](../src/data/datapacks.rs#L190)
- automated tests verify `Property Siege Classic` discovery and bundle loading
- invalid datapacks are separated into catalog errors rather than treated as playable

### New Game Generation

Status: `pass`

Evidence:

- deterministic run generation lives in [src/game/generation.rs](../src/game/generation.rs#L11)
- automated tests verify valid start location, starter inventory, objective freeze, and placement state

### Map And Location Display

Status: `pass`

Evidence:

- map panel, location display, and movement surfaces exist in [src/ui/views.rs](../src/ui/views.rs#L320)
- map layout generation and tile state are wired through derived UI models
- reducer tests verify valid movement, invalid movement, lock-gated movement state changes, and torch-driven route reveal

Manual confirmation:

- live UI movement and map readability passed in manual testing

### Chat And Narration

Status: `pass`

Evidence:

- command input and chat-style log exist in the active run shell
- command parsing now produces a structured parsed-command boundary before reducer or narrator handling
- tests verify parser-boundary behavior for resolved commands and incomplete target verbs
- `MockNarrator` lives in [src/game/narrator.rs](../src/game/narrator.rs#L20)
- tests verify narrator output reflects reducer-confirmed outcomes and surfaces win state without owning truth

### Inventory And Character Display

Status: `pass`

Evidence:

- inventory and character tabs are implemented in [src/ui/views.rs](../src/ui/views.rs#L518) and [src/ui/views.rs](../src/ui/views.rs#L647)
- interaction rows are now model-driven through derived builders
- reducer tests verify item pickup, equip, and use mutate structured state correctly
- character surfaces now also expose noise truth and barricaded-location truth directly

Manual confirmation:

- inventory and character UI updated correctly during live testing

### Movement And Boundaries

Status: `pass`

Evidence:

- reducer movement logic and scenario boundary block behavior live in [src/game/reducer.rs](../src/game/reducer.rs#L58)
- tests verify connected movement succeeds, lock-gated movement blocks cleanly, and invalid movement returns the scenario boundary response

### Item Interaction

Status: `pass`

Evidence:

- take, equip, and use logic live in [src/game/reducer.rs](../src/game/reducer.rs#L157)
- tests verify pickup removes world item state, equip updates equipped item state, medkit use heals and consumes the item, explicit unlock commands target the correct gate when multiple locks share one key item, torch use reveals deterministic connected-route knowledge, and barricade actions mutate structured state cleanly

### Barricade And Siege Pressure

Status: `pass`

Evidence:

- barricade state is modeled in [src/game/state.rs](../src/game/state.rs#L6)
- barricade content and validation live in [src/data/datapacks.rs](../src/data/datapacks.rs#L87)
- barricade behavior lives in [src/game/reducer.rs](../src/game/reducer.rs#L434)
- tests verify `Front Verandah` and `Back Garden` both apply passive pressure before barricading and suppress it after barricading
- tests verify `Back Garden` now grants a recovery payoff when secured
- UI-derived state now also surfaces route-role hints, suggested next verbs, objective progress, utility relevance, siege security summaries, and threat forecasts for the authored siege lanes

### Noise And Escalation

Status: `pass`

Evidence:

- noise state is modeled in [src/game/state.rs](../src/game/state.rs#L6)
- reducer-owned noise updates and authored escalation behavior live in [src/game/reducer.rs](../src/game/reducer.rs#L875)
- tests verify loud actions raise noise, successful non-noisy actions lower it over time, rejected actions do not lower it, exposed pressure scales up at higher noise, max-noise spawns a template-backed enemy instance into an outdoor yard square, and save/load preserves the value
- `Game`, `Character`, and diagnostics surfaces now expose readable noise truth

### Combat

Status: `pass`

Evidence:

- deterministic attack handling lives in [src/game/reducer.rs](../src/game/reducer.rs#L261)
- tests verify boss combat damage, retaliation, alive/defeated state changes, objective progression, template-authored wounded-phase escalation, datapack-authored enemy flavor hooks, and the datapack-authored secured-property retaliation reduction

### Objective Progress

Status: `pass`

Evidence:

- objective state is frozen into `RunState`
- reducer completion logic lives in [src/game/reducer.rs](../src/game/reducer.rs#L431)
- mixed-condition objective checks now require both item and boss truth when configured
- tests verify item-only completion, mixed boss-plus-item completion, objective progress-line surfacing, and `You win.` surfacing
- guided action surfaces can now point at likely next route verbs without owning objective truth

### Media Focus

Status: `pass`

Evidence:

- media panel state is built in [src/media/mod.rs](../src/media/mod.rs#L60)
- tests verify default location focus plus event-driven item and boss focus shifts
- media hooks now also understand barricade-confirmation events without implying fake state
- focus is driven from reducer-confirmed events rather than free narration

### Save/Load JSON

Status: `pass`

Evidence:

- save/load runtime path lives in [src/runtime/mod.rs](../src/runtime/mod.rs#L7)
- tests verify roundtrip preservation of location, HP, inventory length, equipped item, objective condition state, locked gate state, barricaded room state, turn index, and noise state
- tests verify save payload parsing rejects unsupported future versions while preserving missing-version compatibility as `v1`
- app save/load shell wiring lives in [src/app.rs](../src/app.rs#L430)

### Validation Errors

Status: `pass`

Evidence:

- datapack validation is implemented in [src/data/datapacks.rs](../src/data/datapacks.rs#L285)
- diagnostics surface invalid datapacks and missing media
- tests verify diagnostics warnings surface missing referenced media assets

### Reserved Future Folders

Status: `pass`

Evidence:

- `runtime/`, `models/`, `datasets/`, and `handoff/` folders exist locally
- each lane has visible reserved structure and documentation

### Narrator Boundary Test

Status: `pass`

Evidence:

- narrator only transforms reducer outcomes into presentation text
- tests verify narrator surfaces existing events and does not become a hidden source of canonical state

### Ecosystem Boundary Test

Status: `pass`

Evidence:

- no runtime dependency on Chatty-Cog, Chatty-Art, or Chatty-Lora execution paths was found in the gameplay loop
- dry handoff snapshot packaging exists only as in-process shape validation
- multiplayer transport is not implemented

## Current Verdict

If we are strict and honest:

- deterministic `v0.1` core: `accepted`
- desktop-shell release readiness: `accepted`

Recommended final pre-`v0.1` check:

- original `v0.1` sweep completed successfully on `2026-06-11`
- current branch sweep completed successfully on `2026-07-28`
- runbook used: [docs/V0_1_MANUAL_SWEEP.md](docs/V0_1_MANUAL_SWEEP.md)
- supporting screenshots for release documentation live under `assets/ui/screenshots/`
