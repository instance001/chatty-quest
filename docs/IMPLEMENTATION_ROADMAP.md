# Implementation Roadmap

## Purpose

This roadmap defines the intended build order for `v0.1`.

It exists to keep the project moving toward a real playable slice without:

- collapsing into scope creep
- overfitting to `Property Siege Classic`
- prematurely implementing future Chatty ecosystem behavior
- letting narrator concerns overtake deterministic game foundations

The order below is intentionally practical. It prioritizes architecture proof over feature volume.

This roadmap should also preserve the future RD Engine shape without forcing all of it into `v0.1`.

In particular:

- future systems may deepen the template-instance-bucket split
- future NPC systems may combine bucketed truth with summary-supported continuity
- future media may bind real assets to event and context hooks
- future runtime internals may migrate family-by-family toward explicit instances as documented in `docs/INSTANCE_MIGRATION_PLAN.md`

Those are roadmap directions, not immediate `v0.1` requirements.

## Phase 0: Docs And Scaffold

Goals:

- establish the design intent and architecture docs
- create the project folder scaffold
- reserve future folders for runtime, models, datasets, and handoff lanes
- keep read-only reference material clearly quarantined

Deliverables:

- core docs under `docs/`
- visible folder layout for `assets/`, `runtime/`, `models/`, `datasets/`, `handoff/`, and `src/`
- README placeholders for reserved future folders

Why first:

The project has a strong long-term shape and a deliberately small first release. If this phase is skipped, implementation will drift toward either overbuilding or flattening the design.

## Phase 1: Rust GUI Shell

Goals:

- establish the desktop app skeleton in Rust
- use `egui/eframe` unless a better local reason emerges during setup
- prove the main menu and basic screen flow

Deliverables:

- app launches
- main menu screen
- placeholder screens or tabs for:
  - game
  - inventory
  - character
  - load/save entry points

Why here:

The shell gives the project a concrete playable surface early without forcing game logic and UI to be designed blindly in parallel.

## Phase 2: Datapack Loader And Validator

Goals:

- load the selected datapack from `assets/datapacks/`
- parse pack metadata, rules, and templates
- reject broken packs with useful validation errors

Deliverables:

- datapack discovery
- `pack.toml` parsing
- `rules.toml` parsing
- template parsing for `v0.1` content types
- support for shared template fields such as `id`, `name`, `description`, `tags`, and optional `narrator_brief`
- validation errors that are readable and actionable

Why here:

The scenario engine identity depends on externalized content. Before run generation or gameplay exists, the project should already prove that content lives outside the core code.

## Phase 3: RunState And Generator

Goals:

- define canonical `RunState`
- assemble the starting state for a new game
- freeze a starting objective and scenario-specific world state

Deliverables:

- runtime structs for player, map, entities, inventory, objective, and support state
- new-game flow that creates a deterministic starting run
- starting location selection
- initial item, enemy, and boss placement logic

Recommended `v0.1` reading:

Use a small authored or semi-authored property map with deterministic connections. Allow light seeded variation only where it helps prove the shape cleanly.

Future-shape note:

This phase should avoid collapsing the long-term distinction between template, spawned instance, and current bucket state, even if `v0.1` implements a lighter version.

Why here:

Without `RunState`, the project has no truth model. This is the moment the game becomes more than a UI wrapper over ideas.

## Phase 4: Action Reducer

Goals:

- create the deterministic mutation path
- validate and apply actions against `RunState` and scenario rules

Deliverables:

- action enum or equivalent command model
- reducer logic for:
  - move
  - inspect
  - take item
  - use item
  - equip item
  - attack
  - wait
  - save
  - load
- structured action results for narrator/UI use

Why here:

The reducer is the heart of truth ownership. It needs to exist before the narrator or polished gameplay loop can be trusted.

## Phase 5: Game Tab, Map Panel, And Log

Goals:

- connect the deterministic state model to the primary play surface
- make the game feel recognizably like Chatty Quest

Deliverables:

- main play tab
- chat-style log
- text input box
- current location display
- map panel
- image or media placeholder panel
- visible objective summary

Important guidance:

The surface should feel chat-first even if the underlying input interpretation remains narrow in `v0.1`.

Future-shape note:

The media panel should be designed around current visual focus, not around trying to display all possible assets at once.

Why here:

This phase turns core logic into a playable loop and lets the team judge whether the project feels alive instead of merely correct.

## Phase 6: Inventory And Character Tabs

Goals:

- expose deterministic player-facing state cleanly
- support gear, HP, and other key run information

Deliverables:

- inventory tab
- equipped item display
- character tab
- HP and basic stats display
- support for simple state-changing item actions

Why here:

These tabs are central to the "real game state" promise and make the distinction from freeform AI storytelling immediately visible.

## Phase 7: Save/Load JSON

Goals:

- serialize the run safely
- restore the run safely

Deliverables:

- JSON save files under `runtime/saves/`
- metadata linking saves back to the correct datapack
- load path that restores deterministic truth

Important rule:

Save data must restore structured state. It must not treat narration or chat history as the source of truth.

Why here:

Persistence is one of the core differentiators of the project. It should arrive before narrator polish, not after it.

## Phase 8: MockNarrator

Goals:

- add flavor without surrendering truth ownership
- prove the narrator seam is replaceable

Deliverables:

- `Narrator` trait or interface
- `MockNarrator` implementation
- styled responses for success, failure, movement, combat, and boundary rules
- tone hooks from capsules where practical

Important rule:

The narrator may describe outcomes richly, but must only consume structured state and reducer results.

Rolling-summary note:

Summary should remain fact-oriented internally. If a future fact affects mechanics, it should be promoted into structured state rather than left in prose.

Why here:

By this point the engine should already be functioning deterministically. The narrator is then added as presentation, not as a crutch.

## Phase 9: `v0.1` Acceptance Test Sweep

Goals:

- verify that the first release promise is actually met
- catch regressions across loading, state, UI, and scenario behavior

Deliverables:

- acceptance checklist execution
- manual playthrough pass
- datapack validation pass
- save/load verification
- boundary-rule verification

Why here:

The project should be judged against the `v0.1` slice it promised, not against future dreams.

## Phase 10: Future Adapter Seams

Goals:

- reserve clean integration points without activating them
- align Chatty Quest with the broader Chatty ecosystem shape

Deliverables:

- dormant handoff folder structure
- documented future packet expectations
- clear code seams for:
  - real narrator adapter replacement
  - future Chatty-Cog orchestration
  - future Chatty-Art media request lanes
  - future Chatty-Lora style or training metadata lanes
  - future NPC memory and reward systems built on the same template/bucket/reducer spine

Important ecosystem reading:

The reference projects consistently separate:

- module-owned real state
- mediated handoff lanes
- copy-only artifact exchange
- user-confirmed actions

Chatty Quest should preserve that spirit later, but `v0.1` must not implement live handoff behavior yet.

## Recommended Module Creation Order

As implementation begins, likely module or folder priorities are:

1. app shell
2. datapack parsing and validation
3. domain types and `RunState`
4. reducer and action results
5. persistence
6. narrator seam
7. UI tabs and panels

The exact Rust module names can evolve, but this dependency order should stay stable.

## Deliberate Deferrals

The roadmap intentionally defers:

- real LLM integration
- multiplayer and peer-to-peer coordination
- live Chatty-Cog mediation
- live Chatty-Art requests
- live Chatty-Lora style/training loops
- advanced procedural generation
- advanced combat systems
- broad natural-language action understanding
- full NPC coherence systems
- dynamic reward negotiation
- generalized context-media override authoring

These are not omissions by accident. They are deferred so the first release can prove the right shape honestly.

## After `v0.1`

`v0.1` is now accepted.

The recommended next planning document is:

- [docs/V0_2_MILESTONE_PLAN.md](c:/Users/User/Desktop/chatty-quest/docs/V0_2_MILESTONE_PLAN.md)

That milestone keeps the project focused on richer deterministic scenario expression before any ecosystem or LLM expansion.
