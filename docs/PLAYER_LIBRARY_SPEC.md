# Player Library Spec

## Purpose

The player library is a user-owned template source that mirrors datapack templates without modifying an authored pack.

It supports the permissive end of the RD Engine play contract:

- a narrator proposes novel flavour
- the player approves it
- the engine converts it into a typed library template and/or a run instance
- buckets, reducers, saves, UI, and media treat the admitted instance as normal truth

## Two Template Sources

The engine reads templates from two sources:

1. **Datapack templates**: authored, validated scenario canon.
2. **Player-library templates**: player-owned, validated additions scoped to compatible datapacks or globally permitted by a pack.

They share a common mental model: identity, type, display data, tags, mechanical fields, media references, and provenance. A player library never writes into `assets/datapacks`.

## Admission Pipeline

```text
LLM proposal
  -> readable confirmation card
  -> typed proposal validation
  -> player accepts / declines
  -> reducer creates run instance
  -> optional player-library promotion
  -> future runs may spawn compatible library templates
```

The narrator never directly inserts a bucket entry. The reducer owns placement in inventory, location, encounter, objective, or status buckets.

## Player Template Families

The shared schema supports these families as bounded, validated template sources:

- item / weapon
- NPC
- enemy
- boss
- location / temporary exit
- objective
- status or effect
- media asset

Each family has explicit fields appropriate to its bucket. For example, a player enemy needs a runtime encounter instance with HP, location, life status, faction/tags, and combat profile; a player item needs an item instance with owner/location bucket, damage/effect fields, and lifecycle state.

## Tags And Media

Tags are structured template truth. Media is illustrative and selected from the player media library by deterministic tag matching.

Resolution priority:

```text
exact authored context override
-> exact entity media
-> player-library media matching entity/context tags
-> authored location media
-> datapack fallback
-> engine fallback
```

A sword-like generated item can therefore carry tags such as `weapon`, `blade`, and `absurd`; a player media asset tagged `weapon` or `blade` becomes eligible without the LLM needing to name a file. A selected player asset must be surfaced as player-library media, never misrepresented as pack-authored content.

Media selection may not create mechanics. Tags and admitted instances create mechanics; media follows them.

## Compatibility Boundary

Each admitted family must use the same reducer, save/load, diagnostics, UI, event-projection, and
acceptance path as authored content. A player library is an additional template source, never an
exception that lets a narrator bypass capability validation, ownership, ability grants, or media
resolution.
