# Design Intent

## Purpose

This document preserves the intended product shape of Chatty Quest by combining:

- the expansive planning conversation in `chatty-quest-planning.txt`
- the scoped deterministic handoff in `chatty-quest-plan.txt`
- later roadmap planning such as `templates-and-buckets-planning.txt`

It exists to stop two common failure modes:

- losing the larger scenario-engine vision while building `v0.1`
- overbuilding `v0.1` in ways that delay the first playable slice

This project should be built as a small, sane seed of a larger engine, not as a throwaway prototype and not as an attempt to implement the full long-term dream immediately.

## Core Product Idea

Chatty Quest is a moddable Rust desktop adventure engine where deterministic templates and bucketed state create the world, while a narrator layer presents that world as a chat-forward Dungeon Master experience.

Short pitch:

`AI Dungeon, but with actual game state, maps, inventory, save files, datapacks, and deterministic rules.`

The engine owns truth. The narrator owns flavour.

As the project matures, it is useful to name the reusable substrate explicitly:

- `Chatty Quest` is the first game
- `RD Engine` is the reusable engine
- `Radiant Determinism` is the doctrine

## Non-Negotiable Core Line

Templates define canon. Buckets track state. Reducer mutates truth. Narrator describes truth. UI displays truth.

This line is not just a slogan. It is the architecture test for every future feature.

Working translation:

- templates are the nouns
- buckets are the current grammar
- reducers are the verbs
- narration is the accent
- media is the illustration

## What Must Be Preserved From The Original Vision

### It is a scenario engine, not just one zombie game

`Property Siege Classic` is the first scenario, not the whole identity of the project.

The reusable product is:

- datapack-driven world definition
- generated or scenario-assembled run state
- deterministic action handling
- a narrator seam over structured state
- a GUI shell that presents the run cleanly

Future scenario families may include:

- zombie property survival
- fantasy dungeon crawling
- haunted house horror
- spaceship survival
- pirate treasure hunts
- war-map conquest
- detective mystery

`v0.1` should avoid hardcoding assumptions that only make sense for zombies, one property, or one tone.

### It should feel chat-first, not menu-first

The fantasy of the project is that the player is talking to a Dungeon Master inside a dashboard game shell.

That means the experience should center:

- a chat log
- text input
- narrated outcomes
- a sense of improvisational response

Even when the underlying action parser is narrow in `v0.1`, the surface should still feel like a conversational adventure rather than a form-based utility.

### It should be expressive and funny, not sterile

The planning threads make it clear that personality matters.

The project should support:

- hostile or theatrical DM voices
- horror and comedy
- absurd flavour
- strong scenario identity

Deterministic architecture must not flatten the experience into dry system messages.

### It should be obviously moddable

The project should communicate, even early, that worlds are swappable through datapacks and assets.

That means:

- data files should be first-class
- pack structure should be visible and understandable
- reserved folders for future models, datasets, and handoff lanes should make architectural intent obvious

`v0.1` does not need advanced mod tooling, but it should clearly point in that direction.

### It should preserve the future template-instance-bucket shape

The planning notes sharpen an important future-facing distinction:

- template = blueprint
- instance = this spawned copy
- bucket = current status or container of that instance

`v0.1` can stay lighter than the full long-term model, but we should avoid writing docs or code that collapse these concepts into one indistinct blob.

## What Must Be Preserved From The Deterministic Handoff

### The narrator is never the source of truth

The narrator may:

- describe state
- frame events
- supply atmosphere
- voice NPCs
- turn success and failure into entertaining prose

The narrator may not:

- create permanent items
- create new canonical map locations
- move the player without reducer approval
- change HP directly
- modify inventory directly
- unlock doors directly
- kill entities directly
- complete objectives directly

All canonical state changes must pass through deterministic game logic.

Corollary:

- descriptions are flavour
- fields are truth

### Scenario rules, not engine hacks, define boundaries

The famous example remains important:

Player: `I run to Bunnings.`

Narrator: `You make it three fences before the horde eats you, idiot.`

That is not a joke-only feature. It is a design principle.

The engine should understand that the current scenario defines an outside boundary, and the narrator should express the consequence in scenario tone. The engine should not be built around a special global law that says players can never leave maps in general.

### Save/load and validation matter from the beginning

The point of this project is not vague AI storytelling. It is bounded imagination over real game state.

So even in `v0.1`, the project should care about:

- valid datapacks
- deterministic run state
- serializable saves
- recoverable sessions
- testable transitions

### `v0.1` must stay small on purpose

The first release should prove the shape, not the total fantasy.

We are validating:

- desktop shell
- scenario loading
- deterministic state
- reducer-driven play
- one playable scenario
- replaceable narrator seam

We are not validating every future system yet.

## The Four Important Dials

The planning thread implies four distinct configuration dimensions that should remain conceptually separate:

### Datapack

Changes the world, templates, assets, and scenario identity.

### Difficulty

Changes mechanical pressure such as map size, encounter density, resource scarcity, or encounter mix.

### DM Capsule

Changes narration voice and tone only.

Examples:

- grim survival narrator
- hostile meatgrinder DM
- slapstick horror narrator
- cozy storybook narrator

### Chaos Mode

Changes looseness of narration and event suggestion, but not truth ownership.

Chaos can make the narrator weird, theatrical, or surreal.
Chaos cannot give the narrator authority over canonical state.

## Recommended `v0.1` Interpretation

To preserve the vision without scope blowout, `v0.1` should be interpreted this way:

### Scenario

One playable datapack: `Property Siege Classic`.

### World Structure

Use a small authored or semi-authored property map with deterministic connections.

If seeded variation is included, keep it light:

- item placement variation
- enemy placement variation
- objective variation

Do not require fully procedural map generation before the first playable version works.

### Input Model

Support text input because it is central to the fantasy.

But `v0.1` should not pretend to understand unrestricted natural language. A narrow interpreter is acceptable.

Good `v0.1` shape:

- player types freeform text
- engine matches obvious intents to known actions
- reducer validates and applies the action
- narrator returns a styled response
- unknown inputs receive graceful failure narration

### Combat

Keep combat simple and deterministic.

Enough for `v0.1`:

- HP
- equipped item
- enemy HP or alive/dead state
- simple attack resolution

Avoid:

- full D&D math
- initiative ladders
- status stacks
- party systems
- advanced AI tactics

### Objectives

Support a simple frozen objective with visible progress.

The project fantasy includes generated adventures, but `v0.1` only needs enough objective logic to prove that scenario goals can exist independently of narration.

### Media

The image/media panel is part of the intended feel, even if `v0.1` uses placeholders.

The architecture should preserve future support for:

- location art
- item art
- enemy portraits
- audio ambience
- video or animated media hooks

`v0.1` may display placeholder paths or static assets, but the datapack format should leave room for richer media later.

The media planning notes suggest a good future-facing rule:

- location entry defaults focus to location media
- look, inspect, encounter, and item actions may shift focus to a more relevant entity
- future context-specific overrides may exist for `entity + location`
- media never creates truth; it only reflects current focus

## Confirmation And Mutation Philosophy

The planning conversation introduced a useful idea: not every narrated possibility should instantly become fact.

For `v0.1`, we should preserve this philosophy even if implementation stays simple.

Practical reading:

- some actions may resolve immediately
- some actions may produce a pending result before being committed
- the engine, not the narrator, decides whether a mutation is legal

Not every action needs a confirmation step in `v0.1`, but the architecture should not assume that every possible future mutation is immediate and unquestioned.

## Rolling Summary

Rolling summary should be treated as support memory, not truth storage.

It is useful for:

- recap
- narrator continuity
- preserving memorable events
- keeping the chat log from becoming the only history surface

It must not replace structured state, inventory, objective flags, or map truth.

Future clarification:

- if a fact affects mechanics, bucket it
- if a fact only affects tone, recognition, or conversational continuity, summarize it

## Desired Emotional Outcome

The project should feel like:

- a real game, not a prompt toy
- a chat-driven adventure, not a spreadsheet
- moddable and expandable, not one-off and brittle
- funny, tense, or theatrical depending on capsule and scenario
- bounded enough to trust, open enough to feel imaginative

In RD Engine terms, the target feeling is:

- dynamic-feeling behaviour with receipts

If a future implementation is technically correct but loses this feeling, it has missed part of the brief.

## `v0.1` Discipline

The safe seed point is not a compromise with the vision. It is how we protect the vision.

`v0.1` should deliberately prove:

- the architecture is sound
- the scenario pack model works
- the narrator seam is replaceable
- the UI shell can present chat, map, inventory, and character state
- save/load and validation keep the experience trustworthy

`v0.1` should deliberately avoid:

- pretending to be infinite
- depending on real LLM calls
- implementing future Chatty ecosystem handoffs
- building advanced procedural systems before the core loop is fun
- solving every future genre variant up front

## Decision Filter

When deciding whether a feature belongs in `v0.1`, ask:

1. Does this protect or clarify deterministic truth ownership?
2. Does this improve the playable chat-forward scenario loop?
3. Does this strengthen the scenario-engine foundation?
4. Can this be deferred without harming the long-term shape?

If the answer to the first three is mostly no, it probably does not belong in `v0.1`.

## Working Interpretation

The correct build posture is:

- preserve the big engine vision
- implement the smallest honest playable slice
- keep the narrator expressive
- keep the game state deterministic
- keep future expansion visible but dormant

That is the shape we should carry into the full docs pass and then into implementation.
