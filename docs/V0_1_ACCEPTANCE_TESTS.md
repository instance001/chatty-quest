# `v0.1` Acceptance Tests

## Purpose

This document defines what must be true for Chatty Quest `v0.1` to count as an acceptable first release.

It is intentionally focused on:

- one playable scenario
- deterministic truth ownership
- a usable desktop shell
- save/load and validation reliability

This is not a wishlist for future features. It is the definition of the first honest playable slice.

## Acceptance Standard

`v0.1` is acceptable when:

- the app is launchable
- the RD Engine shape is proven
- the first playable loop works
- the narrator seam exists without owning truth
- the reserved future lanes remain dormant

## Launch And Menu

The following should work:

- the application launches without crashing
- the splash or launch presentation completes cleanly or is skippable
- the main menu renders
- the main menu exposes at least:
  - new game
  - load game
  - datapack selection
- the user can reach the playable game view from the menu

Pass condition:

The app can be started locally and the user can navigate from launch into a new or loaded run.

## Datapack Selection

The following should work:

- `Property Siege Classic` appears as a selectable datapack or scenario
- datapack metadata is loaded from data files rather than hardcoded only in UI text
- invalid datapacks do not appear as silently playable

Pass condition:

The game clearly recognizes the active datapack and uses external pack data to drive selection.

## New Game Generation

The following should work:

- creating a new game produces a valid `RunState`
- the player starts in a valid scenario location
- the scenario objective is frozen into the run state
- item, enemy, and boss placement follow scenario-compatible logic

Pass condition:

A new game starts in a coherent deterministic state that can be inspected and played without manual repair.

## Map And Location Display

The following should work:

- the current location is visible
- the map panel renders scenario-relevant location information
- deterministic knowledge-reveal item effects can expand visible known-route state without moving the player
- movement options or connected locations are represented clearly enough to support play
- the player's current location updates after valid movement

Pass condition:

The player can tell where they are and the UI reflects movement truthfully.

## Chat And Narration

The following should work:

- the game tab includes a chat-style log
- the player can enter text input
- raw player text resolves into a structured command before reducer or narrator handling
- incomplete target commands such as `go`, `inspect`, or `use` are rejected clearly
- the system produces narrator-style responses through `MockNarrator`
- narration reflects reducer-confirmed outcomes rather than inventing state

Pass condition:

The surface feels like a conversational adventure while staying grounded in deterministic results.

## Inventory And Character Display

The following should work:

- inventory contents are visible
- equipped item state is visible
- character or status panel shows HP and basic stats
- inventory and character displays update after valid reducer actions

Pass condition:

The player can inspect core mechanical state without relying on prose alone.

## Barricade And Siege Pressure

The following should work:

- barricadable rooms visibly surface whether they are exposed or secured
- `Front Verandah` and `Back Garden` can both be barricaded through explicit deterministic commands
- passive threat pressure applies before barricading in those authored rooms
- passive threat pressure is suppressed after barricading
- authored room-specific payoff differences remain visible, including the `Back Garden` recovery bonus
- exposed-route pressure warnings or helper surfaces make the likely penalty legible before the player has to eat it blindly

Pass condition:

The player can understand and use the siege mechanic through structured truth, not only through trial-and-error prose.

## Noise And Escalation

The following should work:

- the current noise state is visible in the `Game`, `Character`, and diagnostics surfaces
- loud actions raise noise deterministically
- waiting in a barricaded location lowers noise deterministically
- exposed-route risk gets worse at higher noise where authored
- the player can tell from the UI when noise is making an exposed route more dangerous

Pass condition:

The escalation layer is visible, deterministic, and understandable without requiring the player to reverse-engineer it from damage logs.

## Movement And Boundaries

The following should work:

- valid movement between connected locations succeeds
- locked progression gates block movement until their deterministic prerequisite is satisfied
- invalid movement is rejected
- scenario boundaries prevent leaving the allowed playable space
- boundary behavior is scenario-driven rather than hidden engine law

Pass condition:

The player cannot break the playable map or bypass `Property Siege Classic` boundary rules through ordinary play.

## Item Interaction

The following should work:

- item pickup changes deterministic state
- item use changes deterministic state when legal
- utility items can unlock deterministic progression gates through explicit location-targeted commands when more than one valid gate exists
- utility items can also reveal deterministic knowledge state when their template defines a supported effect
- equip changes deterministic state when legal
- consumed or destroyed items no longer appear as untouched inventory

Pass condition:

Item interactions are reflected in structured state and visible UI, not only in narration.

## Combat

The following should work:

- attack actions resolve deterministically through the reducer
- HP changes are reflected in structured state
- enemy alive or dead state updates correctly
- boss alive or dead state updates correctly when relevant
- authored finale payoffs, such as secured-route preparation, affect combat only through reducer-owned deterministic rules

Pass condition:

Combat produces trustworthy mechanical outcomes even if the first model is intentionally simple.

## Objective Progress

The following should work:

- the active objective is visible
- the active objective condition rows show boss and item truth when present
- objective progress lines surface when a tracked condition changes state
- objective-related state progresses when the required conditions are met
- objective completion is driven by structured state rather than narrator prose
- win or loss is surfaced clearly in UI state as well as narration
- after a win, the run enters an epilogue-friendly state where exploration and inspection can continue, datapack-authored aftermath room text and post-credits hooks can surface, and combat, pressure, and resource mutation do not keep pretending the main scenario is unresolved
- guided command or helper surfaces can point toward the next likely objective verb without forcing the player to use them

Pass condition:

The player can meaningfully complete the scenario goal and the game recognizes that completion correctly.

## Media Focus

The following should work:

- the media panel is present in the `Game` tab
- the panel can fall back to scenario or location context without blocking play
- recent reducer-confirmed events can shift the current visual focus
- media never implies a state change that the reducer did not confirm

Pass condition:

The media lane behaves like presentation attached to truth, not like a second hidden rules system.

## Save/Load JSON

The following should work:

- saving writes a JSON save file
- loading restores the run state accurately
- restored state includes player location, inventory, HP, objective progress, objective condition truth, lock-state truth, barricade-state truth, and noise-state truth
- loading a save returns the user to a coherent playable state

Pass condition:

The game can be stopped and resumed without relying on memory or chat history reconstruction.

## Validation Errors

The following should work:

- broken datapacks produce useful validation errors
- missing required files are detected
- impossible references are detected
- invalid scenario setup is reported rather than played blindly

Pass condition:

Content errors are diagnosable by a human without diving into engine internals first.

## Reserved Future Folders

The following should be true:

- reserved `runtime/`, `models/`, `datasets/`, and `handoff/` folders exist
- their intended purpose is documented
- they are not required as active runtime dependencies for `v0.1`

Pass condition:

The future architecture is visible without dragging future systems into the first release.

## Narrator Boundary Test

The following should be true:

- the narrator does not create permanent items
- the narrator does not invent new canonical map locations
- the narrator does not directly alter HP, inventory, objective completion, or player location

Pass condition:

All lasting gameplay changes still flow through deterministic game logic.

## Ecosystem Boundary Test

The following should be true:

- `v0.1` does not depend on Chatty-Cog runtime behavior
- `v0.1` does not depend on Chatty-Art media generation
- `v0.1` does not depend on Chatty-Lora training workflows
- `v0.1` does not implement multiplayer transport

Pass condition:

The first release remains a complete local playable game slice on its own.

## Final `v0.1` Verdict

`v0.1` should be considered successful if it proves all of the following at once:

- one scenario can be loaded from a datapack
- one run can be generated and played deterministically
- the UI supports the intended chat-forward loop
- the narrator seam exists without owning truth
- saves and validation make the game trustworthy

If those conditions are met, the architecture is proven even if many future ambitions remain unbuilt.
