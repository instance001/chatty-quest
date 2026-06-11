# RD Engine Principles

## Purpose

This document names the deeper engine doctrine underneath Chatty Quest.

`Chatty Quest` is the first game.
`RD Engine` is the reusable substrate.
`Radiant Determinism` is the design doctrine.

The engine goal is not just deterministic correctness. It is deterministic correctness that still feels lively, expressive, and surprising at the surface.

## Core Sentence

Templates define canon. Buckets track state. Reducer mutates truth. Narrator describes truth. UI displays truth.

This remains the central architecture sentence.

## Working Translation

For day-to-day design, the practical translation is:

- templates are the nouns
- buckets are the current grammar
- reducers are the verbs
- narration is the accent
- media is the illustration

That sentence is useful because it explains not just how the engine works, but why it scales.

## Radiant Determinism

Radiant Determinism means:

- the world feels dynamic
- narration feels adaptive
- NPCs can appear to remember things
- quests can pay off in satisfying ways
- media can feel context-aware

while:

- templates define what is legal
- buckets define what is currently true
- reducers are the only way meaningful truth changes
- UI surfaces visible proof that a state change actually happened

The shimmer is expressive. The spine is deterministic.

## Descriptions And Fields

Descriptions are flavour.
Fields are truth.

That rule should apply across the engine:

- template prose may inspire narration
- template fields define canon
- runtime buckets define current truth
- media decorates truth
- summaries support memory

If a fact affects mechanics, it must be represented in structured state or reducer-visible data.

## Template, Instance, Bucket

The long-term content model should stay conceptually clean:

- template = blueprint
- instance = this spawned copy
- bucket = current status or container of that instance
- media = optional presentation assets
- narrator brief = safe flavour guidance

Not every `v0.1` implementation needs full instance complexity yet, but the doctrine should stay visible now so future growth remains coherent.

## Mechanical Payoff

The engine should aim for moments that feel magical because narration, UI, and structured state all agree.

Example pattern:

- an NPC promises a reward
- the promise is represented in structured state
- the quest completes through reducer-confirmed logic
- a real item template becomes a real inventory instance
- the narrator voices the handoff
- the inventory tab proves the item now exists

That is the signature RD Engine trick:

dynamic-feeling behaviour with receipts.

## Summary Rule

Rolling summary exists to support continuity, not to replace truth.

Rule:

- if a fact affects mechanics, bucket it
- if a fact only affects tone, memory, or conversational continuity, summarize it

This keeps the narrator rich without turning prose into the hidden database.

## Media Rule

Media is presentation only.

Media may:

- reinforce current focus
- support scenario tone
- decorate reducer-confirmed outcomes
- provide future image, video, or audio hooks

Media may not:

- create mechanics
- imply truth that the reducer did not confirm
- unlock content
- complete objectives
- grant items

## Engine Identity

The intended naming split is:

- `Chatty Quest`: game/app identity
- `RD Engine`: reusable engine identity
- `Radiant Determinism`: design doctrine

Recommended short pitch:

`RD Engine: radiant adventure feel, deterministic game truth.`
