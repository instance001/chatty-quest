# Narrator Admission Spec

## Purpose

This contract lets a selected narrator offer novel, bounded session content without becoming an
untracked source of game truth.

Short rule:

- the host owns blank bounded forms
- the narrator fills values only
- the player edits, accepts, or rejects
- the reducer validates and commits

## Authority Boundary

Reducers own canonical game truth. A narrator may write presentation prose for a player turn, but
that prose cannot create an item, alter a location, or otherwise mutate the run. Ordinary free text
is always allowed to receive narration without creating mechanics.

Only a validated proposal that the player accepts can dispatch an admission action. Rejection or a
turn-local model failure leaves no canonical state behind. Authored datapacks are never modified.

## Host-Owned Proposal Flow

For free text that did not resolve as a reducer-owned command:

1. The host sends bounded context and the player's text through the plain-text narration lane.
2. The host classifies whether the input is a proposal-worthy constructive/discovery intent, then
   independently evaluates the deterministic proposal gate and authorized template set.
3. If permitted, the host reproducibly selects exactly one authorized proposal kind. The narrator
   does not route conversational input into a mechanic or choose the kind.
4. The host supplies only that kind's small flat field form.
5. The narrator returns labelled values, not JSON or an internal proposal object.
6. The host parses syntax, validates the filled form, and shows an editable approval card.
7. The player accepts or rejects. Accepted data goes to the reducer for final validation and commit.

The host may normalize syntax (for example, trimming a value or parsing an integer). It must not
invent omitted mechanics, repair a proposal by guessing intent, or admit unknown fields/kinds.
Conversational expression such as an outburst at scenery fails the host intent check and remains
narration-only even at Wild; this check prevents a model from treating every utterance as a request
to pull a proposal lever.

## Proposal Controls

Narration-event checkboxes decide **when narration runs**. They are separate from proposal
admissibility. The host owns proposal probability for eligible free-text opportunities:

- Tight: 0%
- Guided: 25%
- Loose: 60%
- Wild: 100%

When Wild's gate passes, the narrator is required to attempt a proposal from the currently
authorized form set. If no compatible form exists, diagnostics say so explicitly. All levels retain
the same authority boundary: no direct narrator mutation.

Separately, the host can open a rare ambient-discovery form after reducer ticks (for example, a
contextual discovery the player did not explicitly request). This uses a persisted, deterministic
cooldown and a deliberately small host-owned chance: Guided 2% after 12 ticks, Loose 4% after 8,
and Wild 6% after 6. Tight has none. The host selects the authorized kind; the narrator only fills
the form, which still requires player approval and reducer validation.

The selected play contract controls which bounded kinds can be offered. Current supported runtime
kinds include items, statuses, NPCs, enemies, bosses, objectives, and temporary exits. Every kind
remains subject to its template and reducer validation; a temporary exit, for example, must use
host-known locations rather than inventing map ids.

Before a run, the player also sets a persisted **Random emergence allowed** whitelist. It is an
additional host-side filter over the play contract: an unchecked kind cannot be selected for either
free-text or ambient proposals. Chaos controls how frequently the host rolls for opportunities;
the whitelist controls what kinds may materialize when a roll succeeds.

## Approval, Persistence, And Failure

Approval cards expose their allowed fields for player editing before admission. An accepted
run-scoped proposal is saved as normal `RunState` truth. Where the UI offers it and the selected
play contract allows it, a validated item may also be saved to the player-owned personal narrator
collection at `runtime/config/personal_narrator_collection.json`; this remains separate from
authored datapacks.

The Player Library may also attach optional exact image, motion, and audio paths to a saved
template. Exact template media wins over a tagged player-media pool; leaving a field blank keeps
the template on the deterministic pool-selection path. Media remains presentation-only in both
cases, and a missing exact file is surfaced as a missing asset rather than becoming mechanics.

Narration and proposal generation are separate lanes. A malformed, incomplete, or unavailable
proposal response is discarded for that turn and logged as such; it does not disable a working
narrator or turn successful narration into a game failure. Likewise, a narration failure is
recoverable on a later turn. Diagnostics distinguish gate decisions, kind selection, parse/field
failures, retries, admission decisions, and reducer outcomes.

Selected live narrator work runs after the reducer commit on a single background presentation
worker. The desktop UI remains responsive while it waits; the host temporarily holds the next
command until that worker returns so narration, proposal queues, and action traces retain turn
order. This worker never owns or delays the committed state transition.

## Effective Capability Contract

The approval card is not limited to the narrator-filled name and description. The host first
canonicalizes an effective mechanical contract: family mechanics plus every applicable host
capability and parameter. Missing narrator fields become host defaults and remain visible rather
than silently disappearing. Item and NPC cards expose their shared actor envelope (health, combat,
hearing, sight, movement, inventory, noise spawning/tracking, sight tracking, and disposition) as an editable
capability editor. The host validates prerequisites and the reducer receives the resulting
`ActorState`; accepted items/NPCs and saved player-library templates retain that same component.

Capability roots gate their dependent schema. The host registry, rather than the narrator or an
entity noun, determines which dependent fields are applicable. The actor editor therefore reveals
HP only beneath health and damage only beneath combat; tracking branches are unavailable until
their perception and movement prerequisites are enabled. Future proposal/mutation contracts must
use this same pattern: enable a capability, resolve newly required fields through host defaults or
a bounded form, validate, then admit. Disabling it retracts its dependent mechanics.

Other proposal families display their locked, currently implemented contract rather than pretending
to expose capabilities the runtime cannot yet configure. This keeps “No” meaningful and makes a
missing engine mechanic a host capability gap, not a narrator promise.

## Player-Owned Narrator Configuration

The title screen restores non-secret narrator choices through
`runtime/config/title_setup.json`, including backend, local model or hosted provider/model/endpoint,
narration preferences, and play controls. API keys are process-memory only and are never written to
title configuration, saves, diagnostics, or datapacks.
