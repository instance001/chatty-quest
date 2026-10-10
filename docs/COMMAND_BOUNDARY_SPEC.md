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
  -> recognized command: GameAction -> apply_action -> ActionOutcome -> narration
  -> other free text: bounded narration lane -> optional host-gated proposal lane
  -> UI log / editable proposal card
```

Recognized commands use the reducer path. Other free text is not converted into a command by the
narrator; it can receive presentation-only narration and may independently be considered for a
bounded proposal.

Reducer rejections still produce structured `ActionOutcome` data, because the player attempted a valid command shape that failed game rules.

## Controller-Neutral Action Dispatch

The command boundary describes an attempted action, not a player-only mechanic. The reducer
resolves the current `player_entity_id` as the controlled actor at commit time. That actor becomes
the action origin only after the host has validated the relevant capability and runtime
prerequisites.

```text
`attack <target>`
  -> `AttackTarget { target }`
  -> reducer resolves controlled actor and present target
  -> validates combat contract, hostility, health, readiness, and targetability
  -> commits events or emits a structured rejection
```

The same rule applies to move, wait, inspect, ownership transfer, and ability commands. Changing
the controlled actor changes the origin and its owned state (inventory, ability grants, progress,
readiness); it never selects a separate NPC-only or item-only reducer implementation.

The parser preserves only bounded syntax. It does not look up the actor, infer whether prose
claimed success, or decide who may act. Named-actor natural-language bridges, when available,
must resolve into the same bounded host action request and then pass the same reducer validation.

## Current Command Families

The exact accepted aliases live beside the parser, but the current command families include:

- `go <location>`, `look`, `inspect <thing>`, and `wait`;
- `take <item>`, `drop <item>`, `equip <item>`, `unequip <item>`, and `use <item>`;
- `give <item> to <actor>`, `<actor>, take my <item>`, `<actor>, give me <item>`, and
  `steal <item> from <actor>`;
- `attack [target]` and `hit [target]`; a supplied target remains explicit through reduction;
- `cast <ability> [at <target>]`, `learn <ability>`, and bounded progression commands;
- bounded social commands whose event outcomes are host-defined.

Ownership language always resolves into `TransferItem`, and ability language always resolves to a
real owned `AbilityGrant`. No command wording creates an alternate mechanic.

`cast <ability> at <target>` always means that the controlled actor is casting. Mentioning a
present actor as the target must not reinterpret that actor as the caster. A named non-player
actor must use one of the explicit forms `<actor>, cast <ability> at <target>`,
`<actor> cast <ability> at <target>`, or `ask <actor> to cast <ability> at <target>`; that actor
then still needs its own accessible grant and must pass the same reducer validation.

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
- resolve player control or capability legality

## Reducer Responsibilities

The reducer receives only structured actions, the current run state, and the datapack bundle.

It owns:

- legality checks
- state mutation
- structured events
- deterministic outcome lines
- objective completion
- win/incapacitation truth
- controlled-origin resolution, capability checks, and action readiness
- generic event projection inputs (`type`, `origin`, `impact`, `outcome`)

## Narrator Responsibilities

The narrator receives bounded context after parsing and, for command turns, reduction. For free
text, it can receive the player message plus bounded visible context.

It may:

- add style
- clarify confirmed results
- present the scene in the configured tone
- fill a host-selected bounded proposal form when the host gate permits it

It may not:

- decide command intent from raw player text
- turn free text or prose directly into mechanics
- override reducer rejection
- invent state changes

## Free Text And Proposals

The host owns free-text proposal probability, allowed proposal kinds, fields, parsing, validation,
the editable approval card, and reducer admission. The narrator may provide values for a selected
form but cannot choose an unknown kind, add unknown fields, repair missing mechanics, or mutate a
run.

The safe shape is:

```text
player text
  -> deterministic command parser
  -> recognized command: reducer
  -> otherwise: narration only, optionally host-gated proposal form
  -> player approval
  -> reducer validation and commit
```

## Current Non-Goals

- LLM command parsing or narrator-side action execution
- free text silently becoming a deterministic command
- command memory outside `RunState`
- hidden intent inference
