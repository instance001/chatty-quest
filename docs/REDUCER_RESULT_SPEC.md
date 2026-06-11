# Reducer Result Spec

## Purpose

This document defines the contract for reducer results in Chatty Quest and the RD Engine.

It answers:

- what goes into an action
- what comes out of the reducer
- what is canonical versus descriptive
- what downstream systems may consume
- what future seams should be preserved for narrator, media, diagnostics, saves, and handoff

This is the implementation-facing companion to:

- [ARCHITECTURE.md](c:/Users/User/Desktop/chatty-quest/docs/ARCHITECTURE.md:1)
- [RUNTIME_MODEL_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/RUNTIME_MODEL_SPEC.md:1)
- [STATE_AND_BUCKETS.md](c:/Users/User/Desktop/chatty-quest/docs/STATE_AND_BUCKETS.md:1)

## Core Rule

The reducer owns state mutation.

The reducer result owns the structured description of what just happened.

The narrator, media layer, diagnostics surface, rolling summary, and UI log should all consume reducer-confirmed outputs instead of inferring canon independently.

Short rule:

- reducer mutates truth
- reducer result describes truth
- presentation layers decorate truth

## Current `v0.1` Shape

The current implementation uses:

- `GameAction`
- `GameEvent`
- `ActionOutcome { events, lines }`

This is already the correct basic direction.

It means:

- actions are structured input
- state changes happen inside the reducer
- emitted events are structured output
- `lines` are human-facing summary text for immediate display

This spec blesses that direction while defining how it should evolve cleanly.

## Action Input Contract

The reducer should receive:

- the current canonical runtime state
- the validated datapack bundle or scenario content view
- one structured action

The reducer should not receive:

- raw narrator prose as an instruction source
- UI-only hover state
- media selections treated as mechanics
- summary text treated as truth

## Result Contract

A reducer result should communicate four different kinds of information clearly.

### 1. Canonical Mutation Result

This is the actual change in truth already committed to runtime state.

In the current codebase, this is represented by the mutated `RunState` plus the emitted `GameEvent` list.

Future-friendly result fields may include:

- success or rejected status
- primary committed event
- additional secondary events
- explicit outcome classification

### 2. Structured Event Stream

Events are the machine-readable record of what occurred.

Current examples:

- `Moved`
- `LocationLooked`
- `ItemTaken`
- `ItemEquipped`
- `ItemUsed`
- `AttackResolved`
- `DamageTaken`
- `ObjectiveCompleted`
- `RunWon`
- `RunLost`
- `ActionRejected`

This layer should be the main input for:

- narrator adapters
- media focus selection
- diagnostics counters
- rolling summary updates
- future analytics or replay support

### 3. Immediate Player-Facing Lines

The reducer may emit concise human-facing lines for immediate UI display.

Current examples:

- `You move to Kitchen.`
- `You take the Medkit.`
- `You win.`

These lines are useful, but they are not canonical state.

They are:

- display-ready
- support-friendly
- replaceable by richer narrator presentation later

### 4. Support Side Effects

The reducer may trigger tightly controlled support updates after canonical truth is settled.

Current example:

- rolling summary line generation from reducer-confirmed events and lines

Important rule:

- support side effects may follow truth
- support side effects may not create truth

## Recommended Result Envelope

The long-term mental model should be:

```text
ReducerResult
  status
  events
  ui_lines
  warnings
  summary_inputs
  media_hints
  narrator_payload
```

`v0.1` does not need every field yet, but this is the clean direction.

### Status

Possible future values:

- committed
- rejected
- blocked
- partial

Why:

- makes downstream behavior clearer than inferring success from event shape alone

### Events

This remains the canonical structured record of what occurred.

### UI Lines

These are short human-facing messages suitable for immediate display in a deterministic shell even before narrator styling.

### Warnings

These are non-fatal concerns worth surfacing to diagnostics or tooling.

Examples:

- fallback path used
- scenario content resolved imperfectly
- action accepted but presentation asset missing

### Summary Inputs

This is the clean seam for future rolling-summary updates.

Rather than asking the summary system to scrape prose, the reducer can provide structured summary-worthy signals.

### Media Hints

This is the clean seam for future event-to-media binding without asking the media system to reverse-engineer intent from arbitrary text.

Examples:

- location changed
- combat target focused
- item inspected
- objective completed

### Narrator Payload

This is the clean seam for future real narrator adapters.

It should describe:

- what action was attempted
- what actually happened
- what the current local scene now is
- what emotional or stylistic framing is appropriate from datapack capsules

## Event Design Rules

Events should be:

- structured
- explicit
- mechanically meaningful
- stable enough for downstream consumers

Events should not:

- require parsing prose to recover the basic fact
- hide mechanically relevant details only in display lines
- collapse multiple materially different results into one vague event unless the payload disambiguates them

Good event design:

- `ItemUsed { item_id, effect }`
- `AttackResolved { target_id, target_kind, damage, defeated }`

Weak future event design to avoid:

- `SomethingInterestingHappened`
- `CombatChanged`
- `InventoryUpdated`

Those are too vague for media, diagnostics, or future handoff consumers.

## Result Ownership Boundaries

### The Reducer May Own

- mutation legality
- committed state changes
- structured event emission
- deterministic UI-support lines
- summary-support signals

### The Narrator May Own

- stylistic phrasing
- tone
- cadence
- emotional framing
- humour or menace

### The Narrator May Not Own

- deciding whether the state actually changed
- creating hidden mechanics
- promoting flavour text into canonical truth

### The Media Layer May Own

- choosing a focus from reducer-confirmed context
- resolving assets from authored references
- falling back safely

### The Media Layer May Not Own

- inventing gameplay events
- changing canonical outcomes
- treating absent assets as state failure

### Diagnostics May Own

- counting events
- surfacing warnings
- showing recent outcomes
- flagging missing references or suspicious state

### Diagnostics May Not Own

- deriving new mechanics
- mutating runtime truth

## Rejections And Blocked Outcomes

Rejected or blocked actions should still produce a structured result.

Current good examples:

- `ActionRejected { reason }`
- `MovementBlocked { attempted_destination }`

This matters because downstream systems still need to know:

- the player attempted something
- the attempt failed
- why it failed
- what should or should not be narrated

A rejected action should not quietly disappear just because it caused no canonical mutation.

## Win And Loss Contract

Run completion should always be expressed structurally before it is celebrated stylistically.

Current good direction:

- objective completion emits `ObjectiveCompleted`
- win emits `RunWon`
- death emits `RunLost`
- UI lines can still say `You win.` or `You lose.`

This contract is important because:

- UI banners can trust it
- save/load can trust terminal state
- diagnostics can count it
- future handoff logic can respect it

## Rolling Summary Contract

Rolling summary should consume reducer-confirmed material only.

Good inputs:

- event stream
- deterministic outcome lines
- future summary payloads

Bad inputs:

- narrator improvisation treated as fact
- media captions treated as state

If a future summary system needs more nuance, that nuance should be added as structured reducer output rather than scraped from prose.

## Media Contract

Media focus should be driven by reducer-confirmed context.

Good focus triggers:

- movement committed
- item inspected
- attack resolved
- objective completed
- waiting in a location

This keeps the media lane attached to truth instead of turning it into a second engine.

## Save And Replay Contract

Save files restore canonical state, not reducer results.

However, reducer results should remain stable enough that future systems can use them for:

- diagnostics history
- richer summary generation
- optional replay logs
- peer handoff context slices

This means result structures should be treated as an interface, not as throwaway implementation trivia.

## Handoff-Friendly Direction

Future Chatty-Cog integration will likely want structured recent outcome context.

Useful handoff-safe reducer result material:

- action type
- structured events
- committed success or rejection status
- local scene consequences
- objective progress implications

Not useful as a sole source:

- raw prose
- media-only decisions
- UI widget state

## `v0.1` Guidance

For `v0.1`, the reducer result system should remain:

- explicit
- small
- deterministic
- serializable where useful
- easy to inspect in diagnostics

It does not need a heavyweight command bus.

It does need:

- stable structured events
- clean ownership boundaries
- honest separation between truth and presentation
- a visible path toward richer result envelopes later

That is enough to keep the engine clean while the scenario set remains small.
