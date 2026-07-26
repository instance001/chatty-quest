# `v0.2` Milestone Plan

## Purpose

This document defines the first post-`v0.1` build milestone.

Status note:

- this document was refreshed on `2026-07-16`
- lock state, key-driven gated progression, mixed objective conditions, the first barricade pass, and the first noise-pressure pass are already implemented on the current branch
- the current branch also includes clearer player-facing truth surfacing for pressure, route state, and recommended next verbs
- the current branch now also includes small authored garage-finale wrinkles through the `Garage Brute` wounded end phase and a secured-property payoff when both siege lanes are barricaded before the finale
- the remaining `v0.2` work is now mostly about content depth and selective authored escalation rather than proving those foundations from scratch

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

### 1. Deterministic Siege Pressure Expansion

Add one or two content-backed mechanics that deepen the scenario without exploding complexity.

Recommended targets:

- extend the new barricade mechanic beyond one room
- make optional routes change risk in a visible deterministic way
- give the player a clearer "secure the property" loop before the boss finish

Why first:

- the gated route now works, so the next honest gain is making the property feel under siege instead of merely locked
- barricades already prove the state shape, so the next step is richer authored payoff rather than more foundational abstraction

Concrete deliverables:

- at least two authored barricade targets with distinct room flavor
- deterministic pressure suppression or redirection tied to barricade state
- clearer route choice between direct progression and securing vulnerable spaces

Current branch status:

- complete for first pass
- `front_verandah` and `back_garden` now act as distinct siege lanes
- passive pressure, retaliation differences, recovery payoff, and visible route hints are already live

### 2. Second Objective Condition Type

Broaden objective logic beyond "kill this boss."

Recommended shape:

- add one additional deterministic condition family such as:
  - possess required item
  - reach required location after a prerequisite is satisfied
  - survive after resolving a named threat

Why here:

- this system is now live, so the remaining work is clearer truth surfacing and better use of the existing condition model in content and UI

Concrete deliverables:

- better player-facing objective condition visibility
- cleaner diagnostics and character-side condition summaries
- any follow-up content should use existing multi-condition objective truth instead of reverting to single-condition assumptions

Current branch status:

- complete for first pass
- item, boss, and location conditions are all live in content and reducer truth
- objective state is surfaced in the UI and progress lines already fire when tracked condition truth changes

### 3. Better Reducer Feedback And Command Legibility

Expand the command surface only where it improves trust and usability.

Recommended targets:

- clearer rejected-action reasons
- command aliases for barricading and any follow-up siege actions
- richer inspect output for locked, unlocked, barricaded, or pressured state

Why here:

- the chat-forward fantasy gets stronger when the engine can explain deterministic constraints cleanly

Concrete deliverables:

- reducer result lines for new failure/success states
- command parser support for the minimal new verbs required
- updated rolling-summary coverage

Current branch status:

- largely complete for first pass
- explicit `unlock` targeting, `barricade` aliases, richer inspect output, and state-aware success/failure lines are already live
- the desktop shell now also recommends likely next commands instead of leaving the player to guess the route verbs cold

### 4. UI Truth-Surfacing Pass

Reflect the new deterministic rule layer explicitly in the shell.

Recommended targets:

- show locked/unlocked and barricaded state in map / character / diagnostics surfaces
- show utility-item relevance when appropriate
- make pressure-reduction state and objective status more legible

Why here:

- if the player has to infer the new mechanic only from prose, the engine loses its truth-first identity

Concrete deliverables:

- UI derived-model updates
- visible state affordances in the relevant tabs
- diagnostics visibility for the new state family

Current branch status:

- complete for first pass
- map, character, diagnostics, sidebar, and action bar all surface lock state, barricade state, noise state, or route pressure truth
- the current build also includes lightweight threat forecasts for the exposed siege routes

### 5. Finale Identity Pass

Use one or two authored end-state beats to keep the scenario from flattening at the boss room.

Recommended targets:

- give the garage fight a readable last-phase identity
- make the objective room feel different once the boss is wounded or cleared
- keep the finale deterministic and content-specific rather than inventing a generic encounter framework too early

Current branch status:

- first pass complete
- the `Garage Brute` now enters a wounded end phase at low HP, hits harder, and surfaces explicit finale text
- garage room text also reflects that wounded-state escalation so the player can read the phase change instead of only inferring it from damage
- if both `front_verandah` and `back_garden` are barricaded before the garage fight, brute retaliation is reduced by `1` and the sidebar/inspection text surfaces that the property has been secured

### 6. Acceptance Coverage Expansion

Extend automated and manual tests to cover the new mechanic.

Concrete deliverables:

- reducer tests for the new progression mechanic
- save/load tests covering the new state field(s)
- updated acceptance audit and manual sweep docs

Current branch status:

- in progress as documentation sync work
- automated coverage is already expanded; the remaining cleanup is keeping written acceptance/materials aligned with the current branch

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

1. keep `Property Siege Classic` honest as the first RD Engine testbed
2. verify current barricade, noise, objective, route-forecast, and finale behavior through docs, tests, and manual sweep
3. choose the next feature only after deciding whether the pack needs content depth or a new reusable state family
4. prefer small authored beats over broad abstraction unless the current pack is clearly blocked by missing engine shape

## Current Decision Point

The previous best continuation feature was:

- make barricades a real two-route siege system rather than a one-room proof

Current branch status:

- complete for first pass
- both siege lanes have distinct local payoffs
- securing both siege lanes now also reduces `Garage Brute` retaliation by `1`
- the remaining question is whether the next gain should be authored scenario depth or a genuinely new reusable engine capability

## Suggested Current Task Stack

1. finish the written doc/manual sync for the secured-property finale payoff
2. run the refreshed manual sweep against the current branch
3. decide whether the next gain should be another authored content beat or a genuinely new system family
4. prefer scenario-specific depth over broad abstraction unless the current pack is clearly blocked by missing engine shape

Current implementation anchor:

- [docs/V0_2_LOCKED_PROGRESSION_SPEC.md](docs/V0_2_LOCKED_PROGRESSION_SPEC.md)
- [docs/V0_2_OBJECTIVE_CONDITIONS_SPEC.md](docs/V0_2_OBJECTIVE_CONDITIONS_SPEC.md)
- [docs/V0_2_LOCATION_OBJECTIVE_SPEC.md](docs/V0_2_LOCATION_OBJECTIVE_SPEC.md)
- [docs/V0_2_BARRICADE_SPEC.md](docs/V0_2_BARRICADE_SPEC.md)
