# Rolling Summary Spec

## Purpose

This document defines the rolling-summary contract for Chatty Quest and the RD Engine.

The rolling summary is support memory.

It is not canonical truth.

Short rule:

- reducer confirms events
- rolling summary records compact support memory
- future narrators or handoff packets may read it
- mechanics never depend on it as the only truth source

## Current Implementation

The current implementation stores rolling summary lines on `RunState`.

Reducer actions update that memory through a single helper in `src/game/reducer.rs`.

Current behavior:

- summary lines are generated from `GameEvent` first
- reducer output lines are used only as fallback when no event summary exists
- active-run and epilogue actions both update the summary
- the summary is capped at `24` lines to keep saves and future context payloads bounded
- deterministic percentage rolls and random-feeling choices do not use summary length as entropy; accepted turns advance `RunState.turn_index` for replayable roll variation

## Allowed Inputs

Rolling summary may consume:

- reducer-confirmed `GameEvent` values
- deterministic reducer lines when no structured event summary exists
- future reducer-authored summary payloads

Rolling summary may not consume:

- narrator improvisation
- media captions
- UI hover state
- raw player text as if it were confirmed outcome

## Allowed Uses

Rolling summary may support:

- player recap in the `Character` tab
- future narrator continuity
- current narrator context payloads
- future handoff/export summaries
- debugging or manual sweep context

Rolling summary may not decide:

- current location
- turn index or deterministic RNG cursor
- HP
- inventory contents
- lock state
- barricade state
- objective completion
- enemy or boss state

If a summary fact becomes mechanically important, it must be promoted into structured state or reducer output.

Related rule:

- `turn_index` is canonical run state
- `rolling_summary` is bounded support memory
- changing the summary cap must not change combat, noise, sight, pathing, or hazard-break outcomes

## Bounded Memory Rule

The current cap is `24` lines.

Why:

- saves stay small
- future context packets stay compact
- stale support memory naturally falls away
- downstream readers are nudged toward recent reducer-confirmed outcomes

The cap may change later, but it should remain explicit and tested.

## Epilogue Rule

Epilogue actions still produce summary memory.

That matters because post-win exploration can be useful for:

- screenshots
- modder-authored aftermath content
- future handoff/export context
- future narrator continuity

Epilogue summary entries still do not reopen combat, pressure, resource mutation, or objective truth.

## Future Direction

A future bookkeeper or LLM-adjacent helper should treat rolling summary as one input among many.

The safer shape is:

```text
RunState truth
  + recent reducer events
  + bounded rolling summary
  -> narrator context or handoff snapshot
```

The unsafe shape is:

```text
rolling summary prose
  -> inferred mechanics
```

That unsafe direction is explicitly out of bounds.
