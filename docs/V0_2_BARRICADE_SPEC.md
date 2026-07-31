# `v0.2` Barricade Spec

## Purpose

This document defines the next honest deterministic mechanic for `Property Siege Classic`:

- barricading vulnerable parts of the property

The goal is not to build a full base-building system.

The goal is to make the property feel under siege rather than merely locked, while preserving the existing RD Engine rule:

- canonical state changes happen through explicit reducer-owned truth

Status note:

- refreshed on `2026-07-16`
- first-pass barricades are implemented on the current branch
- current barricadable targets are `front_verandah` and `back_garden`
- the two current barricade targets now have intentionally different payoffs

## Why Barricades First

Barricades are the best next mechanic for the current zombie pack because they:

- fit the scenario fantasy immediately
- deepen the existing house-scale map without requiring more rooms
- create a clear use for an optional route such as `back_garden`
- are easy to express in datapack content and UI truth surfaces
- do not require enemy AI simulation or a generalized crafting system

This keeps `v0.2` narrow while still making the scenario feel more like a siege.

## Intended Experience Change

Current route shape is broadly:

- start at `front_verandah`
- scavenge through `kitchen` and `laundry`
- get `house_keys`
- unlock a gate
- reach `garage`
- kill the brute

Barricades should add one more meaningful deterministic layer:

- secure part of the property before the worst pressure arrives

This does not replace the existing route.

It makes the route more legible and more alive:

- scavenge
- decide what to secure
- reduce or redirect risk
- continue toward the brute

## First-Pass Mechanic

The first barricade pass should stay small and explicit.

Recommended shape:

- one barricade-capable item or room action
- one or two barricade targets only
- explicit barricaded / unbarricaded truth
- visible gameplay effect when a target is barricaded

Avoid in the first pass:

- freeform crafting
- resource stacks
- multi-hit barricade durability
- repair loops
- turn-by-turn zombie breaching simulation
- generic construction systems

## Scenario Reading

For `Property Siege Classic`, the best first barricade reading is:

- the player can temporarily secure the most vulnerable approach
- the secured state matters to the run in a deterministic, inspectable way
- the mechanic supports the fantasy of holding the property together

The scenario should still remain:

- bounded
- readable
- small enough to test and save reliably

## Recommended Barricade Targets

Current barricade targets:

- `front_verandah`
- `back_garden`

Practical reading:

- `front_verandah` is the obvious exposed threshold
- `back_garden` becomes a more meaningful branch if it can be secured or left risky
- `front_verandah` is now the threshold-defense lane
- `back_garden` is now the risky flank lane

The `garage` remains the boss destination rather than a barricade target.

That keeps the mechanic from overlapping too heavily with the existing lock/unlock progression.

## Recommended First Barricade Tool

Recommended first content shape:

- add a dedicated barricade item such as `wood_planks`, `nails_and_hammer`, or `barricade_kit`

Why this is preferable to overloading an existing weapon:

- the action becomes clearer in UI and reducer output
- the datapack stays more expressive
- the mechanic does not blur combat gear with fortification truth

Alternative acceptable shape:

- use a location action backed by a required item tag

The dedicated-item route is still the cleanest first implementation.

## Datapack Shape

The first barricade pass should remain data-driven and explicit.

Recommended additions:

### Item Template Extension

Allow an item to declare:

- `utility_effect = "barricade"`

This should mean:

- the item can be used for legal barricade transitions
- the reducer still decides where it applies

### Location Template Extension

Allow a location to declare optional barricade metadata such as:

- `barricadable = true`
- `barricade_item_id = "barricade_kit"`
- `barricade_response = "You hammer together a rough barricade. It will not hold forever, but it will do."`
- `already_barricaded_response = "That approach is already barricaded."`

This keeps barricade rules attached to scenario content instead of hiding them in code.

## Validation Rules

If barricade metadata is introduced, validation should enforce:

- barricadable locations must not reference a blank `barricade_item_id`
- `barricade_item_id` must reference a known item when present
- barricade response text must not be blank when present
- non-barricadable locations should not silently carry barricade-only fields

The first pass should prefer explicit validation over permissive guessing.

## Runtime State

`RunState` needs a deterministic barricade-state family.

Recommended minimum truth:

- a set of barricaded location ids

Suggested shape:

- `barricaded_locations: HashSet<String>`

This mirrors the existing lock-state model and stays easy to serialize, inspect, and surface in diagnostics.

## Reducer Behavior

### Action Surface

Recommended first-pass command:

- `barricade <location>`

Acceptable parser aliases:

- `fortify <location>`
- `secure <location>`

The reducer should continue to own legality and outcome text.

### Legal Barricade Conditions

A barricade action should succeed only if:

- the target location is part of the scenario
- the target is currently barricadable
- the player has the required barricade item
- the player is in the target location or in a directly connected location, depending on the authored scenario rule
- the target is not already barricaded

### Suggested First-Pass Context Rule

To keep the pack readable, the player should be able to barricade a target when:

- standing in that location

This is stricter than gate unlocking and easier to explain.

If later content needs adjacent barricading, that can be a future extension.

### Successful Result

On success:

- the location id is added to `barricaded_locations`
- the reducer emits a success line
- the change persists through save/load

Suggested player-facing line:

- `You barricade the Front Verandah.`

### Failure Cases

Suggested clean failure cases:

- `That location cannot be barricaded.`
- `You do not have the right materials.`
- `You need to be there to barricade it.`
- `The Front Verandah is already barricaded.`

## Current Mechanical Effect

The barricade must do something visible and deterministic.

Current effect:

- a barricaded location suppresses passive pressure from its authored resident threat while leaving deliberate combat intact

For the current pack:

- barricading `front_verandah` suppresses passive pressure from `shambler_front_gate`
- barricading `back_garden` suppresses passive pressure from `crawler_in_weeds`
- barricading `front_verandah` also blocks direct retaliation from the `Front Gate Shambler`
- barricading `front_verandah` also grants a small attack bonus during the threshold fight
- barricading `back_garden` also grants a one-time recovery payoff through `barricade_heal`
- barricading both authored siege lanes before entering the `garage` reduces `Garage Brute` retaliation by `1`

Possible exact implementations:

- prevent that encounter from damaging the player on `wait`
- mark the encounter as blocked until the player attacks or enters directly
- remove a scenario-authored pressure penalty tied to that location

The best first pass is the smallest one that is easy to inspect in state and easy to explain in tests.

## Current Scenario Effect

This gives the mechanic a real payoff without requiring dynamic enemy movement or breach simulation.

Current room-role split:

- `front_verandah` rewards securing the obvious threshold before direct confrontation
- `back_garden` rewards taking flank risk and then stabilizing with a small recovery bonus

Current practical reading:

- secure `front_verandah` if you want safer direct combat at the entry threshold plus a better attack angle
- secure `back_garden` if you want a risky side route that gives a little breathing room back
- secure both if you want the final garage fight to feel less like the entire property is joining the boss

## UI Truth Surfacing

The barricade state should be visible outside narration.

Required surfaces:

- current location details
- map panel
- diagnostics

Recommended presentation:

- show `Barricade state: Barricaded` or `Unbarricaded` on barricadable locations
- visually mark barricaded map nodes
- surface the required barricade item when relevant

The player should not have to infer barricade truth only from one success line in the log.

## Objective Relationship

The first barricade pass does not need to rewrite the primary objective.

The existing objective may remain:

- keep `house_keys`
- reach `garage`
- defeat `brute_in_garage`

Barricades should improve route quality and siege feel first.

Possible later extension:

- add a secondary or expanded objective condition such as securing a named approach before entering the garage

That should not be required in the first barricade pass.

## Save/Load

Barricade state must round-trip through save/load.

Required behavior:

- a barricaded location stays barricaded after restore
- an unbarricaded location stays unbarricaded after restore

No prose reconstruction should be required.

## Test Coverage

Minimum automated coverage:

- datapack validation rejects unknown `barricade_item_id`
- generated run starts with no barricaded locations unless content explicitly says otherwise
- barricading a valid target with the right item succeeds
- barricading without the required item fails cleanly
- barricading a non-barricadable target fails cleanly
- barricading an already barricaded target fails cleanly
- save/load preserves barricade state
- UI-derived or diagnostics-facing data shows barricade truth
- spawned enemies can attack a blocking barricade with a small deterministic chance to destroy it

## Explicit Non-Goals

Do not include these in the first barricade pass:

- fully simulated zombie pathfinding
- dynamic breach countdowns
- multi-hit barricade HP
- repairable damage states
- material collection economy
- procedural fortification placement
- generalized builder gameplay

## Recommended Implementation Order

1. extend datapack schema for barricadable locations and barricade-capable items
2. extend `RunState` with `barricaded_locations`
3. add reducer and parser support for `barricade <location>`
4. update `Property Siege Classic` content with one real barricade route
5. surface barricade truth in UI and diagnostics
6. add reducer, save/load, and validation tests
7. update milestone and manual-sweep docs once stable

## Current Role In `v0.2`

Barricades are now a completed first-pass foundation rather than the active unknown.

Current branch reality:

- two authored siege lanes exist
- both lanes have distinct local payoffs
- spawned enemies can now break a barricade in one deterministic hazard-attack event
- securing both lanes has a small deterministic garage-finale payoff
- UI, diagnostics, save/load, and reducer coverage all surface or preserve barricade truth

The next barricade work should wait for a concrete content need, such as durability, repair, or additional scenario packs proving that two-state secured/unsecured truth is too small.
