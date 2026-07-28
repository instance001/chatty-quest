# Command Boundary Spec

## Purpose

This document defines the command boundary for Chatty Quest and the RD Engine.

The command boundary exists between raw player text and reducer-owned game truth.

Short rule:

- raw text is input
- command parsing resolves intent
- reducer actions mutate truth
- narrator presents confirmed results

## Current Implementation

The current implementation lives in `src/game/actions.rs`.

It exposes:

- `ParsedCommand`
- `parse_command`
- `parse_action`
- `GameAction`

`parse_command` is the main app-facing boundary. It keeps the raw trimmed input, a normalized lowercase input, and the resolved structured `GameAction`.

`parse_action` remains as a small compatibility helper for tests and reducer-focused callers that only need the action.

## Pipeline

The runtime command path is:

```text
player text
  -> parse_command
  -> ParsedCommand
  -> GameAction
  -> apply_action
  -> ActionOutcome
  -> NarratorContext
  -> Narrator
  -> UI log
```

Parser failures stop before the reducer and narrator.

Reducer rejections still produce structured `ActionOutcome` data, because the player attempted a valid command shape that failed game rules.

## Parser Responsibilities

The parser may:

- trim raw input
- normalize command matching
- map known verbs and aliases to `GameAction`
- reject unknown command shapes
- reject target verbs when their target is blank

The parser may not:

- mutate `RunState`
- infer hidden game state
- decide whether a command succeeds mechanically
- call a narrator
- ask an LLM to reinterpret arbitrary text

## Reducer Responsibilities

The reducer receives only structured actions, the current run state, and the datapack bundle.

It owns:

- legality checks
- state mutation
- structured events
- deterministic outcome lines
- objective completion
- win/loss truth

## Narrator Responsibilities

The narrator receives reducer-confirmed context after parsing and reduction.

It may:

- add style
- clarify confirmed results
- present the scene in the configured tone

It may not:

- decide command intent from raw player text
- convert failed parser text into mechanics
- override reducer rejection
- invent state changes

## Future Natural-Language Direction

A future natural-language input system should be a command resolver, not a narrator power.

The safe shape is:

```text
player text
  -> deterministic parser or bounded resolver
  -> candidate GameAction
  -> reducer
  -> narrator context
  -> narration
```

If a model is ever used to help resolve player intent, it should output a bounded command candidate that is still validated before the reducer runs.

It should not produce authoritative prose as a substitute for `GameAction`.

## Current Non-Goals

Do not add these yet:

- broad free-text command interpretation
- LLM command parsing
- narrator-side action execution
- command memory outside `RunState`
- hidden intent inference

The demo should stay small, explicit, and inspectable while the first datapack proves the engine contract.
