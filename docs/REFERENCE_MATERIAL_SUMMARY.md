# Reference Material Summary

## Purpose

This document summarizes what was learned from the read-only reference projects in `reference_material/`.

These projects exist to clarify future ecosystem direction.

They do not authorize:

- copying code into Chatty Quest
- making `v0.1` depend on those repos
- implementing their workflows inside this game project

## Summary Method

The reference review focused on:

- top-level README and handshake materials
- folder structure
- handoff and bridge intent
- boundaries between tool-owned state and mediated exchange

The goal was to understand future integration seams, not to recreate sibling systems here.

## Chatty-Cog

### Likely Future Role

ChattyCog is the coordination layer of the ecosystem.

Its role is best understood as:

- meeting room
- orchestrator
- switchboard
- shared review space
- mediator for explicit handoffs

It is not the place where every module's real domain state should live.

### What The Reference Material Implies

The handoff docs strongly emphasize:

- loose departments
- strict handoff contracts
- explicit user-confirmed transfers
- copy-only artifact exchange
- metadata-rich envelopes
- clear separation between artifact transfer and interpretation/context

It also explicitly frames ChattyCog as the natural place for future room, network, and multiplayer extensions.

### What That Means For Chatty Quest

Future Chatty Quest integration should likely treat ChattyCog as:

- the mediator for exported game-state packets
- the mediator for future save/export bundles
- the future peer-to-peer coordination layer for multiplayer-style state relay
- the place where sandbox review or broader orchestration may happen

Chatty Quest should remain the owner of its own run state and deterministic rules.

### What Must Not Be Implemented In `v0.1`

- live ChattyCog bridge methods
- active handoff mediation
- sandbox routing
- peer-to-peer multiplayer transport
- automatic remote state sync

### Interface Seam To Reserve

Reserve the idea of:

- outward-published structured state
- explicit inbox/outbox handoff folders
- metadata-bearing state or artifact packets

But keep those seams dormant in `v0.1`.

## Chatty-Art

### Likely Future Role

Chatty-Art is the local media-generation sibling.

It handles:

- image generation
- GIF/video generation
- audio generation
- reference-guided editing
- output review and iteration

### What The Reference Material Implies

The README and handshake materials show that Chatty-Art:

- owns its own real files and generation state
- can be hosted inside ChattyCog
- uses explicit mediated artifact lanes
- does not surrender its primary state to the host

Its existing bridge shape includes:

- outgoing dataset-candidate handoffs
- sandbox export
- incoming LoRA imports

### What That Means For Chatty Quest

Future Chatty Quest integration should treat Chatty-Art as the place where generated game media may eventually come from.

Likely future uses:

- location art
- enemy portraits
- boss portraits
- item imagery
- ambience media requests

Chatty Quest should request or consume such outputs explicitly later rather than trying to generate them inside the game engine.

### What Must Not Be Implemented In `v0.1`

- image generation
- audio generation
- video generation
- embedded Chatty-Art dashboards
- automatic import from Chatty-Art folders

### Interface Seam To Reserve

Reserve:

- request and output folders
- media-reference-friendly datapack fields
- future metadata-bearing request packets

## Chatty-Lora

### Likely Future Role

Chatty-Lora is the local style, dataset, and LoRA-building sibling.

It handles:

- respectful material search
- dataset curation
- training-plan assembly
- backend/lane selection
- training-output management
- source-specific review and fix workflows

### What The Reference Material Implies

The README and handshake materials show that Chatty-Lora:

- owns its own datasets, plans, and training outputs
- can be hosted inside ChattyCog
- uses explicit mediated artifact lanes
- is structured around family, backend, and lane separation

Its existing bridge shape includes:

- incoming dataset candidates
- outgoing LoRA imports

### What That Means For Chatty Quest

Future Chatty Quest integration may eventually benefit from:

- style-pack metadata
- scenario-specific visual consistency lanes
- optional training-oriented exports or references

But these are sibling-tool responsibilities, not game-engine responsibilities.

### What Must Not Be Implemented In `v0.1`

- dataset crawling
- dataset cleanup workflows
- LoRA training
- concept stack planning
- source-fix tooling
- style-training orchestration

### Interface Seam To Reserve

Reserve:

- style reference and output folders
- future style metadata in datapacks if needed
- explicit handoff packet seams for later style-related exchange

## Cross-Project Takeaways

Several ecosystem patterns repeat across the references:

- each tool owns its own real state
- hosting does not erase module boundaries
- artifact exchange is copy-only and explicit
- metadata matters
- user confirmation matters
- orchestration should not quietly become hidden coupling

These patterns are highly relevant to Chatty Quest because they align with the game's own deterministic philosophy:

- structured truth should remain structured truth
- narration should not become hidden authority
- bridge layers should not become the primary source of state

## Guidance For Chatty Quest

The correct `v0.1` interpretation is:

- understand the ecosystem
- reserve the folder and interface seams
- do not implement the live ecosystem

The correct future interpretation is:

- Chatty Quest may later publish structured game-state packets
- Chatty Quest may later receive approved external state or artifact packets
- ChattyCog likely mediates those routes
- sibling tools own their own domains

## Final Summary

The reference material supports a clean future architecture:

- Chatty Quest as the deterministic game engine
- ChattyCog as the coordination and peer-to-peer mediation layer
- Chatty-Art as the media-generation sibling
- Chatty-Lora as the style/training sibling

That is the direction to preserve.

It is not the thing to implement in `v0.1`.
