# Narrator Context Spec

## Purpose

This document defines the pre-LLM narrator contract for Chatty Quest and the RD Engine.

The goal is not to connect a real model yet.

The goal is to make sure every future narrator receives bounded, reducer-confirmed truth instead of raw mutable game internals or prose that it has to reinterpret.

Short rule:

- reducer owns truth
- narrator context packages truth
- narrator presents truth

## Current Implementation

The current branch introduces:

- `NarratorContext`
- `NarratorExitContext`
- `build_narrator_context`

These live in `src/game/narrator.rs`.

`MockNarrator` now builds and consumes this context for action narration. That proves the seam before any real LLM adapter exists.

Raw player text is resolved before this context is built. The command boundary is documented in [COMMAND_BOUNDARY_SPEC.md](COMMAND_BOUNDARY_SPEC.md).

## Context Contents

`NarratorContext` currently includes:

- attempted action, when there is one
- reducer output lines
- structured event facts derived from `GameEvent`
- run phase: `Active`, `Epilogue`, or `Loss`
- current location id, name, description, and narrator brief
- current focus brief derived from reducer-confirmed event focus
- visible exits with locked and barricaded state
- bounded recent rolling summary support memory
- inventory item names
- equipped item name
- HP and max HP
- noise level and label
- active objective name, completion state, and condition facts
- datapack DM style and world tone capsules
- explicit narrator safety rules

This is intentionally small. It should grow only when a real narrator needs a new confirmed fact.

## Safety Rules

The context carries safety rules that future narrator adapters should treat as hard constraints:

- do not invent permanent items
- do not invent canonical locations
- do not alter HP, inventory, equipment, lock state, barricade state, objective state, or player location
- do not claim a reducer action succeeded if the event facts say it was rejected or blocked
- treat reducer lines and event facts as authoritative
- use tone and atmosphere only as presentation over confirmed game truth

These are not prompt decoration. They are the narrator boundary.

## What The Context May Do

The context may:

- translate reducer-confirmed events into compact facts
- expose current local scene state
- include presentation capsules from the datapack
- expose player-visible route state
- expose objective truth
- expose epilogue room descriptions after a win

## What The Context Must Not Do

The context must not:

- mutate `RunState`
- infer hidden objective progress
- reveal hidden content that the current UI would not reasonably expose
- turn narrator prose into mechanics
- carry speculative future outcomes

## Future LLM Adapter Shape

A future LLM narrator should sit behind the same `Narrator` trait or a successor with equivalent boundaries.

The adapter should receive a `NarratorContext`, not raw game state.

The adapter may produce:

- atmospheric prose
- social texture
- clearer phrasing of reducer-confirmed outcomes
- tone-consistent epilogue narration

The adapter may not produce:

- new canonical items
- new canonical locations
- unconfirmed HP changes
- unconfirmed unlocks or barricades
- unconfirmed objective progress
- alternate combat results

## Current Non-Goals

Do not add these yet:

- real LLM API calls
- prompt streaming
- tool calls from the narrator
- narrator-owned memory
- hidden narrator state
- broad natural-language command interpretation

Those can come later only after the context contract has stayed stable under the mock narrator and demo datapack.
