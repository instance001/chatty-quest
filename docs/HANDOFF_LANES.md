# Handoff Lanes

## Purpose

This document defines the future handoff seams that Chatty Quest should reserve without implementing them in `v0.1`.

The goal is to keep Chatty Quest compatible with the broader Chatty ecosystem shape while preserving a clean rule:

- Chatty Quest owns its own UI, deterministic run state, and scenario logic
- Chatty ecosystem tools may later observe, mediate, route, or enrich that state
- Chatty Quest should not become the orchestrator for the ecosystem

## Core Boundary

Chatty Quest is a game engine module, not the room.

In the broader ecosystem:

- Chatty Quest is the game department
- ChattyCog is the room, switchboard, and orchestrator
- future handoff lanes are the wires

This mirrors the pattern seen in the reference projects:

- each tool owns its own real state
- handoffs are explicit
- handoffs are mediated
- handoffs are metadata-rich
- handoffs are user-confirmed

## `v0.1` Rule

`v0.1` does not implement live handoff behavior.

Allowed in `v0.1`:

- reserved folders
- documented packet expectations
- placeholder examples if useful

Not allowed in `v0.1`:

- active ChattyCog bridge code
- live Chatty-Art requests
- live Chatty-Lora requests
- multiplayer transport
- silent background synchronization

## Handoff Categories

Future Chatty Quest handoffs should be separated into distinct categories.

### Artifact Handoff

For real files or copyable payloads.

Examples:

- generated save exports
- reviewed scenario packs
- optional future media request packets
- optional future state export bundles

### Interpretation Handoff

For meaning, advice, summaries, and compatibility hints.

Examples:

- what a save or scenario packet represents
- whether a run is approved for export
- intended scenario tone
- suggested downstream usage

### Shared State Publication

For structured state that Chatty Quest deliberately publishes outward.

Examples:

- current run summary
- current scenario identity
- compact player/map/objective snapshot
- future multiplayer-facing state packets

This lane is especially important because it should not be confused with direct ownership transfer. Publishing state outward is not the same thing as giving another system permission to mutate the run freely.

## Chatty-Cog Lane

### Future Role

ChattyCog is the future orchestration and mediation layer for Chatty Quest.

Likely responsibilities:

- hosting or docking the game as a module
- receiving outward-published game summaries
- mediating approved handoffs
- logging and digesting play-state events
- staging sandbox review artifacts when needed
- supporting future peer-to-peer coordination between ChattyCog instances

### Multiplayer Direction

Future multiplayer should not be implemented as Chatty Quest owning the entire network stack or cross-machine coordination layer.

Instead, the intended shape is:

- Chatty Quest owns its current game state
- Chatty Quest can publish structured game-state packets
- Chatty Quest can receive structured external state packets through approved seams
- ChattyCog handles the peer-to-peer, handshake, and wireless connection layer between ChattyCog hosts
- ChattyCog mediates which state packets are shared, received, or staged

In other words:

Chatty Quest should eventually be able to hand off and receive relevant game-state information, but the broader multiplayer wiring belongs to ChattyCog's peer-to-peer system rather than to the game engine itself.

### Recommended Future State Publication Shape

If state publication is added later, it should remain explicit and bounded.

Examples of reasonable outward-published data:

- scenario ID
- run ID
- player location summary
- objective summary
- health or status summary
- important world flags
- compact event summary

Examples of what should not be assumed:

- another host silently taking over local truth
- another system directly mutating private run internals without reducer rules
- the bridge becoming the primary save system

### Reserved Folder Intent

Future reserved lane:

`handoff/chatty_cog/`

Suggested shape:

- `inbox/`
- `outbox/`

These folders are placeholders in `v0.1`, not active runtime requirements.

## Chatty-Art Lane

### Future Role

Chatty-Art is the likely future media-generation and review sibling for Chatty Quest.

Likely future uses:

- location art requests
- enemy or boss portraits
- item art requests
- ambience image or audio prompts
- sandbox review of visual assets

### Boundary Rule

Chatty Quest should not generate media internally as part of `v0.1`.

Instead, later integration should follow the sibling-project pattern:

- Chatty Quest requests or stages an explicit artifact
- ChattyCog mediates the handoff
- Chatty-Art owns generation
- Chatty Quest later receives approved outputs through explicit lanes

### Reserved Folder Intent

Future reserved lane:

`handoff/chatty_art/`

Suggested shape:

- `requests/`
- `outputs/`

These remain placeholders in `v0.1`.

## Chatty-Lora Lane

### Future Role

Chatty-Lora is the likely future style, training, and dataset sibling for Chatty Quest.

Likely future uses:

- scenario-specific style metadata
- optional style-pack curation for scenario art
- future training packet preparation
- compatibility hints for world-specific visual identity

### Boundary Rule

Chatty Quest should not train LoRAs, curate datasets, or own style-training workflows in `v0.1`.

Later integration should preserve the same ecosystem pattern:

- Chatty Quest exports explicit requests or metadata
- ChattyCog mediates
- Chatty-Lora owns training-oriented workflows
- approved style-related outputs can later be handed back explicitly

### Reserved Folder Intent

Future reserved lane:

`handoff/chatty_lora/`

Suggested shape:

- `style_refs/`
- `outputs/`

These remain placeholders in `v0.1`.

## Shared-State Philosophy

If Chatty Quest later publishes state outward, that state should remain:

- intentional
- reducer-compatible
- easy to inspect
- easy to validate
- separate from UI text and narration prose

The bridge should carry structured snapshots when useful, but it should not become Chatty Quest's primary live database.

This matches the broader reference-project principle:

loose departments, strict handoff contracts

## Packet Guidance

Future handoff packets should be:

- explicit
- copy-oriented or snapshot-oriented
- metadata-rich
- easy to reason about

Useful metadata may include:

- source module ID
- destination kind
- packet kind
- scenario ID
- run ID
- summary
- tags

The deeper payload contract is documented in `docs/HANDOFF_PAYLOAD_SPEC.md`.
- timestamp
- compatibility notes
- user note where relevant

## Anti-Cluster Rules

Future integration should avoid:

- direct hidden coupling to another tool's internal file layout
- turning shared state into a junk drawer
- silent auto-import of foreign state
- giving narration systems authority over canonical truth
- putting multiplayer ownership into the game engine itself

These rules matter because the ecosystem works best when each tool stays a real tool rather than dissolving into one giant shared runtime.

## Short Summary

Chatty Quest should eventually be able to:

- publish its own state outward
- receive approved external state packets
- request or consume sibling-tool artifacts through explicit lanes

But it should not own the broader orchestration or peer-to-peer fabric.

That future belongs to ChattyCog.
