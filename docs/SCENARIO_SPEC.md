# Scenario Spec

## Purpose

A scenario is the rules-and-structure layer that turns templates into a specific playable adventure.

The engine should support multiple scenario identities over time, but `v0.1` only needs to prove one complete example:

- `Property Siege Classic`

The scenario layer exists to keep world-specific rules in data and design docs rather than hardcoding them as universal engine behavior.

## Scenario Identity

A scenario defines:

- the playable space
- the starting conditions
- the main goal
- scenario-specific restrictions
- the tone hooks that shape presentation

The scenario is where the engine stops being a generic content loader and becomes a specific adventure.

## Scenario Authority

A scenario may define:

- what locations are part of the playable map
- which transitions are allowed
- what counts as leaving the scenario boundary
- whether locations can be revisited
- what conditions unlock progress
- what counts as victory
- what counts as failure

A scenario may not bypass the deterministic reducer or narrator boundary rules.

The scenario config defines legal possibilities. The engine still owns truth enforcement.

## Boundaries And Failure Rules

Boundaries must be scenario-defined.

For `Property Siege Classic`, the player is trapped on the property during a zombie apocalypse. Attempting to leave the property should be blocked or turned into scenario-appropriate failure handling according to the rules of that scenario.

Important principle:

The engine should not globally assume that all scenarios forbid leaving the current local map.

Instead, the scenario should define:

- what counts as inside the playable space
- what exits are unavailable
- what message or consequence is associated with boundary attempts
- whether a blocked attempt is non-fatal, fatal, or context-dependent

This keeps the engine reusable for future scenario families.

## Starting State

A scenario should define or influence the starting run state.

That may include:

- starting location
- starting HP or stat baseline
- starting inventory
- initially known locations
- initially locked locations
- starting objective state
- seeded or fixed placement rules for enemies, bosses, and items

For `v0.1`, the starting state should be deterministic enough to test and restore reliably.

## Map Topology

The scenario should define the playable topology of the map.

For `v0.1`, the most practical interpretation is:

- a small authored or semi-authored property map
- deterministic room connections
- scenario-controlled boundary edges

Potential future scenarios may use more dynamic generation, but `v0.1` does not need fully procedural map topology to prove the engine shape.

## Objective Selection Or Generation

A scenario should define how a run receives its objective.

For `v0.1`, acceptable models include:

- choosing one objective template from a small pool
- selecting one scenario-compatible boss and binding the objective to it
- choosing a simple fixed objective for the first version

The key rule is that once the run begins, the active objective becomes frozen canonical state.

The narrator may describe or dramatize the objective, but it does not invent the objective mid-run.

## Enemy And Boss Placement

A scenario should control how enemies and bosses are assigned into the map.

For `v0.1`, that may be:

- fixed placements
- seeded placements from a constrained pool
- a hybrid of fixed boss placement and variable enemy placement

The main requirement is that encounter placement should be deterministic once the run is created.

## Return Policies

Some scenarios may allow free return between locations.
Some may prevent re-entry after certain events.

The scenario layer should support concepts such as:

- can return
- cannot return after exit
- conditionally blocked return

`Property Siege Classic` may use simple return rules in `v0.1`, but the model should leave room for no-return spaces later.

## Locking Rules

Scenarios should be able to define:

- which locations begin locked
- what items or conditions unlock them
- whether locks are permanent, conditional, or one-time

This allows scenario progression to remain in data instead of being hidden in hardcoded scripts.

## Tone Inputs

Scenario tone and narrator tone are related but distinct.

A scenario may supply:

- world tone capsule text
- atmosphere prompts
- preferred style hints
- scenario-specific vocabulary or flavor

These shape presentation only.

They should not override the deterministic state model or insert hidden mechanical rules.

## `Property Siege Classic` Example Reading

`Property Siege Classic` should be understood as:

- a bounded survival scenario
- a property-scale map
- a zombie-themed deterministic adventure
- a strong fit for dark humour or hostile narrator styles
- a small but complete proof of the engine

It should not be treated as proof that all future scenarios must use:

- zombies
- domestic property maps
- survival horror tone
- fixed local confinement

## Scenario Invariants

The following scenario-level invariants should remain true:

- the scenario defines the playable boundary
- the active objective is canonical once the run begins
- the player starts in a valid scenario location
- progression gates are reducer-enforced
- scenario flavour does not override state truth

These invariants help preserve the engine’s reuse value across future datapacks.

## `v0.1` Guidance

For `v0.1`, the scenario layer should stay:

- explicit
- small
- testable
- data-driven where it matters
- expressive enough to feel like a real adventure

The scenario spec is not trying to solve every future campaign format. It is trying to define one complete bounded scenario cleanly enough that future scenarios can extend the same shape.
