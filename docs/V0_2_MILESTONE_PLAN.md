# `v0.2` Milestone Plan

## Purpose

This document defines the first post-`v0.1` build milestone.

`v0.1` proved:

- one datapack can load
- one deterministic run can be generated and played
- the UI shell works
- the narrator seam exists without owning truth
- save/load and diagnostics are trustworthy enough to support iteration

`v0.2` should not try to become "the full engine."

Instead, it should prove the next honest thing:

- the RD Engine can support richer deterministic scenario expression without collapsing back into hardcoded one-pack assumptions

## Milestone Theme

`v0.2` theme:

- broaden scenario depth without breaking truth ownership

In practical terms, that means:

- make `Property Siege Classic` mechanically richer
- generalize the engine where that richer content demands it
- avoid features that only exist as future-ecosystem fantasies

## `v0.2` Success Standard

`v0.2` is successful if all of the following become true:

- the existing scenario supports a richer deterministic route than simple move / take / attack / win
- at least one new stateful gameplay rule is driven by datapack content rather than hardcoded assumptions
- the reducer and UI remain understandable after that expansion
- save/load and diagnostics still hold up after the new rule layer is added

## Recommended `v0.2` Scope

### 1. Deterministic Item And State Expansion

Add one or two content-backed mechanics that deepen the scenario without exploding complexity.

Recommended targets:

- `locked` / `unlocked` location state
- keys or key-adjacent utility items with a real deterministic job
- a simple gated progression beat

Why first:

- `house_keys` currently read as emotionally important but mechanically inert
- the engine should prove that item meaning can move beyond damage/healing without needing a full RPG system

Concrete deliverables:

- runtime state for lock/unlock or equivalent gated progression
- reducer support for a legal state-changing use of a utility item
- datapack-defined scenario rule or location flag that drives the gate
- visible UI feedback when the gate changes state

### 2. Second Objective Condition Type

Broaden objective logic beyond "kill this boss."

Recommended shape:

- add one additional deterministic condition family such as:
  - possess required item
  - reach required location after a prerequisite is satisfied
  - survive after resolving a named threat

Why here:

- this is the smallest real proof that objectives are becoming engine features rather than one-off scenario scripts

Concrete deliverables:

- objective data extension in datapacks
- reducer-visible completion checks
- migration/update of current objective handling without breaking `v0.1`

### 3. Better Reducer Feedback And Command Legibility

Expand the command surface only where it improves trust and usability.

Recommended targets:

- clearer rejected-action reasons
- command aliases for the new gated progression mechanic
- slightly richer inspect output for locked, unlocked, or gated state

Why here:

- the chat-forward fantasy gets stronger when the engine can explain deterministic constraints cleanly

Concrete deliverables:

- reducer result lines for new failure/success states
- command parser support for the minimal new verbs required
- updated rolling-summary coverage

### 4. UI Truth-Surfacing Pass

Reflect the new deterministic rule layer explicitly in the shell.

Recommended targets:

- show locked/unlocked or gated state in map / character / diagnostics surfaces
- show utility-item relevance when appropriate
- make objective status more legible if multiple condition types exist

Why here:

- if the player has to infer the new mechanic only from prose, the engine loses its truth-first identity

Concrete deliverables:

- UI derived-model updates
- visible state affordances in the relevant tabs
- diagnostics visibility for the new state family

### 5. Acceptance Coverage Expansion

Extend automated and manual tests to cover the new mechanic.

Concrete deliverables:

- reducer tests for the new progression mechanic
- save/load tests covering the new state field(s)
- updated acceptance audit and manual sweep docs

## Explicit Non-Goals For `v0.2`

Do not pull these in unless they become directly necessary:

- real LLM integration
- multiplayer
- Chatty-Cog runtime coordination
- Chatty-Art generation requests
- Chatty-Lora training or style pipelines
- fully generalized NPC systems
- advanced procedural world generation
- full natural-language understanding
- broad class/stat/skill systems
- multi-datapack campaign support

## Recommended Build Order

1. extend datapack schema for the first new deterministic mechanic
2. extend `RunState`
3. extend reducer and action parsing
4. update `Property Siege Classic` content to use the new mechanic
5. surface the new state in UI derived models and views
6. add automated tests
7. update manual sweep and acceptance audit

## Best Candidate First Feature

If we want the cleanest next move, the best `v0.2` starter feature is:

- make `house_keys` unlock a gated location or progression state

Why this is the best starter:

- the content already wants it
- the player immediately understands it
- it exercises templates, runtime state, reducer logic, UI feedback, save/load, diagnostics, and acceptance coverage all at once
- it is rich enough to prove engine growth without creating a system explosion

## Suggested First Task Stack

1. Add deterministic lock state to `RunState`
2. Add datapack support for locked locations or equivalent gate metadata
3. Teach the reducer to unlock that gate when the correct item is used
4. Update `Property Siege Classic` so `house_keys` matter mechanically
5. Surface the gate in UI and diagnostics
6. Add tests for locked movement, unlocking, persistence, and objective flow
