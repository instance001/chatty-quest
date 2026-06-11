# Handoff Payload Spec

## Purpose

This document defines the future payload contract for Chatty Quest handoff packets.

It exists to answer:

- what a future outward-published state packet should contain
- what metadata should wrap that packet
- what kinds of packets should exist
- what a receiving system may and may not assume
- how handoff stays compatible with deterministic truth ownership

This is a specification for reserved future seams.

It does not authorize live handoff behavior in `v0.1`.

This document should be read alongside:

- [HANDOFF_LANES.md](c:/Users/User/Desktop/chatty-quest/docs/HANDOFF_LANES.md:1)
- [RUNTIME_MODEL_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/RUNTIME_MODEL_SPEC.md:1)
- [REDUCER_RESULT_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/REDUCER_RESULT_SPEC.md:1)
- [INSTANCE_MIGRATION_PLAN.md](c:/Users/User/Desktop/chatty-quest/docs/INSTANCE_MIGRATION_PLAN.md:1)

## Core Rule

Handoff packets are snapshots or artifacts, not remote authority.

Short rule:

- Chatty Quest owns canonical local truth
- handoff packets publish structured copies of selected truth
- receiving systems may observe, review, relay, or stage those copies
- receiving systems do not silently become the owner of the run

This is the payload-level expression of the broader rule:

loose departments, strict handoff contracts

## `v0.1` Rule

`v0.1` does not implement live payload export or import.

What `v0.1` does allow:

- folder reservations
- packet-shape documentation
- payload examples
- explicit future seams in docs

What `v0.1` does not allow:

- automatic background sync
- silent remote mutation
- implicit multiplayer authority
- Chatty-Cog becoming the primary runtime database

## Payload Design Goals

Future handoff payloads should be:

- explicit
- metadata-rich
- copy-oriented
- reducer-compatible
- easy to inspect by a human
- stable enough for tooling and validation

They should not be:

- vague prose blobs
- hidden authority transfer
- UI-state dumps pretending to be canonical gameplay state

## Payload Categories

Handoff packets should be grouped by intent.

### 1. Snapshot Payloads

Used to publish a structured current-state view.

Examples:

- current run summary
- location and objective snapshot
- player state snapshot

### 2. Event Slice Payloads

Used to publish recent structured outcomes.

Examples:

- latest reducer events
- recent combat outcomes
- recent movement history

### 3. Artifact Payloads

Used to wrap real files or export bundles.

Examples:

- save export
- scenario review bundle
- future media request artifact

### 4. Interpretation Payloads

Used to publish guidance or context around another payload.

Examples:

- scenario tone hint
- export purpose
- review notes
- multiplayer relevance tags

Interpretation payloads should never be the sole source of mechanics.

## Envelope Shape

All future handoff payloads should sit inside a common envelope.

Suggested envelope:

```text
HandoffEnvelope
  protocol_version
  packet_id
  packet_kind
  source_module
  destination_kind
  created_at
  scenario_id
  run_id
  payload_version
  tags
  summary
  body
```

This is an envelope concept, not a required `v0.1` struct.

## Envelope Fields

### `protocol_version`

Purpose:

- identifies the handoff contract generation

Why:

- lets future tooling reject packets from incompatible eras cleanly

### `packet_id`

Purpose:

- unique identifier for this packet instance

Why:

- supports review, deduplication, logging, and mediated transfer

### `packet_kind`

Purpose:

- names the packet family

Examples:

- `run_snapshot`
- `event_slice`
- `save_export`
- `media_request`
- `style_reference`

### `source_module`

Purpose:

- says which module authored the packet

Examples:

- `chatty_quest`
- future `chatty_cog`
- future `chatty_art`
- future `chatty_lora`

### `destination_kind`

Purpose:

- states the intended receiving lane

Examples:

- `chatty_cog`
- `chatty_art`
- `chatty_lora`
- `peer_review`
- `local_archive`

### `created_at`

Purpose:

- gives packet creation time

Why:

- helps ordering, auditing, and review

### `scenario_id`

Purpose:

- ties the packet to authored scenario identity

Why:

- lets receivers understand which content contract the packet belongs to

### `run_id`

Purpose:

- ties the packet to a specific run instance

Why:

- future multiplayer relay and review lanes need per-run identity separate from scenario identity

### `payload_version`

Purpose:

- version for the body schema of this packet kind

Why:

- allows packet families to evolve independently of the top-level protocol

### `tags`

Purpose:

- lightweight routing and filtering hints

Examples:

- `combat`
- `objective`
- `location_shift`
- `media_candidate`
- `review`

### `summary`

Purpose:

- small human-readable description for dashboards and manual review

Examples:

- `Run snapshot at Garage after boss encounter`
- `Media request for location-forward kitchen scene`

### `body`

Purpose:

- packet-kind-specific structured payload

Important rule:

- the body should remain structured enough that another system does not need to scrape prose to recover meaning

## Run Snapshot Payload

The most important future payload is the bounded run snapshot.

Suggested shape:

```text
RunSnapshotPayload
  runtime_schema_version
  datapack_id
  datapack_display_name
  current_location
  player_state
  objective_state
  location_state
  encounter_state
  inventory_state
  important_flags
```

This should be intentionally compact.

It is not meant to serialize the entire save file every time.

## Run Snapshot Field Guidance

### `runtime_schema_version`

States which runtime-model contract the snapshot follows.

### `current_location`

Should include:

- current location id
- display name
- maybe a short description
- maybe known connected exits

### `player_state`

Should include:

- HP and max HP
- equipped item summary
- future high-signal stats only

### `objective_state`

Should include:

- active objective id
- active objective name
- completion state
- high-signal target summary if useful

### `location_state`

Should include only the relevant compact truth.

Examples:

- known location count
- visited location count
- maybe current local threats

It should not try to become a full UI map dump by default.

### `encounter_state`

Should include:

- local live enemies
- local live bosses
- maybe recently defeated encounter ids if packet intent requires it

### `inventory_state`

Should include:

- current carried items summary
- equipped item summary
- future instance ids only when needed

### `important_flags`

Examples:

- objective complete
- run terminal state
- scenario boundary mode
- future sync-safe scenario flags

## Event Slice Payload

Event slices should carry recent reducer-confirmed events, not inferred fiction.

Suggested shape:

```text
EventSlicePayload
  action_context
  events
  ui_lines
  event_count
  terminal_state?
```

This packet is useful for:

- Chatty-Cog review surfaces
- future peer relay
- future summary enrichment

Important boundary:

- event slices may explain what just happened
- event slices do not replace canonical local state

## Artifact Payload

Artifact packets should wrap a file or export bundle with metadata.

Suggested shape:

```text
ArtifactPayload
  artifact_kind
  file_path
  file_hash?
  source_runtime_version?
  review_notes?
```

Examples:

- save export
- future datapack review export
- future media request bundle

## Media Request Payload

Future Chatty-Art requests should be explicit and scenario-aware.

Suggested shape:

```text
MediaRequestPayload
  request_kind
  scenario_id
  run_id?
  focus_template_id
  context_location_id?
  display_role
  preferred_tags
  existing_reference_paths
  request_summary
```

This stays aligned with current datapack media logic:

- focused thing first
- location context second
- no implicit state mutation

## Style Reference Payload

Future Chatty-Lora or style-adjacent packets should remain descriptive, not authoritative.

Suggested shape:

```text
StyleReferencePayload
  scenario_id
  style_family
  world_tone
  visual_keywords
  source_paths
  notes
```

This is for style continuity, not gameplay truth.

## Import Rules

Receiving a packet should never mean silently trusting it as canonical local truth.

Any future import flow should be:

- explicit
- validated
- user-confirmed where relevant
- reducer-mediated if it would affect gameplay state

Short rule:

- packets may propose or relay
- the host engine decides what, if anything, becomes local truth

## Multiplayer Direction

Future multiplayer-aligned exchange should use handoff-safe packets, not full authority transfer.

Recommended direction:

- Chatty Quest publishes bounded state packets
- Chatty-Cog mediates peer-to-peer transport and approvals
- another Chatty Quest host may receive compatible packets
- received packets are staged as external state inputs, not immediate canonical overrides

This preserves the rule that Chatty Quest owns its own reducer-confirmed run state.

## Save Boundary

Save files and handoff packets should remain distinct.

Save file:

- full local restoration artifact

Handoff packet:

- bounded exchange artifact or snapshot

A handoff packet may reference a save export artifact, but it should not be assumed to be a full save by default.

## UI Boundary

Handoff packets should not include accidental UI noise unless the packet kind specifically requires it.

Do not treat the following as default packet material:

- selected tab
- widget state
- hover state
- open viewer state
- temporary draft input text

Those are local shell concerns, not shared canonical run truth.

## Narrator Boundary

Packets may include:

- reducer-confirmed UI lines
- compact summary strings
- capsule or tone hints

Packets should not include:

- narrator prose as the only truth source
- freeform story claims that bypass structured runtime state

## Validation Rules

A future packet validator should check:

- required metadata fields present
- valid packet kind
- scenario id present when required
- run id present when required
- body shape matches `packet_kind`
- payload version supported
- referenced template ids are syntactically valid

Higher-trust validation may also check:

- datapack compatibility
- runtime schema compatibility
- instance id format validity once instance migration happens

## Evolution Rules

When the handoff contract evolves:

- add new packet kinds rather than overloading old ones where practical
- version body schemas explicitly
- preserve backwards-readable metadata where possible
- avoid turning one packet into a kitchen sink

Better:

- several small packet kinds

Worse:

- one giant packet that tries to carry all possible future exchange needs

## `v0.1` Guidance

For `v0.1`, the correct move is documentation only.

That means:

- no active bridge code
- no hidden sync assumptions
- no packet writers running in the background

The value right now is having a clean contract ready before future integration work begins.
