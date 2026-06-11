# Runtime Model Spec

## Purpose

This document defines the canonical runtime model for Chatty Quest and the RD Engine.

It exists to answer one practical question clearly:

- what lives in templates
- what lives in runtime state
- what should become an instance
- what should remain a bucket or status field
- what the reducer is allowed to mutate
- what the UI, narrator, media layer, and save/load system may only derive or display

This is the implementation-facing companion to:

- [ARCHITECTURE.md](c:/Users/User/Desktop/chatty-quest/docs/ARCHITECTURE.md:1)
- [STATE_AND_BUCKETS.md](c:/Users/User/Desktop/chatty-quest/docs/STATE_AND_BUCKETS.md:1)
- [DATAPACK_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/DATAPACK_SPEC.md:1)
- [REDUCER_RESULT_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/REDUCER_RESULT_SPEC.md:1)
- [INSTANCE_MIGRATION_PLAN.md](c:/Users/User/Desktop/chatty-quest/docs/INSTANCE_MIGRATION_PLAN.md:1)

## Doctrine

The runtime model follows one simple doctrine:

- templates define what can exist
- instances define this specific spawned thing
- buckets define what currently applies
- reducer-visible state defines truth
- logs, summaries, narrator prose, and UI widgets describe truth

Short rule:

- if a fact affects mechanics later, it must exist in runtime state
- if a fact only affects tone or recap, it may remain presentation support data

## Runtime Ownership

The runtime model is the canonical authority for a live run.

It owns:

- current location
- known and visited map knowledge
- item possession and placement
- equipment state
- player HP and future stats
- enemy and boss life state
- objective progress
- boundary-relevant scenario state
- reducer event history needed for diagnostics or support systems
- save-serializable truth

It does not delegate truth ownership to:

- narrator text
- rolling summary
- media focus
- UI selection state
- temporary visual cues

## Runtime Layers

The runtime model should be understood as four layers.

### 1. Scenario Identity Layer

This identifies what world the run belongs to.

Examples:

- datapack id
- datapack display name
- scenario id if needed separately later
- generation seed or setup metadata
- active setup toggles such as fog mode or difficulty if they affect run truth

This layer is required so save/load, diagnostics, and future handoff systems know what content contract the run is using.

### 2. Canonical State Layer

This is the reducer-owned core of the live run.

Examples in current `v0.1`:

- `current_location_id`
- `known_locations`
- `visited_locations`
- `inventory`
- `equipped_item_id`
- `hp`
- `max_hp`
- `active_objective`
- `enemies_alive`
- `enemies_defeated`
- `enemy_hp`
- `bosses_alive`
- `bosses_defeated`
- `boss_hp`
- `location_items`
- `location_enemies`
- `location_bosses`
- `boundary_response`

This layer is authoritative and serializable.

### 3. Support Memory Layer

This exists to help presentation, diagnostics, and future adapters without becoming the source of mechanics.

Current `v0.1` examples:

- `rolling_summary`
- recent reducer events
- log lines

Important boundary:

- support memory may explain state
- support memory may not silently replace state

### 4. Derived Presentation Layer

This is not canonical run truth. It is derived from it.

Examples:

- map tile layout coordinates
- current media focus selection
- current asset viewer request
- selected gameplay tab
- current input text
- diagnostics presentation widgets

This layer may be rebuilt from canonical truth and local UI state at any time.

## Template, Instance, Bucket

The engine should preserve a clear three-part mental model even where `v0.1` uses a lighter implementation.

### Template

A template is authored content from a datapack.

Examples:

- location template
- item template
- enemy template
- boss template
- objective template

Template fields define canonical authored facts such as:

- ids
- names
- descriptions
- tags
- base stats
- explicit media references
- authored connections

Templates are not mutated during play.

### Instance

An instance is a runtime-spawned concrete copy of a template.

Examples:

- the medkit currently sitting in the kitchen in this run
- the specific garage boss in this run
- a specific objective selected for this run
- a future NPC spawned into a scenario variation

Instances become important when the engine needs to answer questions like:

- which copy moved
- which copy was consumed
- which copy was promised as a reward
- which copy was handed off to another peer

Current `v0.1` uses a simplified shape where many runtime facts are represented by template ids directly. That is acceptable for the first slice, but it is a simplification, not the long-term canonical target.

### Bucket

A bucket is the live status or container relationship currently applied to a template or instance.

Examples:

- known versus unknown location
- visited versus unvisited location
- item in world versus item in inventory
- item equipped versus unequipped
- enemy alive versus defeated
- objective active versus completed

Buckets may be represented as:

- sets
- maps
- enum status fields
- container ownership fields

The exact Rust structure matters less than the truth model.

## Current `v0.1` Runtime Shape

The current implementation is best described as:

- template-authored content
- direct canonical run state
- template-id keyed buckets
- lightweight support memory

That means the current engine does not yet model all runtime entities as explicit instance structs.

Examples:

- `location_items` maps location ids to item template ids
- `inventory` stores lightweight item entries derived from template content
- `enemy_hp` maps enemy ids directly to HP values
- `boss_hp` maps boss ids directly to HP values

This is a valid `v0.1` implementation strategy because it is:

- deterministic
- inspectable
- serializable
- easy to reason about

But it should be treated as a deliberately small first shape, not as proof that instance seams are unnecessary.

## Recommended Future Instance Seams

When the engine grows, the following runtime entity families should become first-class instances.

### Item Instances

Recommended future fields:

- `instance_id`
- `template_id`
- `owner_bucket`
- `current_location_id` when world-placed
- `equipped_by` if held and equipped
- `consumed`
- `durability` if used later

Why:

- supports duplicate items cleanly
- supports reward promises cleanly
- supports multiplayer handoff cleanly
- prevents item identity from collapsing into template identity

### Encounter Instances

Recommended future fields:

- `instance_id`
- `template_id`
- `encounter_kind`
- `current_location_id`
- `current_hp`
- `status`
- scenario flags relevant to that encounter

Why:

- supports more than one enemy of the same template
- supports future squad or wave scenarios
- keeps combat state local to real runtime entities

### Objective Instances

Recommended future fields:

- `instance_id`
- `template_id`
- `status`
- bound target ids
- prerequisite flags
- completion source

Why:

- future scenarios may generate or branch objectives per run
- objective logic should remain explicit and serializable

### Location State Records

Recommended future fields:

- `location_id`
- `known`
- `visited`
- `locked`
- `no_return`
- future scenario-local flags

Why:

- makes richer location logic easier than parallel sets
- keeps future map conditions inspectable

## Reducer Contract

The reducer is the only system allowed to mutate canonical runtime truth during play.

The reducer may:

- move the player
- transfer items between buckets
- equip and unequip items
- change HP
- change enemy or boss life state
- complete objectives
- append structured events
- update support memory in tightly controlled ways

The reducer may not:

- infer canon from prose alone
- let media state create mechanics
- let UI state create mechanics
- let rolling summary silently backfill missing truth

## Runtime Invariants

The following invariants should remain true.

### Location Invariants

- the player is in exactly one valid location
- every reducer-legal move must target an authored connection or scenario-approved equivalent
- known and visited location state must remain consistent with actual play history

### Item Invariants

- an item cannot be both in the world and in inventory at once
- an equipped item must also belong to the player inventory or a future equivalent owner bucket
- a consumed item must not remain active in inventory

### Encounter Invariants

- a defeated enemy cannot also be alive
- a defeated boss cannot also be alive
- HP maps must not contain contradictory life-state ownership
- objective completion may not contradict encounter truth

### Objective Invariants

- an objective must have a stable id and completion source
- a completed objective must reflect reducer-confirmed state
- narrator celebration may not create objective completion by itself

### Presentation Invariants

- map UI may not invent exits
- media focus may not imply a state change that did not occur
- rolling summary may not be the only source of a mechanically relevant promise
- save/load must restore canonical truth without replaying prose

## Save Model

Save files should serialize canonical runtime truth plus only the support data that is actually useful.

Must save:

- scenario identity
- canonical state
- setup metadata that affects truth
- objective state
- combat state
- inventory and location placements

May save:

- rolling summary
- recent logs
- selected tab or UI convenience state if clearly separated

Must not rely on:

- narrator prose replay
- inferred state from old media focus
- reconstructing truth from chat text

## Narrator Payload Boundary

A future-friendly runtime model should expose structured data to the narrator rather than letting the narrator inspect arbitrary internals.

The narrator input should be derived from:

- action intent
- reducer outcome
- current location state
- visible encounter state
- objective state
- support memory summaries

The narrator output should remain presentation only.

## Media Payload Boundary

The media layer should consume runtime truth and reducer events, not invent them.

Good media inputs:

- current location
- focused item or encounter
- recent event type
- objective relevance
- authored media references

Bad media inputs:

- freeform narrator guesses treated as state
- UI-only hover state treated as canon

## Handoff Boundary

Future Chatty-Cog peer handoff should exchange structured runtime payloads, not whole narrator authority.

Good future handoff candidates:

- scenario identity
- current canonical run snapshot
- explicit bucket state
- explicit instance state once the engine supports it
- recent structured event slice

Not good handoff candidates:

- raw prose as the only truth source
- media selection as canonical state
- UI tab state as gameplay truth

This keeps Chatty Quest responsible for its own current state while allowing future peer-connected systems to pass stable run fragments safely.

## `v0.1` Guidance

For `v0.1`, the runtime model should remain:

- explicit
- small
- serializable
- easy to inspect
- easy to validate

It does not need a universal entity framework yet.

It does need:

- a clean conceptual split
- honest documentation about current simplifications
- a visible path toward instance-aware runtime state later

That is the architectural middle ground worth protecting.
