# `v0.2` Locked Progression Spec

## Purpose

This document fixes the exact shape of the first `v0.2` mechanic before code work begins.

The goal is not to build a generalized puzzle system.

The goal is to prove one deterministic gated progression beat that:

- is driven by datapack content
- changes canonical state through the reducer
- is visible in UI and diagnostics
- persists through save/load

## First `v0.2` Mechanic

`house_keys` unlock the `garage`.

This is the first post-`v0.1` scenario-depth expansion for `Property Siege Classic`.

## Why This Gate

This is the best first gate because:

- `house_keys` already exist in the scenario
- the `garage` is already the boss room and objective destination
- the current route becomes mechanically richer without increasing map size
- the player can understand the rule immediately

This converts the current route from:

- move directly to garage and fight boss

into:

- search house
- find keys
- return to entry
- unlock garage
- fight boss

## Exact Scenario Change

### Locked Location

Locked location:

- `garage`

Initial lock state:

- locked at run start

Lock presentation intent:

- the garage is physically present and visible from the front verandah
- the player is blocked by a locked door, not by abstract engine denial

### Unlocking Item

Unlocking item:

- `house_keys`

Current item location:

- `laundry`

The player must still reach `laundry` through the existing route:

- `front_verandah` -> `kitchen` -> `laundry`

## Command And Reducer Behavior

### Movement While Locked

If the player attempts:

- `go garage`

while the garage is still locked, movement must fail.

Expected reducer result:

- no location change
- no hidden partial progress
- explicit failure line explaining that the garage is locked

Suggested player-facing line:

- `The garage door is locked. You need the house keys.`

### Unlock Action

First-pass unlock command:

- `use house_keys`

Valid use context:

- player is at `front_verandah`
- player has `house_keys` in inventory
- `garage` is still locked

Expected reducer result:

- `garage` becomes unlocked
- state change is durable
- command succeeds even though the player does not move yet

Suggested player-facing line:

- `You unlock the garage.`

### Invalid Use Cases

If the player uses `house_keys`:

- in the wrong location
- before picking them up
- after the garage is already unlocked

the reducer should reject the action clearly.

Suggested result lines:

- `These keys do not help here.`
- `You do not have that item.`
- `The garage is already unlocked.`

## Objective Behavior

The primary objective does not change in the first pass.

Objective remains:

- reach the garage
- kill the `Garage Brute`

What changes is the route to objective completion.

This keeps the mechanic isolated:

- one new state family
- one new deterministic use case
- no objective-system rewrite yet

## Datapack Shape

The first pass should stay narrow and explicit.

Recommended datapack additions:

- location-level locked flag or gate metadata on `garage`
- location-level unlock item reference pointing to `house_keys`
- optional locked description or locked rejection text if needed

Avoid for now:

- generalized multi-step conditions
- arbitrary scriptable rule chains
- reusable puzzle DSL

The schema should support future generalization, but this implementation should stay concrete.

## Runtime State

`RunState` needs a deterministic lock-state family for locations.

Minimum required truth:

- whether `garage` is locked or unlocked

Recommended future-friendly shape:

- a map or set keyed by location id

Do not bury this only in prose or derived UI state.

## UI Truth Surfacing

The new state must be visible outside narration.

Required surfaces:

- `Game` tab current exits
- map tile display for `garage`
- character summary if it already surfaces objective-adjacent state
- diagnostics report

Recommended `v0.2` first-pass presentation:

- current exits list should still show `Garage`, but indicate it is locked
- map tile should visually distinguish locked vs unlocked garage
- diagnostics should include lock-state truth explicitly

The player should not have to infer the lock purely from failed movement prose.

## Save/Load

Lock state must round-trip through save/load.

Required persistence behavior:

- a locked garage stays locked after save/load
- an unlocked garage stays unlocked after save/load

## Tests Required

Minimum automated coverage:

- generated run starts with garage locked
- moving to garage while locked fails
- taking `house_keys` then using them at `front_verandah` unlocks garage
- unlocked garage remains unlocked after save/load
- attempting to unlock in the wrong context fails cleanly

## Explicit Non-Goals

Do not include these in the first locked-progression pass:

- lockpicking
- consumable keys
- multiple locked rooms
- key durability
- branching gate logic
- new objective types
- broad natural-language unlock parsing

## Implementation Order

1. extend datapack schema for locked garage metadata
2. extend `RunState` for lock truth
3. add reducer and parser support for unlock behavior
4. update `Property Siege Classic` content
5. surface lock state in UI and diagnostics
6. add tests
7. update acceptance docs once mechanic is stable
