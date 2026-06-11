# Known Non-Goals

## Purpose

This document defines what `v0.1` is not trying to do.

It exists to stop accidental scope inflation and to keep the project aligned with its core promise:

- deterministic game state
- chat-forward presentation
- one complete datapack-driven playable scenario

If a future idea sounds exciting but does not help prove that promise, it probably belongs outside `v0.1`.

## AI And Model Non-Goals

`v0.1` does not implement:

- real LLM calls
- hosted model APIs
- autonomous world-authoring AI
- narrator-owned game truth
- unrestricted natural language understanding
- model loading from `models/` as a gameplay requirement

Why:

The first release proves the narrator seam through `MockNarrator`, not through live AI integration. The truth model must be valid before any real model is allowed near it.

## Chatty Ecosystem Non-Goals

`v0.1` does not implement:

- live Chatty-Cog orchestration
- peer-to-peer state relay
- mediated inter-module handoff requests
- ChattyCog sandbox export
- live Chatty-Art asset generation requests
- live Chatty-Lora training or style return loops

Why:

The reference projects make an important separation clear: each module owns its own real state, and cross-module exchange is explicit, mediated, copy-only, and user-confirmed. Chatty Quest should reserve similar seams later, but it should not embed those systems in the first playable game release.

## Multiplayer And Networking Non-Goals

`v0.1` does not implement:

- multiplayer
- LAN sync
- remote rooms
- peer sessions
- shared co-op state
- real-time spectator or orchestrator roles

Why:

The project needs a trustworthy single-run local truth model before any coordinated multi-user state becomes sensible.

## Gameplay Scope Non-Goals

`v0.1` does not implement:

- full D&D rules
- advanced combat math
- initiative ladders
- status-effect stacks
- stamina systems
- ammo simulation
- full skill trees
- party systems
- recruitable companions
- pets
- overworld mode
- conquest mode
- settlement building
- unrestricted free-roam outside scenario boundaries

Why:

The first release needs a small playable loop, not a maximal rules engine.

## Scenario Breadth Non-Goals

`v0.1` does not implement:

- multiple fully realized scenario families
- a generalized overworld plus sublocation system
- all future game modes mentioned during early planning
- deeply branching campaign structures
- large-scale procedural lore generation

Why:

`Property Siege Classic` is the first proof-of-shape scenario. Future scenario breadth is part of the long-term engine vision, not a first-release requirement.

## Procedural Generation Non-Goals

`v0.1` does not implement:

- fully procedural world topology generation
- large content-pool balancing systems
- deeply dynamic quest generation
- open-ended simulation of every possible environmental state

What is acceptable:

- a small authored or semi-authored map
- light seeded variation for item, enemy, or objective placement

Why:

The engine should prove scenario-driven play first. Heavy procedural systems can come later once the deterministic foundation is stable.

## Narration Non-Goals

`v0.1` does not implement:

- narration that can mutate game truth directly
- lore generation that overrides templates
- dynamic creation of permanent items or locations through prose
- freeform “yes, and” world mutation without reducer approval

Why:

Narration is flavor, not authority.

## Media Non-Goals

`v0.1` does not implement:

- image generation
- audio generation
- video generation
- live media editing
- full dynamic media pipelines
- automatic imports from Chatty-Art outputs

What is acceptable:

- placeholder media panels
- static media references in datapacks
- basic display of local asset paths or packaged assets

Why:

Media is part of the long-term feel, but not a gating dependency for the first playable engine slice.

## Training And Style Non-Goals

`v0.1` does not implement:

- LoRA training
- style-pack training workflows
- dataset curation
- source crawling
- concept stack planning
- style-consistency enforcement from trained adapters

Why:

Those belong to Chatty-Lora and future ecosystem integration, not the first game engine release.

## Data And Runtime Non-Goals

`v0.1` does not implement:

- datasets as an active gameplay dependency
- model registries as active gameplay dependency
- background orchestration daemons
- complex runtime cache policies
- historical run analytics systems

Why:

Reserved folders should communicate future intent without becoming excuses to implement every future subsystem now.

## UI Non-Goals

`v0.1` does not implement:

- every advanced dashboard panel imagined during planning
- a full browser-like multitool environment
- embedded web dashboards from sibling Chatty tools
- module-hosting behavior similar to ChattyCog

Why:

Chatty Quest is a game client in `v0.1`, not an orchestrator shell.

## Implementation Discipline

When in doubt, `v0.1` should prefer:

- explicit state over implied state
- deterministic logic over expressive guesswork
- one good scenario over many shallow ones
- documented seams over half-built integrations
- honest narrow input handling over fake general intelligence

If a feature weakens those priorities, it belongs on the future list rather than in the first release.
