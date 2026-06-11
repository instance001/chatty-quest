# Modding Guide

## Purpose

This document is the beginning of the modder-facing guide for Chatty Quest and the RD Engine.

It is intentionally small for now.

The goal is to give contributors and future datapack authors one stable place to find the current practical rules without pretending the toolchain is more finished than it is.

## Current Scope

Right now, the most stable modding surface is:

- datapack folder structure
- template ids and references
- media folder conventions
- media naming and fallback behavior

More complete authoring guidance can be added later once the engine surface stops moving so quickly.

## Datapack Shape

Current datapacks live under:

```text
assets/datapacks/<pack_name>/
```

The first example pack is:

```text
assets/datapacks/property_siege_classic/
```

Useful files and folders:

- `pack.toml`
- `rules.toml`
- `templates/`
- `media/`
- `capsules/`

For the deeper structure, see [DATAPACK_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/DATAPACK_SPEC.md:1).

## Template Id Rule

Template ids should stay:

- lowercase
- snake_case
- stable once referenced elsewhere

These ids are used in:

- template cross-references
- runtime state
- save/load continuity
- media naming

## Media Folders

Current recommended pack-local media structure:

```text
media/
  images/
    locations/
    items/
    enemies/
    bosses/
    overrides/
    fallbacks/
  audio/
    ambience/
    items/
    enemies/
    bosses/
    overrides/
  video/
    locations/
    items/
    enemies/
    bosses/
    overrides/
```

## Media Naming

Single-focus media should use the template id directly.

Examples:

- `kitchen.png`
- `medkit.png`
- `garage_brute.png`

Dual-context override media should use:

```text
focus_id__location_id.ext
```

Important rule:

- first = current focused entity or item
- second = current location
- never reverse the order
- never author both permutations for the same pairing

Examples:

- `shambler_front_gate__front_verandah.png`
- `cricket_bat__kitchen.png`
- `brute_in_garage__garage.png`

This keeps override naming deterministic and prevents duplicate authoring.

## Media Resolution

The current engine behavior is:

For movement, idle, and location-forward beats:

1. dual-context override
2. current location media
3. datapack placeholder
4. engine placeholder

For inspect, item, and combat-forward beats:

1. dual-context override
2. focused item or entity media
3. current location media
4. datapack placeholder
5. engine placeholder

This keeps media expressive without turning it into a second hidden state machine.

## Placeholder Rule

Current datapack image fallback candidates are:

- `media/images/fallbacks/placeholder.png`
- `media/images/fallbacks/scenario_default.png`

If neither exists, the engine falls back to its own built-in placeholder state.

## Worked Example

Here is a small practical example showing how a location, an item, and a dual-context override fit together.

Example location template:

```toml
[[locations]]
id = "kitchen"
name = "Kitchen"
description = "A cramped kitchen with too many drawers and not enough comfort."
narrator_brief = "Keep the kitchen tense and domestic at the same time."
tags = ["indoor", "loot_possible"]

[locations.media]
image = "media/images/locations/kitchen.png"
audio = "media/audio/ambience/kitchen_hum.ogg"
display_role = "location"
```

Example item template:

```toml
[[items]]
id = "cricket_bat"
name = "Battered Cricket Bat"
description = "It is not elegant, but it absolutely counts."
narrator_brief = "Play the bat as a practical suburban weapon."
tags = ["weapon", "starter_item"]

[items.media]
image = "media/images/items/cricket_bat.png"
audio = "media/audio/items/cricket_bat_thunk.ogg"
display_role = "item"
```

Matching files:

```text
media/images/locations/kitchen.png
media/audio/ambience/kitchen_hum.ogg
media/images/items/cricket_bat.png
media/audio/items/cricket_bat_thunk.ogg
media/images/overrides/cricket_bat__kitchen.png
media/images/fallbacks/placeholder.png
```

What the engine does:

1. If the player focuses the bat while in the kitchen, the engine first tries `media/images/overrides/cricket_bat__kitchen.png`.
2. If that override is missing, it falls back to `media/images/items/cricket_bat.png`.
3. If the item image is also missing, it falls back to `media/images/locations/kitchen.png`.
4. If the location image is missing too, it falls back to `media/images/fallbacks/placeholder.png`.
5. If the datapack placeholder does not exist, the engine falls back to its built-in placeholder state.

That means modders can start with only location art, then add item art, then add dual-context overrides only where they want extra specificity.

Second example for enemy pressure:

Example location template:

```toml
[[locations]]
id = "front_verandah"
name = "Front Verandah"
description = "The front entry point of the property, boxed in by bad luck and worse noise."
narrator_brief = "Play this as the bad threshold of the run."
tags = ["outdoor", "entry", "starting_location"]

[locations.media]
image = "media/images/locations/front_verandah.png"
audio = "media/audio/ambience/front_verandah_wind.ogg"
display_role = "location"

enemies = ["shambler_front_gate"]
```

Example enemy template:

```toml
[[enemies]]
id = "shambler_front_gate"
name = "Front Gate Shambler"
description = "A corpse with terrible posture and strong opinions about your fence line."
narrator_brief = "Play this one as stubborn, lurching, and annoyingly symbolic."
tags = ["zombie", "melee"]

[enemies.media]
image = "media/images/enemies/front_gate_shambler.png"
audio = "media/audio/enemies/front_gate_shambler_groan.ogg"
display_role = "enemy"
```

Possible matching files:

```text
media/images/locations/front_verandah.png
media/audio/ambience/front_verandah_wind.ogg
media/images/enemies/front_gate_shambler.png
media/audio/enemies/front_gate_shambler_groan.ogg
media/images/overrides/shambler_front_gate__front_verandah.png
media/images/fallbacks/placeholder.png
```

What the engine does:

1. On a location-forward beat like entering or waiting at `front_verandah`, the engine can try `media/images/overrides/shambler_front_gate__front_verandah.png` first if that live threat is the surfaced context.
2. If that override is missing, location-forward resolution falls back to `media/images/locations/front_verandah.png`.
3. If the location image is missing too, it falls back to `media/images/fallbacks/placeholder.png`.
4. On a threat-forward beat like inspecting or attacking the shambler, the engine still tries the override first.
5. If the override is missing on that threat-forward beat, it falls back to the focused threat image `media/images/enemies/front_gate_shambler.png`.
6. If the enemy image is missing too, it falls back to the location image, then the datapack placeholder, then the engine placeholder state.

This is the practical difference between the two lanes:

- location-forward beats prefer place after override
- threat-forward beats prefer the focused enemy after override

## Practical Advice

- Start with a few single-focus images first.
- Add one or two dual-context overrides only where they matter.
- Let the fallback ladder do the rest.
- Do not wait until every media slot is authored before testing a datapack.

## Status

This guide is an initial start point, not a finished manual.

Future versions should grow to cover:

- template authoring examples
- validation troubleshooting
- save compatibility notes
- capsule writing guidance
- scenario packaging
- future diagnostic workflows
