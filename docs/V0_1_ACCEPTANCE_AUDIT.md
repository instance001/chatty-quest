# `v0.1` Acceptance Audit

Date audited: `2026-07-16`

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

- `cargo test` passes with `47` passing tests
- tests cover datapack discovery, multi-gate locked progression, barricade validation, two-room barricade pressure behavior, noise escalation, threat forecasting, utility-effect item behavior, objective-condition feedback, reducer behavior, narrator boundaries, media focus, diagnostics, UI-derived recommendation behavior, and save/load roundtrip

## Acceptance Grid

### Launch And Menu

Status: `pass`

Evidence:

- setup screen exists in [src/app.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/app.rs:138) and [src/ui/views.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/ui/views.rs:95)
- branded splash-to-setup launch flow exists before the playable shell
- menu exposes new game, load game, and datapack selection
- successful generate/load paths route into the active run shell in [src/app.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/app.rs:176)

Manual confirmation:

- local desktop launch, splash flow, and menu navigation passed in live testing

### Datapack Selection

Status: `pass`

Evidence:

- datapack discovery is external-file driven in [src/data/datapacks.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/data/datapacks.rs:190)
- automated tests verify `Property Siege Classic` discovery and bundle loading
- invalid datapacks are separated into catalog errors rather than treated as playable

### New Game Generation

Status: `pass`

Evidence:

- deterministic run generation lives in [src/game/generation.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/generation.rs:11)
- automated tests verify valid start location, starter inventory, objective freeze, and placement state

### Map And Location Display

Status: `pass`

Evidence:

- map panel, location display, and movement surfaces exist in [src/ui/views.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/ui/views.rs:320)
- map layout generation and tile state are wired through derived UI models
- reducer tests verify valid movement, invalid movement, lock-gated movement state changes, and torch-driven route reveal

Manual confirmation:

- live UI movement and map readability passed in manual testing

### Chat And Narration

Status: `pass`

Evidence:

- command input and chat-style log exist in the active run shell
- `MockNarrator` lives in [src/game/narrator.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/narrator.rs:20)
- tests verify narrator output reflects reducer-confirmed outcomes and surfaces win state without owning truth

### Inventory And Character Display

Status: `pass`

Evidence:

- inventory and character tabs are implemented in [src/ui/views.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/ui/views.rs:518) and [src/ui/views.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/ui/views.rs:647)
- interaction rows are now model-driven through derived builders
- reducer tests verify item pickup, equip, and use mutate structured state correctly
- character surfaces now also expose noise truth and barricaded-location truth directly

Manual confirmation:

- inventory and character UI updated correctly during live testing

### Movement And Boundaries

Status: `pass`

Evidence:

- reducer movement logic and scenario boundary block behavior live in [src/game/reducer.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/reducer.rs:58)
- tests verify connected movement succeeds, lock-gated movement blocks cleanly, and invalid movement returns the scenario boundary response

### Item Interaction

Status: `pass`

Evidence:

- take, equip, and use logic live in [src/game/reducer.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/reducer.rs:157)
- tests verify pickup removes world item state, equip updates equipped item state, medkit use heals and consumes the item, explicit unlock commands target the correct gate when multiple locks share one key item, torch use reveals deterministic connected-route knowledge, and barricade actions mutate structured state cleanly

### Barricade And Siege Pressure

Status: `pass`

Evidence:

- barricade state is modeled in [src/game/state.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/state.rs:6)
- barricade content and validation live in [src/data/datapacks.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/data/datapacks.rs:87)
- barricade behavior lives in [src/game/reducer.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/reducer.rs:434)
- tests verify `Front Verandah` and `Back Garden` both apply passive pressure before barricading and suppress it after barricading
- tests verify `Back Garden` now grants a recovery payoff when secured
- UI-derived state now also surfaces route-role hints, suggested next verbs, and threat forecasts for the authored siege lanes

### Noise And Escalation

Status: `pass`

Evidence:

- noise state is modeled in [src/game/state.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/state.rs:6)
- reducer-owned noise updates and authored escalation behavior live in [src/game/reducer.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/reducer.rs:875)
- tests verify loud actions raise noise, barricaded waiting lowers it, exposed pressure scales up at higher noise, and save/load preserves the value
- `Game`, `Character`, and diagnostics surfaces now expose readable noise truth

### Combat

Status: `pass`

Evidence:

- deterministic attack handling lives in [src/game/reducer.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/reducer.rs:261)
- tests verify boss combat damage, retaliation, alive/defeated state changes, objective progression, wounded-phase escalation, and the secured-property retaliation reduction when both siege lanes are barricaded

### Objective Progress

Status: `pass`

Evidence:

- objective state is frozen into `RunState`
- reducer completion logic lives in [src/game/reducer.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/game/reducer.rs:431)
- mixed-condition objective checks now require both item and boss truth when configured
- tests verify item-only completion, mixed boss-plus-item completion, objective progress-line surfacing, and `You win.` surfacing
- guided action surfaces can now point at likely next route verbs without owning objective truth

### Media Focus

Status: `pass`

Evidence:

- media panel state is built in [src/media/mod.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/media/mod.rs:60)
- tests verify default location focus plus event-driven item and boss focus shifts
- media hooks now also understand barricade-confirmation events without implying fake state
- focus is driven from reducer-confirmed events rather than free narration

### Save/Load JSON

Status: `pass`

Evidence:

- save/load runtime path lives in [src/runtime/mod.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/runtime/mod.rs:7)
- tests verify roundtrip preservation of location, HP, inventory length, equipped item, objective condition state, locked gate state, barricaded room state, and noise state
- app save/load shell wiring lives in [src/app.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/app.rs:430)

### Validation Errors

Status: `pass`

Evidence:

- datapack validation is implemented in [src/data/datapacks.rs](/C:/Users/User/Desktop/github_portal/chatty-quest/src/data/datapacks.rs:285)
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
- multiplayer transport is not implemented

## Current Verdict

If we are strict and honest:

- deterministic `v0.1` core: `accepted`
- desktop-shell release readiness: `accepted`

Recommended final pre-`v0.1` check:

- original `v0.1` sweep completed successfully on `2026-06-11`
- current branch sweep should use the refreshed runbook on `2026-07-16`
- runbook used: [docs/V0_1_MANUAL_SWEEP.md](docs/V0_1_MANUAL_SWEEP.md)
