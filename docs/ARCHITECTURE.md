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

Example deterministic actions for `v0.1`:

- move
- inspect
- take item
- use item
- equip item
- attack
- wait
- save
- load

If a state change matters to the game, it should happen here or through a tightly controlled equivalent mechanism.

If a fact affects mechanics later, it should be promoted into reducer-visible structured state instead of living only in prose or summary text.

## Narrator Seam

The narrator is a replaceable presentation layer that turns structured outcomes into player-facing prose.

For `v0.1`, this is a `MockNarrator`.

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

The narrator seam exists so a future real LLM can be added without replacing the engine's truth model.

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

## Future Adapter Seams

The project should reserve future seams without activating them in `v0.1`.

Likely future seams include:

- real LLM narrator adapters
- handoff lanes for Chatty-Cog orchestration
- handoff lanes for Chatty-Art asset requests
- handoff lanes for Chatty-Lora style consistency metadata

In `v0.1`, these seams should remain visible but dormant.

Likewise, future NPC memory systems, dynamic rewards, and richer media overrides should be designed as extensions of the same template/bucket/reducer spine, not as parallel improvisational subsystems.

## Architectural Priorities For `v0.1`

`v0.1` architecture should optimize for:

- deterministic correctness
- clear separations of responsibility
- small playable scope
- datapack-driven scenario identity
- a replaceable narrator layer

It should not optimize for speculative future complexity at the cost of the first playable version.
