# Modding Guide

Last updated: `2026-09-29`

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
- narrator-facing presentation briefs and bounded proposal compatibility

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

For the deeper structure, see [DATAPACK_SPEC.md](DATAPACK_SPEC.md).

For a code-backed inventory of every currently implemented template field, tag, and mechanic, see
[TEMPLATE_CAPABILITY_REFERENCE.md](TEMPLATE_CAPABILITY_REFERENCE.md). Start there when deciding
what a new item, location, enemy, boss, or objective can actually do.

## Action Timing

Timing is authored mechanics, not a property of whether an entity is player- or AI-controlled.
Actor templates may set positive integer `traversal_cost` and native `attack_cost`; enemies and
bosses expose the same fields, equipped items may set basic `attack_cost`, and abilities define
their own `action_cost`. Omitted costs default to `1` for existing packs. The reducer owns the
shared world-time/readiness commit, while ability cooldown remains a separate per-grant rule.
See [ACTION_TIMING_SPEC.md](ACTION_TIMING_SPEC.md) for examples and the complete invariant.
An ability `effect.kind = "move"` is an ability-shaped request to the same actor-movement reducer,
not an alternate teleport state path: its cast target is the destination and ordinary movement
capability, adjacency, and lock checks remain authoritative.
Ability-created status effects can additionally declare `timing_action_kinds`, an additive
`timing_cost_delta`, or `blocked_action_kinds`; those apply through the same readiness resolver
and never require a special haste/slow/root implementation.

## Narrator Boundary

Datapacks supply authored templates, descriptions, tags, and optional `narrator_brief` presentation
guidance. They do not configure a provider, model filename, endpoint, or API key. Those are
player-owned runtime choices made in Setup.

Datapacks are declarative content, not code extensions. The host owns health, combat, perception,
movement, spawning, and every other mechanical reducer path. A template may select an exposed
host capability and configure its validated parameters, but a pack may not add private mechanics,
mutate state, or ask the narrator to simulate a missing system in prose.

The narrator is never a source of canonical state. It can present reducer-confirmed events and may
fill a host-owned, bounded proposal form only when the player's proposal controls permit it. The
host validates the form, the player can edit and approve or reject it, and a reducer is the only
component that can commit the result. Do not make gameplay depend on an LLM remembering hidden facts
or producing a particular phrase.

### Writing Narrator Briefs

Keep `narrator_brief` short, presentational, and grounded in the template it accompanies. Good
briefs describe tone, emphasis, or point of view. They must not add secret rules, promises of
rewards, implicit route unlocks, or state changes. The structured template and reducer remain the
source of those facts.

### Proposal-Compatible Content

The runtime's generic proposal forms can cover bounded runtime items, statuses, NPCs, enemies,
bosses, objectives, and temporary exits when the selected play contract authorizes them. A form is
not a new datapack template and does not rewrite authored content. Mods should therefore continue to
provide complete authored locations, ids, tags, routes, and validation-friendly references. In
particular, a temporary exit may only reference locations already known to the host; it cannot make
up a new location id.

Narrator controls belong to the player rather than a pack:

- narration-event checkboxes decide when narration is requested
- Tight, Guided, Loose, and Wild decide the host-owned proposal chance and authorized breadth
- provider credentials are runtime-only and never belong in a datapack or mod configuration

Proposal output uses small host-selected field forms rather than a model-authored internal JSON
object. Modders should not rely on a model reproducing a pack schema. Invalid or incomplete forms
are discarded with no state mutation.

## Authored NPCs And Shared Actor Capabilities

### Capability-Gated Dependent Schema

**A checkbox is not merely a boolean: it is the root of a host-owned mechanical contract.**

Capabilities are the roots of schema subtrees. A template, runtime proposal, or future mutation
only exposes fields that are applicable to enabled host capabilities. For example, `health`
enables the `actor.hp` branch and `combat` enables the `actor.damage` branch; without those
roots, their dependent fields are inapplicable and cannot affect runtime state. `noise_track` and
`sight_track` are likewise only applicable after their required perception and movement roots are
enabled.

For an explicit capability list, prerequisite roots must be selected as well as configured: a
true `actor.can_hear` field does not make `noise_track` legal without `hearing`, and retained HP
or movement fields do not preserve a mechanic after their root is switched off. Empty capability
lists are only a compatibility fallback for pre-registry authored content and saves.

At materialization, a non-empty list replaces inferred legacy capabilities rather than merging
with them. An actor explicitly authored with `capabilities = ["inventory"]` is therefore
inventory-only even when old scalar fields remain stored for compatibility.

The same rule applies to enemy and boss encounter instances: their selected capability envelope
is copied into runtime state when they are placed or noise-spawned. A health-only hostile can be
targeted but cannot retaliate; an actor without movement cannot take autonomous movement steps.

The host capability registry is the canonical source for capability ids, prerequisites, dependent
fields, defaults, and validation. The same registry drives proposal questionnaires, approval UI,
template validation, runtime canonicalisation, and reducer admission. Packs and LLMs may fill
permitted values, but they never define a questionnaire or add a private mechanical field.

When a future mutation enables a capability, the host must resolve every newly applicable required
field through a safe default, authored configuration, or the same bounded proposal form before the
reducer commits it. When it disables a capability, dependent mechanics stop applying; stale draft
values are not authoritative. Future systems such as trade, quest giving, following, and dialogue
must add one host capability subtree rather than noun-specific fields such as
`NPCQuestFields` or `WeaponCombatFields`.

An optional `templates/npcs.toml` file supports first-class authored NPCs. Place an NPC by adding
its id to a location's `npcs` list. NPCs use explicit `disposition` (`friendly`, `neutral`, or
`hostile`), `faction`, and `quest_objective_ids` fields; a quest reference must name an existing
objective. Friendly is a presentation and interaction role, not a hidden combat default.

NPC and item templates may opt into the shared `[...actor]` capability table. It can assign `hp`,
`damage`, `can_hear`, `can_see`, `can_move`, and `combat`. A hostile NPC with valid health, damage,
and `combat = true` participates in the reducer-owned combat path. A friendly combat-capable runtime
actor in the player's current location uses that same reducer path on a host `wait` tick; this applies
equally to an NPC and a world-item actor, and it is not narration-only. Actor-capable items use the
same host state and, when they opt into `inventory`, become genuine `TransferItem` inventory endpoints
rather than narrated containers. Authored NPC placement remains stationary unless a separately
materialized pool instance opts into movement behavior. A player may issue a bounded instruction such
as `Lira, go kitchen` to any active actor with the movement contract; the host resolves the existing
actor and authored destination, then the reducer validates and commits the move. See
[TEMPLATE_CAPABILITY_REFERENCE.md](TEMPLATE_CAPABILITY_REFERENCE.md) for the precise supported
behavior and validation rules.

### Social State Is Generic Actor Data

Disposition is a mutable, player-relative result of host-owned social state, not permanent NPC
identity. Runtime actors begin with their authored `friendly`, `neutral`, or `hostile` stance, then
the host records a signed relationship score when a reducer-confirmed social event occurs. The
current generic events are `Gift`, `Help`, `Insult`, `Threat`, `Betrayal`, `Theft`, and
`ReturnItem`. They are shared facts, so a gift is first a real `TransferItem`, then (only after the
move commits) a `Gift` social event; failed transfers have no social effect and narration may not
claim that they happened.

Personality is interpreted from descriptive tags through one host registry. Current recognized
archetype tags are `cheerful`, `dour`, `trusting`, `suspicious`, `stubborn`, and `volatile`.
They alter the same positive and negative relationship thresholds used by every social consumer.
Tags remain data and may be changed by a future validated mutation; packs must not encode bespoke
friendship, haggling, or vendetta arithmetic in an NPC-specific field. This is deliberately a
small foundation, not an authoring promise for trade or quest systems that do not exist yet.

### Ability Templates Are Immutable; Grants Carry Possession

Magic, skills, training, scrolls, staffs, and temporary powers use the same template/grant split.
An ability template is shared authored data; a runtime grant records its owner, finite uses (or
infinite use), expiry, and provenance. See [ABILITY_GRANTS_SPEC.md](ABILITY_GRANTS_SPEC.md).
Access schools are descriptive tags such as `can_necromancy`; mutable charge counts never belong
in capability or tag names. Templates are `learnable` by default; set `learnable = false` for an
effect that may be granted and cast but must not enter the player XP unlock list. A passive
high-level literacy template can declare `scroll_learning_tags`; its permanent player grant lets
the host turn a carried, matching-tag, `scroll_learnable` item source into permanent knowledge.
This consumes the actual item through one reducer action; it is not a narrator shortcut or a
special necromancy mechanic.

### Shared Randomized Pools

Use top-level `capabilities` to opt an actor template into the engine's existing deterministic
noise pool rather than writing a type-specific spawner. The current host vocabulary includes
`health`, `combat`, `hearing`, `sight`, `movement`, `inventory`, `noise_spawn`, `noise_track`, and
`sight_track`. A selected NPC or boss can be selected,
materialized, attracted by noise, acquire sight, move, search, and attack movement hazards through
the same reducer-owned loop used by legacy spawned enemies. A materialized runtime NPC or world-item
actor with `noise_track` or `sight_track` joins that loop directly; merely selecting `movement`
does not make an entity wander. Hearing/sight/movement prerequisites
are validated when the pack loads.

Old enemy packs need no migration: `behavior_tags` remains a loader alias and enemies with no
`capabilities` keep their historical behavior.
NPCs and bosses must opt in explicitly. Items that select `noise_spawn` and meet the actor
prerequisites materialize as distinct world-item actors in the same host-owned pool; ordinary items
remain inert. Do not use normal descriptive `tags` to request mechanics.

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

### Player Library Media

Player-owned templates can use the Library's **Edit media** control to pin optional local image,
motion, or audio paths. A pinned template asset is the highest-priority player-owned presentation
choice for that template; otherwise the host selects a matching enabled player-media pool asset
deterministically from its tags. Disabled templates and disabled pool assets are ineligible for
new resolution. Missing pinned files remain visible as missing presentation assets and never alter
mechanics or reducer state.

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
epilogue_description = "The front verandah is still splintered, but the worst pressure has gone slack."
epilogue_hook = "Post-credits hook: a neighbor's porch light flickers twice across the street."
narrator_brief = "Play this as the bad threshold of the run."
tags = ["outdoor", "entry", "starting_location"]

[locations.media]
image = "media/images/locations/front_verandah.png"
audio = "media/audio/ambience/front_verandah_wind.ogg"
display_role = "location"

enemies = ["shambler_front_gate"]
```

The optional `epilogue_description` is used after the active objective is complete. It lets a datapack author make post-win exploration feel authored without adding a scripting system.

The optional `epilogue_hook` is also post-win presentation content. It appears as an aftermath hook, can surface in sidebar/media cues, and creates a future media key such as `epilogue_hook:front_verandah`, but it does not create objectives, items, routes, or state changes by itself.

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
