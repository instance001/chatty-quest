# Instance Migration Plan

## Purpose

This document defines how Chatty Quest and the RD Engine can move from the current `v0.1` runtime model toward explicit runtime instances without breaking:

- save files
- datapack authoring assumptions
- reducer correctness
- diagnostics
- narrator and media seams

The goal is not to rush instance complexity into `v0.1`.

The goal is to make sure future growth happens through a controlled migration path instead of an emergency rewrite.

This document should be read alongside:

- [RUNTIME_MODEL_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/RUNTIME_MODEL_SPEC.md:1)
- [REDUCER_RESULT_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/REDUCER_RESULT_SPEC.md:1)
- [ARCHITECTURE.md](c:/Users/User/Desktop/chatty-quest/docs/ARCHITECTURE.md:1)

## Current State

The current engine uses a valid and intentionally small `v0.1` shape:

- many runtime facts are keyed directly by template ids
- inventory entries are lightweight copied records
- location contents are represented as maps from location id to template id lists
- enemy and boss health is tracked in template-id keyed maps

Current examples:

- `location_items: HashMap<String, Vec<String>>`
- `location_enemies: HashMap<String, Vec<String>>`
- `location_bosses: HashMap<String, Vec<String>>`
- `enemy_hp: HashMap<String, i32>`
- `boss_hp: HashMap<String, i32>`

This is good enough for `v0.1` because it is:

- deterministic
- easy to inspect
- easy to serialize
- good for a single small scenario

But it has a clear ceiling.

## Why Migration Will Be Needed

Template-id keyed runtime state becomes limiting when the engine needs to support:

- multiple copies of the same item template
- multiple enemies sharing the same template
- promised rewards that exist before pickup
- NPC-specific state
- objective branching
- cross-peer handoff of a specific runtime thing
- scenario modes that duplicate or clone authored content at run start

The issue is not that the current system is wrong.

The issue is that template identity and runtime identity are currently fused together in places where future systems will need them separated.

## Migration Philosophy

The migration should be:

- staged
- reversible where practical
- save-versioned
- datapack-compatible
- family-by-family rather than universal all at once

Important discipline:

- do not replace stable `v0.1` systems with abstract machinery unless the new seam solves a real concrete growth problem

## Migration Goals

The migration should eventually provide:

- explicit runtime instance ids
- stable ownership buckets
- template references remaining intact
- save payloads that can restore instance identity safely
- reducer results that describe instance-aware changes cleanly

The migration should not require:

- changing datapack ids
- rewriting authored datapack content into a new content language
- replacing the chat-first surface

## Non-Goal

This plan is not trying to build a generalized ECS or simulation framework.

The engine needs instance-aware truth, not architecture-for-architecture’s-sake.

## Migration Strategy

Recommended strategy:

1. preserve current `v0.1` runtime shape as the compatibility baseline
2. introduce instance ids alongside existing template-id keyed fields where needed
3. teach save/load how to restore both shapes safely
4. migrate one content family at a time
5. remove old parallel fields only after a full compatibility window and test pass

This should be treated as an additive migration first, subtractive cleanup later.

## Stage 0: Document And Freeze Baseline

Status:

- mostly complete

Required outcomes:

- current runtime model documented honestly
- reducer result contract documented honestly
- save payload versioning already visible

Why this matters:

- the team needs a known baseline before any migration can be judged safe

## Stage 1: Introduce Stable Runtime Instance IDs

First explicit step:

- create a stable runtime instance id format

Suggested format:

- `item::<template_id>::<ordinal>`
- `enemy::<template_id>::<ordinal>`
- `boss::<template_id>::<ordinal>`
- `objective::<template_id>::<ordinal or scenario token>`

Exact string format may change, but the important properties are:

- deterministic within a run
- stable through save/load
- distinct from template ids
- human-debuggable

At this stage, template ids still remain the authoring key.

## Stage 2: Migrate Items First

Items are the safest first family to migrate because they already move between world, inventory, equipped, and consumed states.

Recommended future item instance shape:

```text
ItemInstance
  instance_id
  template_id
  owner_bucket
  current_location_id?
  equipped
  consumed
```

Migration approach:

- keep datapack item templates unchanged
- generate item instances during new-run generation
- continue exposing simple inventory UI from derived views
- gradually replace `location_items` template-id lists with item instance ownership records

Why items first:

- they already demonstrate bucket transitions clearly
- duplicate-item support becomes possible immediately
- future rewards and handoff gain a safe identity model

## Stage 3: Migrate Encounters Second

After items, migrate enemies and bosses into explicit encounter instances.

Recommended future shape:

```text
EncounterInstance
  instance_id
  template_id
  encounter_kind
  current_location_id
  current_hp
  status
```

Migration approach:

- keep datapack enemy and boss templates unchanged
- generate encounter instances at run creation
- derive current location encounter lists from instance ownership
- derive alive and defeated sets from explicit status where practical

Why second:

- current combat is already deterministic and simple
- instance-aware encounters unlock repeated templates and future waves
- media focus can still derive from template references while runtime truth improves underneath

## Stage 4: Migrate Objective Runtime Shape

Objectives should become explicit runtime instances once scenarios need:

- branching goals
- optional side goals
- generated target binding
- multiple concurrent objectives

Recommended future shape:

```text
ObjectiveInstance
  instance_id
  template_id
  status
  bound_target_ids
  completion_source
```

This can remain later than items and encounters because current `v0.1` objective needs are still small.

## Stage 5: Consolidate Bucket Families

Once item and encounter instances exist, bucket ownership can become more explicit and less parallel.

Likely future bucket families:

- location state records
- item ownership buckets
- encounter status buckets
- objective status buckets

Possible future simplifications:

- replace separate alive and defeated sets with status fields
- replace world-location arrays with owner-bucket or location-owner fields
- derive UI-friendly lists rather than storing duplicate representations

Important rule:

- only consolidate after downstream systems have stable derived views

## Save Compatibility Plan

Save migration must be versioned explicitly.

Current save version:

- `1`

Recommended migration behavior:

### Save Version 1

- current direct `RunState`
- template-id keyed runtime buckets

### Save Version 2

- add optional instance collections
- keep compatibility loader for version 1
- if loading version 1, synthesize runtime instances from old fields

### Save Version 3

- retire old duplicate fields only after the compatibility loader and tests are stable

Important discipline:

- do not silently break old saves just because runtime internals improved

## Datapack Compatibility Plan

Datapacks should remain largely unchanged through the instance migration.

Reason:

- datapacks define templates, not runtime instances

The generator should be responsible for turning authored template content into runtime instances.

This keeps modding stable while runtime internals improve.

Good migration outcome:

- the modder still writes `medkit`
- the runtime may create `item::medkit::0`

That is exactly the separation we want.

## Reducer Migration Plan

The reducer should migrate by changing what it mutates internally, not by changing who owns truth.

This means:

- action input stays structured
- reducer still owns canonical mutations
- events should gradually become instance-aware where needed

Recommended event migration pattern:

- keep template ids where they are enough for `v0.1`
- add instance ids alongside template ids when a content family migrates

Example future event:

```text
ItemTaken
  item_instance_id
  template_id
```

This preserves compatibility for presentation while improving runtime specificity.

## UI Migration Plan

The UI should consume derived views rather than raw runtime internals wherever practical.

That means:

- inventory tab should not care whether the runtime stores template-id lists or item instances internally
- map should care about location truth, not storage representation
- media panel should care about resolved focus context, not bucket implementation detail

This derived-view discipline is what makes staged migration feasible.

## Narrator And Media Compatibility

Narrator and media systems should continue to receive:

- template-facing descriptive context
- reducer-confirmed events
- current location truth

They do not need to become instance-centric immediately unless the scenario depends on that distinction.

This protects the chat-first experience from internal storage churn.

## Diagnostics And Tooling

Diagnostics should evolve to surface both:

- template compatibility
- runtime instance health

Future useful checks:

- duplicate instance ids
- invalid owner buckets
- orphaned instances
- inconsistent alive/dead versus HP state
- instance references to missing templates

Migration is much safer when diagnostics become stricter as runtime complexity grows.

## Risks To Avoid

### Parallel Truth Risk

Biggest risk:

- keeping old template-id fields and new instance fields both authoritative for too long

Mitigation:

- define one canonical source per family during each migration stage
- make the other representation derived or compatibility-only

### Save Drift Risk

Biggest risk:

- old saves loading into subtly wrong runtime identity

Mitigation:

- explicit save version handling
- migration tests from older payloads
- diagnostics warnings when compatibility reconstruction occurs

### Over-Abstraction Risk

Biggest risk:

- building a universal runtime entity framework before a real scenario needs it

Mitigation:

- migrate only the families that are actually under pressure
- keep the engine small and inspectable

## Recommended Order

Best practical migration order:

1. item instances
2. encounter instances
3. objective instances
4. richer location state records
5. cleanup and old-field retirement

This order matches where the current architecture will feel pressure first.

## Exit Criteria For A Migration Stage

A family migration should not be considered complete until:

- canonical source of truth is unambiguous
- save/load round-trips correctly
- diagnostics can inspect the new shape
- UI still behaves correctly through derived views
- reducer results still provide stable downstream context
- existing datapacks still play without author changes

## `v0.1` Guidance

For `v0.1`, the correct move is not to implement this migration immediately.

The correct move is:

- document it
- preserve compatibility-friendly seams
- avoid design choices that make future migration harder

That means the current direct runtime shape is acceptable.

It just should not be mistaken for the final architectural destination.
