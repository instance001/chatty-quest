# UI Shell Spec

## Purpose

This document defines the intended desktop UI shape for Chatty Quest as the first game built on the RD Engine.

It exists to answer a specific question before implementation goes too far:

How should the shell stay clean, chat-forward, expandable, and scenario-engine-friendly as new tabs, game types, and future model settings appear?

The intended answer is:

Use a tabbed browser-style dashboard where:

- setup and generation happen first
- gameplay tabs appear once a run exists
- new gameplay domains usually arrive as new tabs instead of cluttering the main play surface

## Core UI Philosophy

The UI should feel like:

- a real desktop game client
- easy to navigate
- chat-first during play
- expandable without becoming a junk drawer

The shell should not feel like:

- one giant scrolling control panel
- a hidden-systems toy where important state is buried
- a mod-hostile hardcoded layout

At the RD Engine level, the UI also has a specific job:

- prove that reducer-confirmed state changes really happened
- give the narrator a trustworthy stage
- show media as illustration, not as truth ownership

## High-Level Structure

The UI should have two major states:

### 1. Setup State

Before a run exists, the shell is focused on:

- datapack selection
- world generation
- scenario settings
- future model configuration
- difficulty and chaos controls
- new game and load game entry points

### 2. Active Run State

After a run is created or loaded, the shell expands into gameplay tabs.

This keeps the early interface focused and prevents inactive game tabs from showing empty or confusing content before a world exists.

## Browser-Style Tab Model

The tabbed model is the recommended UI spine because it solves several long-term problems cleanly.

### It keeps the main play screen focused

The central game loop should not compete with every other system at once.

### It supports future growth cleanly

If a scenario or mod wants:

- world lore notes
- spellbook
- codex
- faction dashboard
- war map
- settlement view
- crew roster

the clean answer is often:

add a new tab

### It supports different game types

The planning thread made it clear that future game types may vary, not just themes.

Possible future examples:

- D&D-style adventure
- zombie property survival
- multiplayer war sim
- office-worker life sim

A tabbed shell lets the engine surface different deterministic domains without collapsing them all into one overloaded screen.

## Top-Level Shell Areas

The shell should be thought of as four layers:

### Title / Utility Bar

Reserved for:

- app identity
- current run name or scenario name
- save/load shortcuts later
- future lightweight status indicators

### Primary Tab Bar

The main navigation layer.

Example active-run tabs:

- `Game`
- `Inventory`
- `Character`
- `Diagnostics`
- future: `Lore`
- future: `Spellbook`
- future: `Faction`
- future: `War Room`

### Context Body

The content area for whichever tab is selected.

### Optional Footer / Status Strip

Reserved for:

- validation or save status
- narrator or reducer state hints if useful
- future hosted or integration status markers

This should remain lightweight and not become a dumping ground.

## Setup State Spec

Before world generation or load, the primary content area should function as a clean setup dashboard.

Recommended setup sections:

- scenario or datapack selection
- new game controls
- load game controls
- difficulty
- chaos mode
- DM capsule
- future model settings

## Setup Controls

### Datapack Selection

Required:

- visible selectable datapack list
- clear pack name
- short pack description

### World Generation Controls

Required for the long-term shape:

- generate new game
- load existing save

Future-facing controls may include:

- seed
- map scale
- objective mode

`v0.1` can keep these simple.

### Difficulty

Should be a first-class visible control rather than buried in advanced settings.

Reason:

Difficulty is one of the major dials described in planning and should remain distinct from narration tone.

### Chaos Mode

Should be visible as its own dial, separate from difficulty.

Reason:

Chaos affects narration looseness, not truth ownership.

### DM Capsule

Should be selectable as a tone control, separate from mechanics.

Reason:

Narrator voice should not be conflated with world rules or difficulty.

### Future Model Settings

The planning direction here makes sense and should be reserved in the setup shell.

Long-term example:

- bookkeeper or summary helper on CPU
- main chat narrator on GPU

This is conceptually similar to the broader Chatty ecosystem split-brain model direction and belongs in setup rather than buried deep in active-run tabs.

Important:

`v0.1` does not need live model integration, but the setup shell can still reserve a clearly labeled area for future narrator and summary model selection.

Good `v0.1` interpretation:

- visible placeholder section
- disabled or informational controls
- no live runtime dependency

## Active Run Tab Set

The default active-run tab set should start small.

Recommended first set:

- `Game`
- `Inventory`
- `Character`
- `Diagnostics`

This is enough for `v0.1` while preserving obvious expansion room.

## `Game` Tab Spec

The `Game` tab is the heart of the experience and should get the most design attention.

Recommended layout:

- top at-a-glance status strip
- central chat area
- left side panel for map
- right side panel for media

This creates one clear focal column while still surfacing essential deterministic context.

## `Game` Tab Layout Regions

### At-A-Glance Strip

A narrow band above the main chat area for critical current-run state.

Examples:

- HP
- mana
- sanity
- stamina
- objective status
- time pressure

Important:

This strip should be scenario-aware rather than globally hardcoded to one stat set.

That means the shell should be designed to surface a compact set of relevant stats for the active scenario or game type, not always the exact same fantasy RPG fields.

For `Property Siege Classic`, this may be very small.
For a future spell-heavy or sanity-heavy scenario, it may surface different values.

### Main Chat Column

This is the primary interaction area.

It should contain:

- chat log
- player text input
- reducer and narrator responses
- maybe compact action feedback or pending-confirmation prompts later

This column should remain visually dominant.

### Map Panel

The map panel should stay visible inside the `Game` tab rather than being buried in another tab.

Reason:

Location and movement are part of the immediate play loop.

The map panel should surface:

- current location
- discovered or visible connected locations
- future lock or boundary hints

### Media Panel

The media panel should stay visible beside the chat area because it is part of the intended emotional feel of the game.

It should surface:

- location image
- enemy portrait
- item art
- short video or placeholder media

For `v0.1`, placeholders are fine.
The important thing is that the shell reserves the lane.

At the RD Engine level, the media panel should follow a current visual focus rule:

- location entry defaults to current location media
- look, inspect, and encounter outcomes may shift focus to a more relevant entity
- future context overrides may allow `entity + current_location` combo presentation
- if no stronger focus exists, fall back cleanly

Recommended focus priority:

1. explicit current focus from recent reducer-confirmed action
2. future dual-context media such as entity plus location
3. focused entity media
4. current location media
5. scenario placeholder

This keeps media expressive without making it a hidden state machine.

## `Inventory` Tab Spec

The inventory tab should hold the deterministic item state cleanly.

Expected future contents:

- carried items
- equipped items
- consumables
- item details
- future context actions

The key reason this belongs in its own tab is focus.

Inventory should be accessible quickly, but it should not crowd the main play column all the time.

## `Character` Tab Spec

The character tab should hold player-centric deterministic state.

Expected future contents:

- HP and other core stats
- traits
- skills
- progression
- status effects

This tab should scale naturally for future game types without forcing the `Game` tab to become a giant stats page.

## `Diagnostics` Tab Spec

The diagnostics tab is the home for development-time and future support-time visibility.

Expected `v0.1` contents:

- application health
- content health
- save and version checks
- event counters
- recent canonical reducer events

Longer-term, it can also hold:

- template warnings
- missing runtime or asset checks
- compatibility issues
- future IT-style bugtest surfaces

This keeps health and debug visibility out of the main play tab while preserving a durable place for support tooling.

## Future Tab Expansion Rules

New tabs should be added when they represent a stable gameplay domain.

Good candidates:

- `Lore`
- `Spellbook`
- `Codex`
- `Quest Log`
- `Faction`
- `War Room`
- `Crew`

Bad candidates:

- tiny one-off toggles
- controls that belong in setup
- values that belong in the at-a-glance strip

Rule of thumb:

If a feature needs sustained reading, editing, or management, it is a good tab candidate.

## Modder-Facing UI Philosophy

The tabbed shell is also the cleanest modder story.

Instead of forcing every modder extension into the main gameplay surface, the shell should conceptually allow:

- new tabs
- scenario-specific tab visibility
- scenario-specific stat-strip values
- scenario-specific media usage

Even if `v0.1` does not implement dynamic tab registration yet, the layout should be built with that future in mind.

## Cleanliness Rules

To keep the shell navigable:

- setup controls should not bleed into active play unnecessarily
- the `Game` tab should stay focused on the current turn loop
- side panels should support immediate play context, not every subsystem
- secondary systems should become tabs instead of permanent clutter

## `v0.1` UI Recommendation

For the first playable slice, the most honest version is:

### Setup Screen

- datapack selection
- new game button
- load game button
- difficulty control
- chaos mode control
- DM capsule control
- disabled or placeholder future model settings area

### Active Run Tabs

- `Game`
- `Inventory`
- `Character`
- `Diagnostics`

### `Game` Tab Layout

- top status strip
- left map panel
- center chat log and input
- right media panel

This proves the intended shell shape without overbuilding tab infrastructure on day one.

## Short Summary

The right UI direction is:

- setup-first before a run exists
- tabbed-browser shell after the run exists
- chat-first `Game` tab with map and media beside it
- deterministic systems split into clean tabs
- future model and scenario growth reserved in the shell
- modder expansion handled by adding tabs rather than bloating the core view

That is the cleanest path to a scalable Chatty Quest UI.
