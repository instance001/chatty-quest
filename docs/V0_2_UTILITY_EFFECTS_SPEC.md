# `v0.2` Utility Effects Spec

## Purpose

This document records the exact shape of this `v0.2` mechanic.

The goal is not to build a general item-scripting engine.

The goal is to prove that datapack items can carry deterministic non-combat utility behavior beyond healing and lock access.

Status note:

- this mechanic is implemented on the current branch
- `torch` uses `utility_effect = "reveal_connections"`
- validation rejects unknown utility-effect values
- reducer tests cover route reveal and stable no-new-info behavior

## First Utility Effect

Add an item-backed reveal effect for map knowledge.

First supported effect:

- reveal connected locations from the current position

Recommended first content use:

- `torch`

## Why This Effect First

This is the best next step because:

- the torch already exists and currently reads as useful but under-expressed
- the map and fog systems already exist, so this effect has a clear truth target
- it broadens utility state without requiring new enemies, stats, or puzzle chains
- it proves item effects can alter deterministic knowledge state, not just HP or locks

## Exact Behavior

When the player uses an item with the reveal effect:

- the reducer inspects the current location
- every directly connected location becomes known
- already-known locations stay known
- the player does not move
- no hidden narration-only progress occurs

Suggested first-pass result line:

- `You sweep the torch across the exits and get a better read on the nearby routes.`

If no new location knowledge is gained, the effect should still respond cleanly.

Suggested result line:

- `The torch does not reveal anything new from here.`

## Datapack Shape

First-pass schema should stay narrow.

Recommended item fields:

- optional `utility_effect`

First supported value:

- `reveal_connections`

Example:

```toml
[[items]]
id = "torch"
name = "Torch"
description = "A slightly unreliable flashlight."
tags = ["utility", "starter_item", "weak_weapon"]
damage = 1
utility_effect = "reveal_connections"
```

Validation rules:

- unknown `utility_effect` values must be rejected
- items without `utility_effect` remain unchanged

## Runtime Truth

No new top-level run-state field is required for the first pass.

The effect writes into existing state:

- `known_locations`

That keeps the mechanic small while still proving a second deterministic utility family.

## UI Truth Surfacing

The effect should become visible through existing surfaces:

- map tiles in `Known` and `Visited` fog modes
- known-locations list
- rolling summary
- diagnostics recent events

No dedicated new tab or panel is needed.

## Reducer Behavior

The reducer should support the reveal effect through normal `use <item>` flow.

Effect ordering:

1. resolve held item
2. if the item matches an unlock use in context, handle the unlock path
3. otherwise, if the item has `utility_effect = "reveal_connections"`, apply reveal logic
4. otherwise, fall back to the existing no-effect message

This preserves the current gate behavior while allowing multiple utility families to coexist.

## Tests Required

Minimum automated coverage:

- datapack validation rejects unknown `utility_effect`
- generated run preserves the torch utility metadata through bundle load
- using the torch from `front_verandah` reveals connected locations
- using the torch again without gaining anything new reports a stable no-new-info line
- save/load preserves the revealed known-location state because it already lives in `known_locations`

## Explicit Non-Goals

Do not include these in the first utility-effect pass:

- arbitrary per-item scripts
- durability or battery systems
- limited charges
- item cooldowns
- multi-step scanning minigames
- fog rewriting outside deterministic location knowledge

## Implementation Order

1. extend datapack item schema with optional `utility_effect`
2. validate allowed values
3. update `Property Siege Classic` so `torch` uses `reveal_connections`
4. extend reducer `use` flow
5. ensure existing map and known-location surfaces reflect the effect automatically
6. add tests
7. update docs once stable

## Current Role In `v0.2`

Utility effects are now a completed first-pass foundation.

The next utility-effect work should wait for a concrete scenario need rather than expanding into arbitrary item scripting.
