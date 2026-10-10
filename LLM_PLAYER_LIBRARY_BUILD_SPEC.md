# Chatty Quest LLM + Player Library Build Spec

## Goal

Make Chatty Quest a gradient game engine. A player can choose strict, deterministic simulation or a permissive narrator-led adventure where an LLM proposes new fiction and the player can make that fiction real.

The governing rule is:

> The LLM may propose. The player chooses. The reducer admits. The runtime remembers.

Once admitted, a generated fact is structured session truth: it is visible, saved, mechanically enforceable, and available to later narration. The LLM never mutates `RunState` directly.

## Execution Contract

> Work through the delivery plan in order.
> Safe, local prerequisites discovered during a step become substeps and should be completed automatically.
> Stop only for external dependencies, destructive/risky ambiguity, or a decision requiring the user.
> Do not stop merely because an intermediate state creates warnings or reveals another necessary local patch.

## Product Model

There are three content sources:

1. **Authored datapack templates**: scenario canon under `assets/datapacks/`.
2. **Session scratch truth**: player-approved additions that exist for one run.
3. **Player library templates/media**: player-owned additions promoted from a session and eligible for compatible future runs.

Authored packs are never modified by narrator or player-library actions.

## Current Implementation

Already implemented:

- host-owned narrator proposal/admission flow with editable approval cards
- typed session and player-library templates for items, effects, NPCs, enemies, bosses, objectives,
  locations, and temporary exits
- stable runtime instances, lifecycle buckets, provenance, save/load support, and generic ownership
  transfer through `TransferItem`
- capability-gated actor contracts shared by NPCs and actor-capable items, including combat,
  perception, movement, tracking, and inventory endpoints
- reducer-confirmed event projection (`type | origin | impact | outcome`) consumed by narration,
  summaries, diagnostics, media, and action traces
- separate live narration and compact bounded-proposal passes, with local llama.cpp, compatible
  local HTTP, cloud BYO-key, and mock backends
- host-owned strictness/proposal gates, ambient proposal controls, template-kind whitelist, and
  deterministic proposal eligibility
- player-library UI for import, tags, enable/disable, removal confirmation, persistence feedback,
  diagnostics, and tagged image/motion/audio media pools
- hierarchical deterministic media selection with provenance and runtime inspection surfaces
- structured effects/tag evaluation for combat, movement, inspection, wait, and failure triggers

Current intentionally deferred capability families:

- quest/reward/haggle contracts
- trade/economy contracts
- dialogue/social contracts
- following, loyalty, personality, magic, and memory contracts

These are host capability gaps, not datapack-specific mechanics. They should be introduced through
the same capability-gated dependent-schema registry when their reducer-backed action paths exist.

## Non-Negotiable Architecture

```text
Player text
  -> command resolver
  -> reducer action
  -> confirmed outcome
  -> bounded narrator context
  -> LLM narration + typed proposals
  -> player confirmation
  -> reducer admission action
  -> canonical run state / optional player-library promotion
```

- Reducers own all canonical mutations.
- Narrator prose is never mechanics by itself.
- A proposal must be schema-valid before display and valid again at admission.
- All persistent additions need stable ids, provenance, tags, a lifetime, and save/load coverage.
- Media is illustrative. Tags and instances create mechanics; media follows them.

## Delivery Plan

### Phase 1: Complete the Player Library UI

1. Add backward-compatibility tests for old JSON missing `enabled` fields.
2. Ensure disabled template entries do not restore into new runs.
3. Ensure disabled media entries do not participate in matching.
4. Add action-result feedback to Library controls instead of silently swallowing persistence errors.
5. Add confirmation before removal.
6. Add a local-file import form for image, audio, GIF, and video assets.
7. Add tag editing for templates and media using simple chips/comma-separated tags.
8. Add enable/disable, edit, remove, and missing-file status rows in Diagnostics.

Acceptance:

- a player can manage their own templates and media without editing a datapack
- all writes stay under `runtime/config/`
- disabled content is retained but inactive
- removed content no longer appears in future runs or media resolution

### Phase 2: Generalize Player Templates

Replace item-only collection semantics with a typed player-template schema.

Required common fields:

- `id`
- `kind`
- `name`
- `description`
- `tags`
- `enabled`
- `source_datapack_id` or compatibility policy
- provenance and admission metadata
- optional media references

Template kinds:

- item / weapon
- status / effect
- NPC
- enemy
- boss
- objective
- location / temporary exit

Keep family-specific mechanics explicit; do not use unstructured prose as a rule language.

### Phase 3: Runtime Instances and Buckets

Introduce first-class run instances where template ids are currently used directly.

Required instance fields:

- `instance_id`
- `template_id` or player-template id
- current bucket/owner
- current location where applicable
- lifecycle status
- source/provenance
- family-specific mutable state

Bucket targets:

- inventory/world item/equipped item
- location NPC bucket
- live/defeated encounter bucket
- active/completed objective bucket
- known/visited/locked map state
- player/entity status-effect bucket

Migration order:

1. items
2. effects/statuses
3. NPCs
4. enemies
5. bosses
6. objectives
7. locations/exits

### Phase 4: Effect and Tag Evaluator

Add a small structured evaluator for admitted mechanics.

First supported rules:

- additive damage/healing modifiers
- target/source/context tag checks
- combat, movement, inspect, wait, and failure triggers
- scene/run/until-used durations
- explicit expiration events

Example:

```text
Giant-Slaying Mountain Dew Knife
  modifier: +10 damage
  target tag: giant
  context tag: mountain_dew
  duration: run
```

The reducer evaluates this rule. The narrator only proposes and describes it.

### Phase 5: Live LLM Adapter

Add an adapter behind the narrator seam.

Inputs:

- existing bounded `NarratorContext`
- active play contract/strictness settings
- relevant session scratch truth
- relevant player-library templates/media tags

The live adapter uses two bounded output lanes rather than asking a model to serialize narration
and mechanics together:

- **Narration lane:** plain presentation text only. It is recoverable per turn and can never
  mutate state.
- **Proposal lane:** runs only after the host-owned gate selects one authorized template kind.
  The model fills that kind's small host-owned `FIELD: value` form; the host parses it into the
  canonical typed proposal, validates it, and opens the editable approval card.

The model never constructs an internal proposal JSON object. A malformed proposal is discarded
without invalidating otherwise successful narration.

Requirements:

- strict host-side form/schema validation
- recoverable timeout/malformed-output handling; proposal failure is turn-local and narration can
  continue on the next turn
- optional local audit log of request/response metadata
- no raw mutable game state or direct tool access
- no claim that an unaccepted proposal became real

### Phase 6: Play Contract and Strictness Presets

Replace the single broad chaos setting with a pack-default, player-overridable play contract.

Controls:

- command interpretation tolerance
- accounting strictness
- failure severity
- narrator tone freedom
- generated flavor permission
- generated persistent-content permission
- generated mechanical-effect permission
- protected invariants

Initial presets:

- Simulation
- Rules First
- Adventure
- Chaos
- Unleashed

Packs may limit the available range and name protected invariants. The engine still records all admitted truth.

### Phase 7: Natural-Language Intent Resolution

Keep this separate from narration.

```text
free player text
  -> deterministic parser or bounded LLM command candidate
  -> reducer validation
  -> confirmed outcome
  -> narration
```

In permissive modes, a resolver may propose an allowed action or request an admission card. It must never bypass the reducer.

### Phase 8: Media Pool Expansion

Media resolution priority:

```text
exact authored context override
-> exact entity media
-> tagged player-library media
-> authored location media
-> datapack fallback
-> engine fallback
```

Add:

- file import and validation
- tag editor
- kind-aware previews
- missing-file warnings
- per-run player-media enable control
- player-library provenance labels in all media views

### Phase 9: Testing and Diagnostics

Add automated coverage for every template family and admission stage:

- accepted proposals create valid instances and buckets
- declined proposals create no canonical truth
- invalid proposals are rejected cleanly
- disabled library entries never spawn or resolve
- accepted truth round-trips through save/load
- effects trigger and expire deterministically
- player-library media selection is stable for equal inputs
- malformed LLM outputs cannot corrupt a run
- diagnostics explain origin, active state, and missing assets

Manual acceptance sweep:

- strict mode admits nothing beyond authored rules
- chaos mode can admit and remember a novel item
- promoted item appears in a compatible new run
- tagged media follows the item without changing mechanics
- removal/disable immediately changes future eligibility

## Definition of Done

The feature is complete when a player can start a permissive run, receive a live LLM proposal, approve it as structured session truth, optionally preserve it in a player library, see tagged player media illustrate it, and encounter it reliably in later compatible runs—while strict simulation mode remains deterministic, narrow, and free of unsolicited generated mechanics.
