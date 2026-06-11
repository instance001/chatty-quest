# Derived View Spec

## Purpose

This document defines the contract for derived views in Chatty Quest and the RD Engine.

It exists to answer one recurring architecture question:

When a UI surface, media surface, diagnostics panel, or future handoff summary is computed from canonical state, what is it allowed to do and what must it never become?

Short answer:

- a derived view may interpret and present truth
- a derived view may not become truth

This document should be read alongside:

- [RUNTIME_MODEL_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/RUNTIME_MODEL_SPEC.md:1)
- [REDUCER_RESULT_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/REDUCER_RESULT_SPEC.md:1)
- [UI_SHELL_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/UI_SHELL_SPEC.md:1)
- [MEDIA_PANEL_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/MEDIA_PANEL_SPEC.md:1)
- [MAP_SYSTEM_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/MAP_SYSTEM_SPEC.md:1)

## Core Rule

Canonical state is authoritative.

Derived views are rebuildable interpretations of canonical state plus tightly scoped support context.

That means:

- if a derived view disappears, truth still exists
- if a derived view is rebuilt, truth should remain the same
- if a derived view becomes stale, it must not silently override canonical truth

Short rule:

- state first
- derived view second

## What Counts As A Derived View

A derived view is any representation built from canonical truth for consumption by another layer.

Examples:

- map tile layouts
- media panel focus state
- inventory display rows
- character summary panels
- diagnostics counters
- recent event summaries
- handoff snapshots
- future compact API payloads

These outputs may be rich and useful.

They are still downstream products of truth, not peer authorities beside it.

## Inputs To Derived Views

Derived views may consume:

- canonical runtime state
- validated datapack content
- reducer-confirmed event output
- support memory where clearly marked non-authoritative
- local UI session state where interaction requires it

Derived views should not consume:

- raw narrator prose as if it were mechanics
- temporary hover or selection state as canonical world truth
- missing media as evidence that game state failed
- guessed future outcomes

## Derived View Layers

Most derived views should be thought of in three layers.

### 1. Truth Inputs

These are the authoritative inputs.

Examples:

- `RunState`
- datapack templates
- reducer events
- setup metadata that affects canonical play

### 2. Derived Model

This is the structured presentation-friendly shape.

Examples:

- `GeneratedMapLayout`
- `MediaPanelState`
- inventory row models
- diagnostics report structs

This layer should be rebuildable.

### 3. Widget Or Transport Surface

This is the final consumption format.

Examples:

- visible UI widgets
- modal viewer requests
- scrollable diagnostics panels
- future handoff packet bodies

This layer is the least authoritative of all.

## Rebuildability Rule

A good derived view should be reconstructible from its authoritative inputs.

Examples:

- the map tile grid can be rebuilt from location truth and media resolution
- media panel focus can be rebuilt from recent events and current runtime state
- diagnostics counters can be rebuilt from event history
- inventory rows can be rebuilt from inventory truth plus item templates

If a feature cannot be rebuilt without consulting stale UI state or prose, that is a design smell.

## Ownership Rule

Derived views should not own gameplay mutations.

Examples of correct ownership:

- reducer changes location
- map view reflects new location

- reducer moves item into inventory
- inventory tab reflects that item

- reducer resolves attack
- media and diagnostics reflect that attack

Examples of incorrect ownership:

- map tile click invents a new legal exit
- media focus implies a boss died when reducer state says otherwise
- diagnostics summary changes objective status

## View Families

### UI-Derived Views

These are player-facing runtime interpretations.

Examples:

- map tiles
- inventory rows
- character status chips
- quick-exit buttons
- game log sections

These views should be optimized for readability and interaction, but they still remain downstream of truth.

### Media-Derived Views

These are presentation-focus interpretations.

Examples:

- current visual focus
- fallback source labels
- asset viewer requests
- cue summaries

These should remain event-aware but state-safe.

### Diagnostics-Derived Views

These are inspection and debugging interpretations.

Examples:

- event counters
- missing media reports
- environment checks
- validation warnings

Diagnostics should help humans reason about truth without becoming a second runtime.

### Handoff-Derived Views

These are exported or staged interpretations for external systems.

Examples:

- bounded run snapshots
- event slices
- review packets

These are derived summaries of truth, not remote truth ownership.

## Good Derived View Properties

A good derived view is:

- rebuildable
- explicit about its inputs
- compact for its purpose
- safe to discard and regenerate
- honest about fallback or approximation

A weak derived view is:

- secretly authoritative
- manually edited into divergence
- dependent on prose scraping for mechanics
- storing duplicate truth without a clear owner

## Duplication Rule

Derived views may duplicate information for readability or performance, but that duplication must be one-way.

Good duplication:

- storing resolved thumbnail paths in map tile models
- storing display-ready labels in diagnostics rows
- storing preview-ready image state in media panel state

Bad duplication:

- storing a second authoritative inventory list in the UI layer
- storing an objective completion flag in both runtime truth and a UI-only source with no owner rule

If truth is duplicated, one copy must be canonical and the other clearly derived.

## Staleness Rule

Derived views may become temporarily stale between recomputations.

That is acceptable if:

- recomputation is predictable
- stale views do not mutate truth
- the user cannot commit illegal mechanics through stale controls

This means:

- a view may lag slightly
- a view may not invent a legal action

## Map Rule

The map is a classic derived view.

Canonical inputs:

- current location
- known and visited locations
- authored connections
- location templates

Derived outputs:

- tile positions
- visibility mode
- connector lines
- movement affordances
- thumbnail choices

The map may suggest movement.

The reducer still decides whether a move is legal.

## Media Rule

The media panel is a classic derived view.

Canonical inputs:

- current location
- local encounters
- focused item or entity context
- recent reducer-confirmed events
- media references and fallback rules

Derived outputs:

- current focus title
- image selection
- source labels
- cue lists
- asset viewer requests

The media panel may reflect the scene.

It may not define the scene.

## Diagnostics Rule

Diagnostics are derived from truth and environment inspection.

Canonical inputs:

- runtime state
- datapack validation results
- recent events
- file presence checks

Derived outputs:

- counters
- missing-media tables
- warnings
- summaries

Diagnostics should make hidden system state legible, not become a hidden system of their own.

## Handoff Rule

Handoff payloads are derived views intended for export or mediation.

Canonical inputs:

- runtime state
- reducer results
- explicit export intent

Derived outputs:

- bounded snapshots
- event slices
- artifact envelopes

The handoff layer is therefore a derived view family with stronger metadata requirements.

## Save Boundary

Save files are not ordinary derived views.

They are restoration artifacts for canonical runtime truth.

That means:

- saves may include support memory
- saves may include some convenience state
- saves must remain authoritative enough to restore play

By contrast:

- diagnostics reports
- media focus state
- UI shell selections
- map layouts

should all remain regenerable rather than save-critical.

## Recommended Implementation Discipline

When adding a new panel, packet, or helper model, ask:

1. what are the canonical inputs
2. what exact derived structure do we want
3. can it be rebuilt
4. can it drift from truth
5. who owns the final mutation if the user acts through it

If the answer to `who owns the mutation` is not `the reducer` or a tightly controlled equivalent, the design likely needs correction.

## Suggested Future Code Direction

As the engine grows, code should increasingly prefer:

- explicit derived-view structs
- view builders or resolvers
- stable derived model boundaries between runtime and widgets

Examples:

- map layout builder
- inventory row builder
- character panel summary builder
- diagnostics report builder
- handoff snapshot builder

This will make future instance migration easier because UI and export surfaces will depend on derived models instead of raw storage internals.

## `v0.1` Guidance

For `v0.1`, derived views should stay:

- simple
- inspectable
- obviously downstream of canonical truth

The goal is not to introduce a heavy presentation architecture.

The goal is to keep the current engine honest as it grows:

- runtime owns truth
- reducer owns mutation
- derived views make truth usable

That boundary is worth making explicit now.
