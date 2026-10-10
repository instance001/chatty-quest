# Narrator Context Spec

## Purpose

This document defines the narrator-context contract for Chatty Quest and the RD Engine.

The goal is to make sure every narrator—Mock, local GGUF, or hosted BYO provider—receives bounded,
host-selected information instead of raw mutable game internals or prose it has to reinterpret.

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

`MockNarrator` and live narrator adapters build on this bounded context. A live adapter has separate
plain-text narration and compact proposal lanes; neither receives authority to mutate state.

Raw player text is resolved before this context is built. The command boundary is documented in [COMMAND_BOUNDARY_SPEC.md](COMMAND_BOUNDARY_SPEC.md).

## Context Contents

`NarratorContext` currently includes:

- attempted action, when there is one
- reducer output lines
- structured event facts derived from `GameEvent`
- run phase: `Active`, `Epilogue`, or `Incapacitated` (the 0-HP controlled actor state, which is reversible)
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

This is intentionally small. It should grow only when a narrator needs a new confirmed fact.

## Safety Rules

The context carries safety rules that narrator adapters must treat as hard constraints:

- do not claim permanent items or canonical locations exist
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

## Live Narrator Adapter Shape

A live narrator sits behind the same bounded narrator seam as the Mock narrator. It receives a
`NarratorContext`, not raw game state.

The narration lane may produce:

- atmospheric prose
- social texture
- clearer phrasing of reducer-confirmed outcomes
- tone-consistent epilogue narration

The narration lane may not produce:

- new canonical items
- new canonical locations
- unconfirmed HP changes
- unconfirmed unlocks or barricades
- unconfirmed objective progress
- alternate combat results

For an eligible free-text turn, the host can independently allow a separate proposal lane. The host
first chooses authorized form kinds, then asks the narrator to select one and fill a small flat form.
The host parses and validates those fields, the player may edit the approval card, and a reducer
alone commits an accepted proposal. The adapter never constructs a canonical internal JSON object or
applies a mutation itself.

## Current Non-Goals

Current non-goals remain:

- narrator tool calls or direct action execution
- narrator-owned hidden state
- unrestricted natural-language command interpretation
- narration as an alternative source of saved truth
