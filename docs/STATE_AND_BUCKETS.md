# State And Buckets

## Why Structured State Exists

Chatty Quest uses structured state so the game remains authoritative, deterministic, and testable.

The chat log is presentation.
The narrator is presentation.
The rolling summary is support memory.

Canonical truth lives in structured runtime state.

Without this rule, the project would be unable to guarantee:

- reliable saves
- predictable combat
- valid inventory
- trustworthy map transitions
- scenario rule enforcement

## Bucket Philosophy

Buckets are a practical way to represent the changing status of entities and world facts during a run.

A bucket answers questions like:

- has this location been visited
- is this location locked
- is this item in the world or in the inventory
- is this enemy alive or defeated
- is this NPC neutral, hostile, or dead

Templates define what can exist.
Buckets define what currently applies.

Longer-term, the model should stay conceptually distinct:

- template = blueprint
- instance = this spawned copy
- bucket = current status, location, or container

## Required `v0.1` State

At minimum, `v0.1` runtime state should track:

- current location
- known locations
- visited locations
- locked and unlocked locations
- no-return locations, if the scenario uses them
- inventory
- equipped item
- consumed or destroyed items
- player HP
- enemy alive or dead state
- boss alive or dead state
- objective progress
- rolling summary support
- scenario boundary-relevant state

This set is large enough to support a real playable loop while remaining small enough to reason about.

## Suggested `v0.1` Bucket Families

Possible bucket families include:

- `locations_known`
- `locations_visited`
- `locations_locked`
- `locations_no_return`
- `items_world`
- `items_inventory`
- `items_equipped`
- `items_consumed`
- `items_destroyed`
- `enemies_alive`
- `enemies_defeated`
- `boss_alive`
- `boss_defeated`
- `objectives_active`
- `objectives_completed`

The exact Rust data structures may vary, but the conceptual separation should remain visible.

For `v0.1`, some of these may still be represented with relatively direct fields or collections instead of a fully generalized bucket engine. That is acceptable as long as the truth model stays explicit and extensible.

## Optional Future State

Future scenarios may need more detailed state such as:

- NPC affinity buckets
- NPC promised rewards
- quest reward offers
- faction reputation
- noise or alertness
- status effects
- skill progression
- resource scarcity models
- temporary world conditions

Some future systems will also benefit from summary-supported continuity, but the rule should remain:

- mechanically relevant promises, offers, completions, and flags belong in structured state
- flavour-only memory may remain in rolling summary

These should be treated as future expansion areas, not required `v0.1` complexity.

## Rolling Summary Versus Truth

Rolling summary is useful, but it is not canonical state.

Rolling summary may store:

- important story beats
- memorable recent events
- recap material for the player
- continuity support for a future narrator model

Rolling summary must not be the source of truth for:

- current location
- inventory contents
- HP totals
- lock state
- objective completion
- enemy death state

If a fact matters mechanically, it belongs in structured state first.

Practical rule:

- if it affects future mechanics, bucket it
- if it only affects future tone or recognition, summarize it

## Pending Changes And Confirmed Changes

Not every narrated possibility should automatically become fact.

The architecture should preserve the possibility of:

- immediate deterministic mutations
- pending state changes
- confirmation-gated actions where useful

For `v0.1`, many actions may resolve immediately for simplicity.

Even so, the model should remain conceptually clear:

- the player expresses intent
- the engine interprets the action
- the reducer validates legality
- the engine commits or rejects the mutation
- the narrator describes the outcome

This protects the game from narration-driven drift.

## Example State Shape

An implementation-friendly mental model for runtime state is:

- scenario identity
- generation seed or scenario setup metadata
- player state
- location state
- entity state
- reward or contract state where relevant
- inventory and equipment state
- objective state
- summary or log support state

The exact structs can be refined during implementation, but these concerns should remain separate.

## Invariants

The following invariants should hold throughout play:

- the player is in exactly one valid location
- an item cannot be both equipped and still in the world
- a defeated enemy cannot also be alive
- a locked location cannot be entered unless scenario rules allow it
- objective completion must reflect actual reducer-confirmed state
- narration cannot override structured state
- media cannot imply a canonical change that state does not confirm
- summary cannot be the only source of a mechanically relevant promise or reward

If any proposed implementation weakens these invariants, it should be treated as a design issue, not just a coding detail.

## `v0.1` Guidance

For `v0.1`, state and buckets should stay:

- explicit
- serializable
- easy to inspect
- easy to validate
- easy to test

The goal is not to create the most abstract or future-complete state system immediately. The goal is to create a reliable truth model that can support one playable scenario now and broader scenario families later.
