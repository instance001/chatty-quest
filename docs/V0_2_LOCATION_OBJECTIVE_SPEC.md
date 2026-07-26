# `v0.2` Location Objective Spec

## Purpose

Add a third deterministic objective condition family:

- `required_location_id`

This lets objectives demand that the player be at a named location when completion is evaluated.

Status note:

- this mechanic is implemented on the current branch
- `Property Siege Classic` requires the player to reach `garage` as part of the primary objective
- reducer, diagnostics, UI, and save/load coverage all treat the location requirement as structured truth

## Why This Shape

This is the smallest useful expansion after:

- `target_boss_id`
- `required_item_id`

Reasons:

- it reuses existing truth in `RunState.current_location_id`
- it broadens objective composition without introducing a new subsystem
- it matches the current scenario language of "reach the garage"

## Datapack Schema

Objective templates may now define:

- `target_boss_id`
- `required_item_id`
- `required_location_id`

At least one must be present.

Example:

```toml
[[objectives]]
id = "secure_property"
name = "Secure The Property"
description = "Hold the property together long enough to keep the house keys, reach the garage, and kill the Garage Brute."
tags = ["primary", "v0_2"]
target_boss_id = "brute_in_garage"
required_item_id = "house_keys"
required_location_id = "garage"
```

## Validation Rules

- `required_location_id` must not be blank when present
- `required_location_id` must reference a known location
- objectives still need at least one completion condition across all supported fields

## Runtime Shape

`ObjectiveState` now carries:

- `target_boss_id`
- `required_item_id`
- `required_location_id`

No new top-level run field is needed because location truth already exists in:

- `RunState.current_location_id`

## Completion Semantics

Objective completion is an `AND` across all populated objective condition fields.

That means:

- if only `required_location_id` is present, the player must reach that location
- if multiple fields are present, all of them must be true at once

For the current scenario:

- hold `house_keys`
- be in `garage`
- defeat `brute_in_garage`

## Reducer And UI Surfacing

The reducer should:

- evaluate location condition truth after each action
- emit objective progress lines when the location condition flips

The UI and diagnostics should:

- show whether the required location is currently reached
- preserve the same truth-first presentation style used for item and boss conditions

## Test Coverage

Add coverage for:

- datapack validation rejecting unknown `required_location_id`
- generated run copying `required_location_id`
- save/load preserving `required_location_id`
- objective completion with a location-only requirement
- combined objective completion where location is the remaining unmet condition

## Current Role In `v0.2`

Location objective conditions are now a completed first-pass foundation.

The next objective work should focus on better use of existing condition families before adding branching, ordered, or scripted objective logic.
