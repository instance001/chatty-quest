# Architecture

## Core Principle

Templates define canon. Buckets track state. Reducer mutates truth. Narrator describes truth. UI displays truth.

This line is the governing rule of the project architecture.

Every major feature in Chatty Quest should map back to one of these responsibilities. If a feature blurs them, it should be reconsidered.

Useful shorthand:

- templates are the nouns
- buckets are the current grammar
- reducers are the verbs
- narration is the accent
- media is the illustration

## Current Implemented Substrate

The current branch has moved beyond the historical `v0.1` proof slice. The following are active
engine contracts, not future aspirations.

### Capability-Gated Mechanical Contracts

Capabilities are roots of schema subtrees. A capability's registry entry defines or points to its
dependent fields, defaults, prerequisites, validation, and runtime requirements. That one source
drives authored validation, bounded proposal forms, approval UI, runtime mutation validation, and
persistence.

For example, `combat` makes combat fields such as damage applicable, while `inventory` makes an
entity a valid ownership endpoint. A disabled capability makes its dependent data inapplicable and
its mechanics unavailable. Entity nouns do not own mechanics: an NPC, item, player, boss, or
future summon uses the same contract whenever it has the relevant capability.

### Generic Actors And Control

The player is a runtime entity selected by `player_entity_id`; it is not a special combat or
inventory implementation. The currently controlled entity is the origin of player-driven actions.
Actors use shared host actions for movement, combat, perception, inventory, abilities, and timing.
Changing control changes who is attempting an action, never which reducer system makes it true.

The parser can preserve a target request such as `attack <target>`, but it does not choose a
controller or decide success. The reducer resolves the current controlled actor, validates its
capabilities and the target's state, then commits or rejects the action.

### Events, Projection, And Presentation

Each committed mechanic emits `GameEvent` truth. The event projection layer derives one generic
downstream representation:

```text
type | origin | impact | outcome
```

Action traces, narration context, diagnostics, media focus, summaries, and epilogue routing use
that projection. A downstream consumer should ask what happened, not hand-maintain a separate
interpretation of every event variant. Presentation may decorate a reducer-confirmed event but
may never claim a mutation that was not committed.

### Ownership And Abilities

`TransferItem { item_id, origin, destination }` is the sole authoritative ownership mutation.
Location pickup, drop, give, return, theft, equip, and unequip resolve into it and emit an
`ItemTransferred` event. Equipment is a derived relationship over canonical ownership.

Ability templates are immutable content. An `AbilityGrant` is a distinct owned runtime copy with
its own charges, expiry, cooldown, source, and owner. The same template can therefore be
permanent knowledge, a one-use scroll, a charged staff effect, or temporary access without shared
mutable charges. See [ABILITY_GRANTS_SPEC.md](ABILITY_GRANTS_SPEC.md).

### Media

Media resolves from reducer-confirmed semantic roles through a hierarchy of exact template asset,
template-preferred pool or tag, family pool, category pool, and global fallback. The resolution is
deterministic when the run needs it, varies within eligible pools, and visibly identifies its
chosen asset and source. Media is strictly non-authoritative.

## Engine Core

The engine core is the deterministic heart of the project.

Its responsibilities include:

- loading or receiving validated scenario data
- generating or assembling the starting run state
- accepting player actions
- validating those actions against scenario rules and current state
- mutating canonical state through deterministic logic
- producing structured results for the UI and narrator

The engine core must remain independent from any one narration implementation.

## Datapack Layer

The datapack layer defines playable scenario content and assets.

It is responsible for:

- pack metadata
- scenario rules
- template files
- capsule files for tone
- media references

The datapack layer should allow the engine to support multiple scenario families without rewriting core game logic.

For `v0.1`, `Property Siege Classic` is the active datapack and the proof that this layer works.

Longer-term, template data should remain conceptually separable from runtime instances and bucket state, even where `v0.1` uses simplified representations.

## Scenario Rules

Scenario rules describe what is allowed, what is blocked, and what counts as success or failure inside a specific world.

Scenario rules may define:

- map boundaries
- return policies
- lock and unlock conditions
- objective logic
- boss placement
- scenario-specific failure responses

The engine should not hardcode scenario-specific assumptions when those assumptions belong in data.

Example:

The player being unable to leave the property in `Property Siege Classic` is a scenario rule, not a universal engine law.

The test suite includes a live-source boundary guard that scans non-test Rust code for active demo content ids and names. Scenario-specific ids such as demo locations, items, enemies, and bosses should appear in datapack assets, documentation, or test fixtures, not in live engine decisions.

## Runtime State

Runtime state represents the current truth of a single run.

The deeper implementation-facing shape is documented in `docs/RUNTIME_MODEL_SPEC.md`.

It should include structured fields for:

- player location
- known and visited locations
- inventory and equipped item
- HP and basic stats
- live enemy and boss state
- objective progress
- scenario boundary-relevant flags
- rolling summary support data
- log or turn history support data as needed

Runtime state must be serializable for save/load and must remain authoritative over the narrator.

The future-friendly mental split is:

- template: what something is
- instance: this specific copy
- bucket/state: where it is now and what currently applies

## Action Reducer

The reducer is the only component allowed to mutate canonical game truth during play.

The deeper reducer-output contract is documented in `docs/REDUCER_RESULT_SPEC.md`.

Its responsibilities include:

- receiving structured actions
- checking whether the action is legal
- applying valid state mutations
- rejecting invalid transitions
- producing structured outcomes

Current deterministic action families include:

- move
- inspect
- take item
- use item
- equip item
- transfer item
- attack a resolved target
- cast, learn, grant, and progress an ability through host-owned contracts
- wait
- save
- load

If a state change matters to the game, it should happen here or through a tightly controlled equivalent mechanism.

If a fact affects mechanics later, it should be promoted into reducer-visible structured state instead of living only in prose or summary text.

## Narrator Seam

The narrator is a replaceable presentation layer that turns structured outcomes into player-facing prose.

The active runtime supports Mock, local GGUF, and configured hosted narrator adapters. All remain
presentation adapters; none receive mutation authority.

The narrator may:

- describe the current scene
- frame success and failure
- provide humour, horror, or atmosphere
- speak in a specific DM capsule tone

The narrator may not:

- create permanent items
- invent new canonical locations
- alter HP directly
- move the player directly
- unlock locations directly
- complete objectives directly
- mutate any other authoritative state directly

The narrator seam exists so a future live LLM can be added without replacing the engine's truth model.

The command boundary before that seam is documented in `docs/COMMAND_BOUNDARY_SPEC.md`.

The implementation-facing payload for that seam is documented in `docs/NARRATOR_CONTEXT_SPEC.md`.

The narrator should be free to add style, humour, menace, cadence, and social texture.

It should not be asked to carry hidden mechanics, authoritative promises, or canonical state transitions in prose alone.

## UI Shell

The UI shell presents the current run to the player through a desktop dashboard.

The broader rule for UI-, media-, diagnostics-, and export-facing derived models is documented in `docs/DERIVED_VIEW_SPEC.md`.

For `v0.1`, the UI should support:

- main menu
- datapack selection
- new game and load game entry points
- game tab with chat log and text input
- map panel
- image or media placeholder panel
- inventory tab
- character tab

The UI should preserve the fantasy of "talking to a Dungeon Master inside a real game," not simply expose raw state tables.

The UI also carries the job of proving that reducer-confirmed changes really happened:

- the inventory shows the item was actually received
- the map shows the move actually happened
- the objective panel shows the win actually completed
- the media panel reflects current reducer-confirmed focus

## Save/Load

Save/load preserves deterministic run truth between sessions.

The save system should capture:

- enough structured runtime state to fully restore play
- scenario or datapack identity
- any seed or generation metadata needed for consistency
- narrator-adjacent support data only where useful and non-authoritative

The save system must not treat chat text as the source of truth.

UI session state such as the selected gameplay tab may also be saved when convenient, but it should stay clearly separate from canonical run truth.

Rolling summary or chat history may be saved as support context, but save integrity must never depend on replaying prose to recover mechanics.

## UI Shell Note

The recommended shell model is documented in `docs/UI_SHELL_SPEC.md`.

That model emphasizes:

- setup-first before a run exists
- gameplay tabs after generation or load
- a chat-first `Game` tab with side panels
- clean expansion through new tabs instead of permanent clutter

## Adapter Seams

The project keeps external adapters separate from the deterministic substrate.

Likely future seams include:

- live LLM narrator adapters
- handoff lanes for Chatty-Cog orchestration
- handoff lanes for Chatty-Art asset requests
- handoff lanes for Chatty-Lora style consistency metadata

Live narrator adapters are active. Handoff, external asset-generation, and model-training lanes
remain optional integrations rather than runtime authorities.

Likewise, future NPC memory systems, dynamic rewards, and richer media overrides should be designed as extensions of the same template/bucket/reducer spine, not as parallel improvisational subsystems.

## Architectural Priorities For `v0.1`

`v0.1` architecture should optimize for:

- deterministic correctness
- clear separations of responsibility
- small playable scope
- datapack-driven scenario identity
- a replaceable narrator layer

It should not optimize for speculative future complexity at the cost of the first playable version.
