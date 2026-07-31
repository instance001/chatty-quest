# `v0.2` Locked Progression Spec

## Purpose

This document records the exact shape of the first `v0.2` mechanic.

Status note:

- refreshed on `2026-07-16`
- this mechanic is implemented on the current branch

The goal is not to build a generalized puzzle system.

The goal is to prove one deterministic gated progression beat that:

- is driven by datapack content
- changes canonical state through the reducer
- is visible in UI and diagnostics
- persists through save/load

## Current Implemented Mechanic

`house_keys` unlock the `garage` and `back_garden`.

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

### Locked Locations

Locked locations:

- `garage`
- `back_garden`

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

Current command surface:

- `use house_keys`
- `unlock <location>`
- `open <location>`

Valid use context:

- player is near a reachable matching gate
- player has `house_keys` in inventory
- the target gate is still locked

Current reducer result:

- the targeted reachable location becomes unlocked
- state change is durable
- command succeeds even though the player does not move yet

Current player-facing behavior includes:

- explicit lock rejection lines on blocked movement
- explicit targeting when more than one gate matches
- successful unlock lines such as `You unlock Garage with house keys.`

### Current Invalid Use Cases

If the player uses `house_keys`:

- in the wrong location
- before picking them up
- after the relevant gate is already unlocked
- when more than one valid nearby gate matches and the target is ambiguous

the reducer should reject the action clearly.

## Objective Relationship

The current branch now uses the lock route together with mixed objective conditions:

- hold `house_keys`
- reach `garage`
- defeat `brute_in_garage`

That follow-up objective work is implemented separately in:

- [docs/V0_2_OBJECTIVE_CONDITIONS_SPEC.md](docs/V0_2_OBJECTIVE_CONDITIONS_SPEC.md)

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
- whether a formerly locked gate is broken open rather than normally unlocked

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
- broken locked gates should show a distinct `broken` state
- map tile should visually distinguish locked vs unlocked garage
- diagnostics should include lock-state truth explicitly

The player should not have to infer the lock purely from failed movement prose.

## Save/Load

Lock state must round-trip through save/load.

Required persistence behavior:

- a locked garage stays locked after save/load
- an unlocked garage stays unlocked after save/load
- a broken-open gate stays broken after save/load

## Current Verified Behaviors

Current automated coverage includes:

- generated run starts with `garage` and `back_garden` locked
- moving to a locked target fails cleanly
- taking `house_keys` then unlocking a reachable target succeeds
- spawned enemies can break a locked gate into a passable broken-open state
- ambiguous `use house_keys` prompts explicit targeting
- unlocked gate state survives save/load

## Explicit Non-Goals

Do not include these in the first locked-progression pass:

- lockpicking
- consumable keys
- multiple locked rooms
- key durability
- branching gate logic
- new objective types
- broad natural-language unlock parsing

## Current Role In `v0.2`

This mechanic is no longer the active planning frontier.

It is now a completed foundation that supports the next siege-depth work, especially:

- barricade-driven room security
- clearer route pressure
- stronger UI truth surfacing around secured versus exposed approaches
