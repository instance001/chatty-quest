# `v0.2` Objective Conditions Spec

## Purpose

This document records the exact shape of the second `v0.2` expansion.

Status note:

- refreshed on `2026-07-16`
- mixed objective conditions are implemented on the current branch

The goal is not to invent a general quest language.

The goal is to prove that objective completion can be driven by more than one deterministic condition family while keeping reducer truth simple.

## Current Implemented Expansion

Add optional item-possession support to objectives.

New objective field:

- `required_item_id`

This document now sits alongside the follow-up location requirement extension:

- [docs/V0_2_LOCATION_OBJECTIVE_SPEC.md](docs/V0_2_LOCATION_OBJECTIVE_SPEC.md)

## Why This Condition First

This is the best next condition because:

- it is the smallest honest extension beyond boss-kill logic
- it fits the newly added gated-progression route cleanly
- it proves objectives can depend on inventory truth, not just combat truth
- it does not require route scripting, timers, or scenario-specific hacks

## Exact Objective Semantics

### Supported Condition Fields

`v0.2` objective expansion keeps the existing field:

- `target_boss_id`

and adds optional fields:

- `required_item_id`
- `required_location_id`

### Completion Rule

Objective completion uses `AND` semantics across all populated condition fields.

That means:

- if only `target_boss_id` is present, the boss must be defeated
- if only `required_item_id` is present, the item must be in inventory
- if only `required_location_id` is present, the player must be at that location
- if multiple fields are present, all of them must be true

This keeps the rule deterministic and easy to inspect in code, UI, save files, and diagnostics.

### Explicit Non-Semantics

Do not add in this pass:

- `OR` conditions
- nested condition groups
- ordered quest stages
- counters
- timers
- scenario scripting callbacks

## Datapack Shape

Recommended objective template shape:

```toml
[[objectives]]
id = "secure_property"
name = "Secure The Property"
description = "Hold the property together long enough to reach the garage, keep the house keys, and kill the Garage Brute."
tags = ["primary", "v0_2"]
target_boss_id = "brute_in_garage"
required_item_id = "house_keys"
required_location_id = "garage"
```

Schema rules:

- at least one completion field must be populated
- `target_boss_id` must reference a known boss when present
- `required_item_id` must reference a known item when present
- `required_location_id` must reference a known location when present
- blank strings must be rejected as invalid content

## Runtime State

The reducer should not need a separate objective-condition runtime structure yet.

Minimum required truth:

- `ObjectiveState` stores the copied condition fields needed for completion checks
- inventory already provides the truth source for `required_item_id`
- boss defeat state already provides the truth source for `target_boss_id`
- current location already provides the truth source for `required_location_id`

This keeps `v0.2` narrow while still moving objective logic out of the single hardcoded assumption.

## Reducer Behavior

Objective completion should be recomputed after every action exactly as it is today.

What changes:

- completion no longer means only `bosses_defeated.contains(target_boss_id)`
- completion now evaluates every populated deterministic condition field

No hidden progression is allowed:

- picking up the required item should update objective truth immediately
- consuming or losing the required item later should make the objective incomplete again unless design later freezes completion on first success

For `v0.2`, objective completion should remain live truth:

- current state decides whether the objective is complete

## Current Scenario Use

`Property Siege Classic` currently uses all three supported deterministic condition fields in the primary objective.

Current content use:

- require `house_keys`
- require `garage`
- require defeat of `brute_in_garage`

Why this specific pairing:

- it matches the new gated route
- it prevents the objective model from becoming broader without being used
- it proves the objective system can combine inventory and combat truth without adding a second scenario

Current garage-finale reading:

- the objective destination is not just a box to tick
- the `garage` now acts as a readable finale room with a live boss-space identity
- once `brute_in_garage` is pushed into its wounded end state, the room and combat text both surface that the finish has become more dangerous
- if both exposed siege lanes are barricaded before the fight, the garage finale also surfaces a small secured-property payoff through reduced brute retaliation

Those wrinkles do not change objective truth semantics.

They make the final condition feel more authored than a flat last HP exchange.

## UI Truth Surfacing

If objective logic gains multiple condition families, the shell should stop implying the objective is only boss-based.

Required `v0.2` first-pass truth surfacing:

- objective summary can still show one primary line
- character or diagnostics surfaces should show the active objective conditions explicitly
- diagnostics should make it obvious whether the required item is currently held
- diagnostics and character surfaces should make it obvious whether the required location is currently reached

Do not rely on prose alone to explain objective state.

## Save/Load

No special persistence model is needed beyond the existing inventory and boss-defeat state.

Required behavior:

- objective completion still round-trips correctly after save/load
- item-backed objective truth recalculates correctly from restored inventory

## Current Verified Behaviors

Current automated coverage includes:

- datapack validation rejects missing or unknown completion references
- generated run copies `required_item_id` and `required_location_id`
- item-only, location-only, and mixed-condition objective cases evaluate correctly
- save/load preserves mixed-condition completion truth

## Explicit Non-Goals

Do not include these in the first objective-condition pass:

- secondary quest logs
- branching objectives
- hidden objectives
- ordered multi-step quest chains
- fail-state objectives
- generic rules engine abstractions

## Current Role In `v0.2`

This system is now a completed foundation.

The remaining work is not to re-prove objective conditions exist. It is to use them more clearly only when a concrete scenario need appears:

- richer scenario content that benefits from the existing condition model
- small finale-authorship beats that make the required destination and boss condition feel intentional
- future condition families only when item, location, and boss truth are no longer enough
