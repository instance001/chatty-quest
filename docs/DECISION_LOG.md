# Decision Log

## Purpose

This document records important design and scope decisions for Chatty Quest.

It exists so that:

- future contributors can understand why the project is shaped this way
- the team does not repeatedly reopen settled foundation questions
- deferred questions remain visible instead of being forgotten

This is not meant to capture every tiny implementation choice. It is for decisions that influence architecture, scope, ownership, or long-term direction.

## How To Use This Log

When adding a new entry, include:

- decision title
- status
- date
- short context
- decision
- consequence

Suggested statuses:

- accepted
- deferred
- superseded
- open

## Initial Decisions

### Decision: RD Engine Is The Reusable Engine Identity

- Status: accepted
- Date: 2026-06-09

Context:

The project is increasingly acting like a reusable engine substrate with one flagship game on top, and the planning material now gives that architecture a clear name.

Decision:

Use `RD Engine` as the reusable engine identity, with `Radiant Determinism` as the guiding doctrine and `Chatty Quest` as the first game built on it.

Consequence:

- docs can speak clearly about engine versus game identity
- future contributors have a cleaner mental model for reuse
- branding and architecture language now reinforce each other

### Decision: Host Owns Canonical Truth

- Status: accepted
- Date: 2026-06-08

Context:

The project is meant to feel like a chat-driven AI DM adventure, but it must remain a real game with save/load, inventory, map state, and testable outcomes.

Decision:

Canonical state is owned by the host engine, not by narration.

Consequence:

- map, inventory, HP, objectives, and combat outcomes must live in structured runtime state
- narration becomes replaceable presentation instead of authority
- future real model integration remains possible without surrendering control

### Decision: Descriptions Are Flavour, Fields Are Truth

- Status: accepted
- Date: 2026-06-09

Context:

The planning material reinforces that prose, media, and narrator style should remain expressive without becoming hidden mechanics.

Decision:

Template descriptions and narrator-facing flavour may inspire presentation, but mechanically relevant facts must live in explicit fields, structured state, or reducer-visible data.

Consequence:

- datapacks remain easier to validate
- future LLM narration stays grounded
- summaries and media can stay expressive without becoming secret rule carriers

### Decision: Scenario Engine First, Zombie Scenario Second

- Status: accepted
- Date: 2026-06-08

Context:

`Property Siege Classic` is the first scenario, but the planning intent clearly describes a reusable scenario engine rather than a one-off zombie game.

Decision:

The engine should be designed as a datapack-driven scenario system, with `Property Siege Classic` as the first proof-of-shape scenario.

Consequence:

- avoid hardcoding zombie-specific assumptions into core systems
- keep content external where practical
- name systems in engine terms rather than one-scenario terms

### Decision: `Property Siege Classic` Is The First Playable Scenario

- Status: accepted
- Date: 2026-06-08

Context:

The project needs a bounded, understandable, and emotionally legible first scenario that can prove the architecture without overbuilding.

Decision:

`Property Siege Classic` is the single active playable scenario for `v0.1`.

Consequence:

- `v0.1` scope stays small
- boundary rules and map constraints become easier to prove
- the first release remains focused on one complete slice instead of many partial ones

### Decision: Scenario Boundaries Belong To Scenario Rules

- Status: accepted
- Date: 2026-06-08

Context:

The project specifically wants scenario-configured boundaries, such as the player being unable to leave the property during the zombie siege.

Decision:

Scenario boundaries are defined by scenario rules and content, not by a universal engine law that forbids leaving local maps.

Consequence:

- future scenarios can define different boundary behaviors
- the engine remains reusable across different game types
- `Property Siege Classic` remains a scenario example rather than a permanent engine assumption

### Decision: Chat-First Surface, Narrow `v0.1` Interpretation

- Status: accepted
- Date: 2026-06-08

Context:

The original planning thread emphasized a chat-window-first fantasy, but `v0.1` should not pretend to support unrestricted natural-language understanding.

Decision:

`v0.1` keeps a chat-first UI while allowing a narrow action interpreter under the hood.

Consequence:

- the experience still feels like talking to a Dungeon Master
- the first release avoids fake general intelligence
- reducer-driven actions remain reliable

### Decision: `MockNarrator` First

- Status: accepted
- Date: 2026-06-08

Context:

The project wants future narrator replaceability, but the architecture must be proven before real LLM integration is attempted.

Decision:

`v0.1` uses a `Narrator` seam with `MockNarrator` only.

Consequence:

- the presentation boundary is proven early
- the first release avoids model/runtime instability
- future LLM replacement remains straightforward in concept

### Decision: `egui/eframe` As The Default GUI Direction

- Status: accepted
- Date: 2026-06-08

Context:

The planning materials explicitly recommend a Rust desktop GUI using `egui/eframe` unless a strong reason appears not to.

Decision:

Use `egui/eframe` as the default UI direction for `v0.1`, barring a concrete blocker discovered during implementation.

Consequence:

- desktop development can proceed without inventing a new front-end stack
- the game shell can stay close to the Rust core

### Decision: Save Files Store Deterministic State, Not Prose Truth

- Status: accepted
- Date: 2026-06-08

Context:

The project's reliability depends on structured state being authoritative.

Decision:

Save files restore deterministic runtime state. Narration and summaries may be stored only as support data.

Consequence:

- save/load behavior stays trustworthy
- chat logs do not become accidental databases
- debugging and validation remain easier

### Decision: Rolling Summary Supports Continuity, Not Canon

- Status: accepted
- Date: 2026-06-09

Context:

Future NPC memory, dynamic dialogue, and richer narrator continuity will benefit from rolling summary, but summary must not replace state.

Decision:

Use rolling summary as support memory only. If a fact affects mechanics, promises rewards, changes quest state, or alters future legality, it must be represented in structured state or buckets.

Consequence:

- future NPC and quest systems remain trustworthy
- summary can stay lightweight and flexible
- the engine avoids turning prose memory into hidden canon

### Decision: Future Ecosystem Seams Are Reserved, Not Implemented

- Status: accepted
- Date: 2026-06-08

Context:

The reference projects clearly separate tool-owned state from mediated handoff behavior. The game should respect that architecture without importing those systems into `v0.1`.

Decision:

Reserve `Chatty-Cog`, `Chatty-Art`, and `Chatty-Lora` integration seams through folders and docs only.

Consequence:

- `v0.1` stays independent and playable
- future integration remains easier to reason about
- the project avoids becoming a partial ecosystem host too early

### Decision: Future Handoff Uses Snapshot Packets, Not Remote Truth Ownership

- Status: accepted
- Date: 2026-06-10

Context:

The reference projects and current RD Engine doctrine both point toward explicit, metadata-rich handoff seams. As Chatty Quest reserves future Chatty-Cog, Chatty-Art, and Chatty-Lora integration, the project needs a clear rule about what a handoff packet is allowed to mean.

Decision:

Future handoff payloads are copy-oriented structured packets or artifact envelopes. They may publish bounded state, recent reducer-confirmed outcomes, or explicit artifact references, but they do not silently transfer canonical run ownership away from the local host.

Consequence:

- future multiplayer-facing exchange can stay reducer-compatible
- Chatty-Cog can mediate packets without becoming the live source of Quest truth
- save files, UI state, and prose narration remain clearly separate from handoff payload contracts

### Decision: Derived Views Are Rebuildable, Not Authoritative

- Status: accepted
- Date: 2026-06-10

Context:

As the project grew map layouts, media focus state, diagnostics reports, asset viewers, and future handoff packet planning, the risk increased that presentation-friendly computed models might quietly become parallel authorities beside runtime truth.

Decision:

Treat UI, media, diagnostics, and export-facing computed models as rebuildable derived views. They may interpret canonical truth for readability or transport, but they do not own gameplay authority and may not silently mutate or override reducer-owned state.

Consequence:

- future panels can be added without inventing hidden mechanics
- instance migration becomes easier because views can depend on derived models rather than raw storage
- handoff and diagnostics systems stay useful without becoming shadow runtimes

### Decision: Future Multiplayer Belongs To Chatty-Cog Mediation, Not Game-Core Ownership

- Status: accepted
- Date: 2026-06-08

Context:

Future multiplayer is envisioned as cross-ChattyCog peer-to-peer coordination using handshake and wireless connection features rather than a monolithic networking stack inside Chatty Quest itself.

Decision:

If multiplayer-style state sharing is added later, Chatty Quest should publish and receive structured game-state packets while Chatty-Cog owns the broader peer-to-peer mediation and coordination layer.

Consequence:

- Chatty Quest remains focused on its own deterministic state model
- networking and cross-host orchestration stay outside the game engine
- future collaboration features can align with the broader ecosystem architecture

## Deferred Decisions

### Decision: Fixed Versus Lightly Seeded `Property Siege Classic` Map

- Status: deferred
- Date: 2026-06-08

Context:

The planning material supports both a generated-run fantasy and a practical small first scenario. The exact degree of procedural variation in `v0.1` remains open.

Decision:

Defer the exact balance between fixed topology and lightly seeded variation until implementation planning reaches run generation details.

Consequence:

- the docs preserve both options
- the team can choose the smallest honest version once engine scaffolding is in place

### Decision: Which Actions Need Confirmation Gates

- Status: deferred
- Date: 2026-06-08

Context:

The planning thread introduced a useful idea around pending state changes and confirmation, but not every action necessarily needs a confirm step in the first release.

Decision:

Defer the exact confirmation-gate list until reducer and UI interaction design are closer.

Consequence:

- the architecture keeps room for pending changes
- `v0.1` can still stay simple where immediate resolution is cleaner

### Decision: When To Generalize Full Template Instance IDs Across Content Families

- Status: deferred
- Date: 2026-06-09

Context:

The long-term design clearly benefits from a clean `template -> instance -> bucket` model, but `v0.1` can still prove the architecture with lighter representations in some places.

Decision:

Defer the exact point at which every content family adopts a fully generalized instance-ID model.

Consequence:

- the future shape is preserved in doctrine
- `v0.1` is not forced to over-abstract too early

## Open Questions

### Question: How Much Natural-Language Parsing Is Worth Supporting In `v0.1`?

- Status: open
- Date: 2026-06-08

Context:

The chat-first feel matters, but parser complexity can quickly explode.

Current direction:

Prefer a narrow interpreter that handles obvious commands honestly and fails gracefully elsewhere.

### Question: How Rich Should The Rolling Summary Be In `v0.1`?

- Status: open
- Date: 2026-06-08

Context:

Rolling summary is important to the project's feel and future narrator continuity, but it must remain clearly secondary to structured truth.

Current direction:

Treat it as recap and support memory first, not as a mechanical dependency.

### Question: When Should Context-Aware Media Overrides Become A Formal Datapack Feature?

- Status: open
- Date: 2026-06-09

Context:

The planning material suggests a strong future UX for location-plus-entity combo media, but the team does not need to force full authoring complexity into the first playable slice.

Current direction:

Keep current visual focus and event-driven media as the main seam now. Add formal context override authoring once the basic BYO-media workflow is proven and useful.
