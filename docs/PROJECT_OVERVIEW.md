# Project Overview

## What Chatty Quest Is

Chatty Quest is the first game built on the `RD Engine`, the `Radiant Determinism Engine`.

At the product level, Chatty Quest is a Rust desktop adventure engine built around deterministic game state, datapack-defined scenarios, and a chat-forward narrator experience.

The project is designed so that structured templates and runtime buckets define what is real, while a narrator layer turns that reality into a lively Dungeon Master style play experience.

Short pitch:

`AI Dungeon, but with actual game state, maps, inventory, save files, datapacks, and deterministic rules.`

This is not intended to be a pure prompt toy or a freeform hallucination sandbox. It is intended to be a real game engine with a narrator seam.

Shorter doctrine line:

`Radiant adventure feel, deterministic game truth.`

## Historical `v0.1` Release

`v0.1` is the first honest playable slice of the larger engine vision.

Current status:

- `v0.1 accepted`
- automated test sweep green
- manual release sweep completed successfully on `2026-06-11`

It exists to prove:

- a Rust desktop GUI shell can present the game cleanly
- a datapack can define a playable scenario
- deterministic state can drive movement, inventory, combat, and objective progress
- a mock narrator can provide chat-style presentation without owning truth
- save/load and validation can make the run trustworthy

`v0.1` did not try to solve every future scenario type, world scale, or AI integration problem.

## Current Engine Substrate

The active branch keeps the release's deterministic core and adds reusable primitives rather than
one-off noun-specific systems:

- capability-gated dependent schemas, with a capability as the root of every applicable
  mechanical contract;
- runtime actors and controller-neutral actions, so player, NPC, enemy, boss, and actor-capable
  item use the same combat, movement, inventory, ability, and timing paths when capable;
- one canonical event projection (`type | origin | impact | outcome`) for narration, media,
  diagnostics, summaries, and action traces;
- entity-agnostic item ownership through `TransferItem` and `ItemTransferred`;
- immutable ability templates plus owned `AbilityGrant` instances for charges, expiry, cooldown,
  temporary access, learning, and actor-local progression;
- semantic media roles and hierarchical pools, with deterministic fallback and inspection;
- host-selected, bounded narrator proposals that are validated, player-approved, and reducer
  committed before they become canonical.

This is still deliberately bounded. It is a substrate for future content families, not a claim
that quests, trade, memory, magic schools, or negotiation have all been implemented.

## Why The Host Owns State

The host engine must own canonical truth so that the game remains:

- testable
- saveable
- reloadable
- moddable
- predictable
- portable across future narrator implementations

If narration becomes the source of truth, the project loses its core identity and turns into an unstable storytelling wrapper.

The engine therefore owns:

- map and connection state
- player location
- inventory and equipment
- HP and combat outcomes
- enemy and boss alive/dead state
- objective progress
- locked and unlocked locations
- scenario boundaries
- save data
- actor identities, control, health, readiness, capabilities, and owned ability grants
- item ownership and equipment relationships
- per-actor progress pools and unlock state

The narrator may describe truth, but it does not create truth.

## Why Templates And Buckets Are Used

Templates define the canonical building blocks of a scenario:

- locations
- items
- enemies
- bosses
- NPCs
- objectives
- media references

Buckets define the live state of a run:

- visited versus unvisited
- locked versus unlocked
- world item versus inventory item
- equipped versus unequipped
- alive versus defeated
- current objective progress

This shape makes the game easier to:

- reason about
- validate
- serialize
- extend with new datapacks
- eventually narrate with stronger AI systems without surrendering control

Longer-term, the engine should continue moving toward a clean three-part content model:

- template = blueprint
- instance = this spawned copy
- bucket = where that instance currently lives or what state currently applies

The current runtime now materializes entity and ability instances where mutable ownership or
lifetimes require them. It still preserves the template = blueprint, instance = copy, bucket =
current relationship distinction rather than hiding it in narrator prose.

The implementation-facing runtime contract for that split is documented in [RUNTIME_MODEL_SPEC.md](RUNTIME_MODEL_SPEC.md).

## Why `Property Siege Classic` Is First

`Property Siege Classic` is the right first scenario because it is:

- easy to understand
- naturally bounded
- small enough for `v0.1`
- tense by default
- flexible in tone

It demonstrates the system clearly:

- the map is limited
- movement and boundaries matter
- items and combat are meaningful
- scenario rules can prevent leaving the playable space
- the narrator can be funny or hostile without changing game truth

The property itself is the playable map in `v0.1`. That is a scenario rule, not a permanent engine law.

## Historical `v0.1` Exclusions

The following describes the original release boundary, not necessarily the current branch:

- multiplayer
- peer-to-peer networking
- live Chatty ecosystem handoff
- advanced procedural generation
- advanced D&D-style combat
- party systems
- overworld mode
- unrestricted natural language understanding
- narrator-controlled mechanics

It also does not require:

- full NPC memory systems
- dialogue-tree replacements
- dynamic reward negotiation
- generalized template-instance duplication for every content family

Those are future RD Engine growth areas, not first-slice requirements.

## Experience Goals

Even at small scope, Chatty Quest should feel:

- chat-first rather than form-first
- deterministic rather than vague
- expressive rather than sterile
- moddable rather than hardcoded
- bounded enough to trust and open enough to feel imaginative

`v0.1` succeeds when it proves the architecture and still feels like a real, characterful adventure rather than a dry systems demo.

That standard has now been met for the initial `Property Siege Classic` release slice, which gives the project a clean pause point before `v0.2` expansion.
