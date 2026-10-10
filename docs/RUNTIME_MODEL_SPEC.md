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

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [STATE_AND_BUCKETS.md](STATE_AND_BUCKETS.md)
- [DATAPACK_SPEC.md](DATAPACK_SPEC.md)
- [REDUCER_RESULT_SPEC.md](REDUCER_RESULT_SPEC.md)
- [INSTANCE_MIGRATION_PLAN.md](INSTANCE_MIGRATION_PLAN.md)

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

## Capability, Actor, And Ownership Invariants

An enabled mechanical capability is a promise of a real host-owned action path. The capability
registry is the root of its dependent schema: it defines applicable child fields, defaults,
validation, prerequisites, and runtime requirements. Child fields are not a giant flat template
surface and are inapplicable when their parent capability is disabled.

Runtime actors are entity-agnostic. The controlled player, NPCs, enemies, bosses, pets, summons,
and actor-capable items may share a single actor state and action dispatch path when their
capabilities permit it. `player_entity_id` identifies the entity currently controlled by the
player; it does not grant that entity separate mechanics.

Ownership is similarly generic. Eligible entities with the inventory contract and locations are
endpoints. Every ownership mutation is `TransferItem { item_id, origin, destination }`, which
commits one `ItemTransferred` event before optional social or presentation consequences. Equipment
is a relationship layered over ownership and cannot outlive it.

An ability definition is template data. An `AbilityGrant` is runtime state owned by an actor or
item. It carries the mutable use count, expiry, cooldown, source, and ownership identity. Progress
pools are actor-local; controlling another actor never transfers the default player's XP, grants,
or milestone choices.

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
- `turn_index` (the authoritative integer world-time cursor)
- `actor_ready_at` (absolute per-runtime-entity action readiness)
- `noise_level`
- `noise_spawn_count`
- spawned-enemy noise, sight, origin, search, and delay state
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

`turn_index` advances by the successful action's positive resolved world-time cost. Controller
identity never chooses that cost: actor/template/action data does. `actor_ready_at` records when
each runtime entity may next take an ordinary action, preserving the same mechanics if control
changes. Rejected and movement-blocked commands do not advance time or readiness. Deterministic
percentage rolls and random-feeling reducer choices may use `turn_index` as a replayable cursor;
bounded support memory such as `rolling_summary` must not be used as the only source of roll
variation.

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

## Current Runtime Shape

The current implementation is a compatibility-preserving hybrid:

- template-authored content remains immutable scenario canon
- canonical run state stores explicit runtime instances where identity or mutable state matters
- legacy template-id maps remain only as compatibility mirrors for established enemy/boss rules
- session scratch truth uses the same reducer-owned instance buckets

Implemented instance seams include `instance_id`, `template_id`, bucket/lifecycle, provenance, and
location/owner state for world/inventory items, NPCs, runtime encounters, objectives, temporary
exits, and status effects. `ActorState` is reusable state for entities that opt into host-owned
health, perception, movement, or combat capabilities. The host capability registry is the single
vocabulary for those mechanics; a template family never receives mechanics merely because it is
called an enemy, NPC, boss, or item.

Examples:

- `world_item_instances` and inventory entries retain item ownership/location and provenance
- `location_npcs` stores authored and runtime NPC instances with disposition and actor state
- `enemy_instances` and `boss_instances` retain live/defeated encounter state alongside legacy
  HP maps used by existing reducer paths
- the shared noise-pool blackboard keeps target/origin/search/sight state by spawned instance id

Autonomous outcomes are entity-agnostic reducer facts: `ActorSpawnedFromNoise`, `ActorMoved`,
`ActorWaited`, and `ActorAttackedHazard` carry an actor origin id rather than assuming the
actor is an enemy. Noise and sight-attractor events likewise carry actor ids. Diagnostics,
narration, action traces, and media consume those shared events, so a capability-enabled NPC or
item follows the same causal presentation path as an encounter actor.

The host exposes active actor-capable entities at a location through one runtime actor directory.
That directory is a read projection over compatibility storage, not a second state store: it
includes the controlled player projection, active NPCs, and only items that explicitly opt into an
actor contract (including actor-capable items held by the player or an NPC, at the holder's effective
location). Combat dispatch, named actor instructions, autonomous friendly turns, and
observation surfaces consume this directory rather than adding separate NPC/item lookup paths.
Entity nouns remain presentation and storage details; capability contract and disposition determine
which host action is available.

The controlled player is included as a compatibility projection. Player Attack and Move first enter
the generic actor-action dispatch, then bridge to their established deterministic reducers so
equipment damage, fog, objective, and save semantics remain authoritative during the migration.
This is intentionally a bridge, not a second player mechanics system.

This shape is deterministic, inspectable, serializable, and allows a template family to opt into a
shared engine capability without handing authority to the narrator.

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
- explicit save version
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

- implicit best-effort parsing of unknown save versions
- narrator prose replay
- inferred state from old media focus

Current implementation note:

- `SAVE_VERSION = 1`
- missing `save_version` fields are treated as `v1` for backward compatibility with early local saves
- unsupported older placeholders and future save versions fail with explicit load errors
- migration logic lives in the runtime layer, before a parsed payload is returned to app state
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

It does not need a new parallel entity framework yet. It does need all new actor mechanics to be
routed through the host capability registry and shared `ActorState`, rather than added as
pack-specific or template-species-specific logic.

It does need:

- a clean conceptual split
- honest documentation about current simplifications
- a visible path toward instance-aware runtime state later

That is the architectural middle ground worth protecting.
