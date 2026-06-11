# Datapack Spec

## Purpose

Datapacks are the primary content format for Chatty Quest and, more broadly, the RD Engine.

A datapack defines a playable scenario package containing:

- pack metadata
- scenario rules
- templates
- capsule text
- media references

The engine should treat datapacks as first-class playable content, not as hardcoded afterthoughts.

For `v0.1`, one active datapack is required:

- `property_siege_classic`

## Datapack Goals

The datapack layer should support these goals:

- swappable worlds
- readable content structure
- mod-friendly organization
- validation before play
- future asset and narrator expansion without engine rewrites

The goal is not to support infinite flexibility immediately. The goal is to make content external, inspectable, and scenario-driven from the beginning.

## Folder Layout

Recommended `v0.1` layout:

```text
assets/
  datapacks/
    property_siege_classic/
      pack.toml
      rules.toml
      templates/
        locations.toml
        items.toml
        enemies.toml
        bosses.toml
        objectives.toml
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
      capsules/
        dm_style.txt
        world_tone.txt
```

This structure should stay visible and understandable to future contributors and modders.

## Required Files

For `v0.1`, a playable datapack should include:

- `pack.toml`
- `rules.toml`
- `templates/locations.toml`
- `templates/items.toml`
- `templates/enemies.toml`
- `templates/bosses.toml`
- `templates/objectives.toml`

Optional or lightly used in `v0.1`:

- NPC templates
- ambient event templates
- audio references
- video references
- additional capsule files

## `pack.toml`

`pack.toml` should define pack-level metadata.

Suggested responsibilities:

- pack identifier
- display name
- version
- author or source metadata
- short description
- scenario key or primary scenario identity
- supported engine version or compatibility note

This file should be sufficient for the main menu to list the datapack as a selectable scenario pack.

## `rules.toml`

`rules.toml` should define scenario-level rules that affect play.

Likely `v0.1` responsibilities:

- starting location
- scenario boundary behavior
- allowed or blocked exits
- return policies
- lock and unlock behavior
- objective selection mode
- combat baseline modifiers if needed
- fail-state or boundary-response text hooks

This is where `Property Siege Classic` should express that the player is trapped on the property. That behavior should not be hidden in engine code.

## Template Files

Template files define the canonical content pool for the scenario.

At minimum, `v0.1` should support templates for:

- locations
- items
- enemies
- bosses
- objectives

NPCs and ambient events may be optional or minimal in the first release.

## Common Template Fields

Each template type should have a small shared mental model, even if exact fields vary by type.

Common fields should include:

- `id`
- `name`
- `description`
- `narrator_brief` where useful
- `tags`
- optional media references

Additional type-specific fields can then extend that base.

The engine should prefer explicit fields over unstructured text whenever a fact matters mechanically.

Practical rule:

- descriptions are flavour
- fields are truth

The narrator may use prose-rich template text for style, but the reducer and runtime state must rely on explicit fields.

## Location Templates

Location templates should describe places that can exist in the scenario map.

Suggested location fields:

- unique `id`
- display `name`
- short `description`
- `narrator_brief`
- `tags`
- connection hints or allowed connection IDs
- lock state metadata where relevant
- return policy where relevant
- item slots or placement hints
- enemy slots or placement hints
- optional image or ambience references
- optional context-aware media overrides later

For `v0.1`, the map may be authored or semi-authored. Even so, location data should live in templates rather than being buried in UI code.

## Item Templates

Item templates should describe interactable or usable objects.

Suggested item fields:

- unique `id`
- display `name`
- short `description`
- `narrator_brief`
- `tags`
- item category
- usable or equippable flags
- simple stat modifiers where relevant
- optional durability or single-use metadata
- optional media references
- optional use-effect metadata

Items that matter mechanically should expose the relevant data directly rather than forcing the narrator to infer it from prose.

## Enemy Templates

Enemy templates should define hostile entities that participate in deterministic encounters.

Suggested enemy fields:

- unique `id`
- display `name`
- short `description`
- `narrator_brief`
- `tags`
- HP or durability
- attack strength
- encounter flavor text hooks if useful
- optional media references
- optional placement rules
- optional future location-context media overrides

`v0.1` enemy templates should remain small and combat-focused.

## Boss Templates

Boss templates are special enemy templates or parallel content with stronger scenario relevance.

Suggested boss fields:

- unique `id`
- display `name`
- short `description`
- `narrator_brief`
- `tags`
- HP
- attack strength
- special scenario role
- optional lock or objective linkage
- optional media references

For `v0.1`, the important point is not complexity. It is that boss state and objective relevance are explicit and deterministic.

## Objective Templates

Objective templates define the frozen goal structure for a run.

Suggested objective fields:

- unique `id`
- display `name`
- short player-facing summary
- required target IDs
- completion conditions
- optional prerequisite conditions
- optional reward or state-unlock hooks

The narrator may dramatize the objective. The objective template defines what actually counts.

Future scenarios may also use template-backed reward rules or reward pools, but those are future-facing and not required in `v0.1`.

## Media References

Templates may include references to media assets stored under the datapack.

Possible media reference categories:

- location image
- enemy portrait
- boss portrait
- item image
- ambience audio
- short video or animated clip

For `v0.1`, media may remain placeholder or lightly used, but the data format should leave room for richer presentation later.

Recommended `v0.1` media rule:

- datapacks may reference local media files
- Chatty Quest validates and displays them
- Chatty Quest does not generate them
- missing optional media falls back cleanly

Recommended naming rule:

- use template ids in snake_case
- use double underscore for dual-context assets
- prefer `entity_id__location_id.ext` over symbols like `+`
- always put the focused entity or item first and the current location second
- do not author both orders for the same override

Examples:

- `front_gate_shambler__kitchen.png`
- `cricket_bat__kitchen.png`
- `garage_brute__garage.mp4`

Recommended display logic:

- attempt a dual-context override first when a focused entity and current location both exist
- otherwise use the strongest single focused media
- if focused media is missing, fall back to current location media
- if location media is missing, fall back to a datapack placeholder
- if the datapack has no placeholder, fall back to an engine-level placeholder

Practical `v0.1` examples:

- moving into `kitchen` while `front_gate_shambler` is the surfaced threat may try `front_gate_shambler__kitchen.png` first
- if that override does not exist, the engine may fall back to `kitchen.png`
- inspecting or finding `cricket_bat` in `kitchen` may try `cricket_bat__kitchen.png` first
- if that override does not exist, the engine may fall back to `cricket_bat.png`, then `kitchen.png`, then placeholder

For `v0.1`, dual-context assets should remain additive rather than required. The engine should never assume that every entity-location combination has been authored.

Future datapacks may optionally support context overrides such as `entity + current_location`, but those should remain additive rather than mandatory.

## Capsule Files

Capsules shape tone, not truth.

For `v0.1`, plain text capsule files are enough.

Suggested capsule roles:

- `dm_style.txt` for narrator voice
- `world_tone.txt` for scenario atmosphere

Capsules may influence narration style, but they must not contain hidden gameplay rules that bypass deterministic systems.

## Validation Rules

Datapacks should be validated before they appear selectable or before a run begins.

At minimum, validation should detect:

- missing required files
- duplicate IDs
- missing referenced IDs
- broken media paths
- impossible location connections
- missing starting location
- missing boss where scenario expects one
- impossible objective definitions

Validation errors should be useful and human-readable.

The goal is to make broken content diagnosable rather than silently tolerated.

Optional media should not block the scenario from running by default. The engine should prefer readable warnings and placeholders over hard failure where the content is decorative rather than canonical.

## `Property Siege Classic` Notes

`Property Siege Classic` should demonstrate the datapack format with one small, complete scenario pack.

It should show:

- a bounded property map
- scenario-defined outside boundary rules
- deterministic item and enemy placement logic
- a simple boss or end-condition structure
- capsule-driven tone hooks

This datapack is the first proof that the engine can host a real scenario without hardcoding the whole game around it.

## Future-Compatible But Inactive Fields

The datapack spec may reserve room for future features such as:

- NPC affinity metadata
- NPC dialogue cadence hints
- reward-pool hints
- ambient event pools
- richer media triggers
- context-aware media overrides
- style-pack references
- handoff packet hints

These should remain clearly optional and inactive in `v0.1`.

The pack format should not force future complexity into the first playable slice.
