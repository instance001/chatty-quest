# `v0.1` Manual Sweep

This is the final short live click-through for Chatty Quest `v0.1`.

Goal:

- confirm the desktop shell behaves correctly in a real run
- confirm the visible UI matches the deterministic state we already test automatically
- close the remaining gap between automated confidence and release honesty

Estimated time: `5-10 minutes`

## Before You Start

1. Run `cargo test`
2. Launch the app locally
3. Start with a clean mindset: if the UI lies, stalls, or looks broken, treat that as a real failure even if the tests are green

Expected baseline:

- `Property Siege Classic` is the selected datapack
- setup screen is visible
- no crash on launch

## Sweep Steps

### 1. Setup Screen

Check:

- the app opens to the setup screen
- `Generate Game` and `Load Game` are visible
- datapack selection is visible
- `Property Siege Classic` appears as the active playable datapack
- datapack status text renders without obvious layout breakage

Pass if:

- you can clearly understand how to start or load a run

### 2. New Game Flow

Action:

- click `Generate Game`

Check:

- the app transitions into the active run shell
- `Game`, `Inventory`, `Character`, and `Diagnostics` tabs appear
- the chat log is populated
- the map panel shows a current location
- the media panel is present

Pass if:

- the new run feels coherent immediately, without needing repair or restart

### 3. Map And Movement

Action:

- move from `Front Verandah` to `Kitchen`
- try one invalid move by typing `go laundry` from `Front Verandah` before moving, or another obviously disconnected destination from your current room

Check:

- valid movement updates the current location in the map panel
- the chat log reflects the move
- invalid movement is rejected with the boundary response
- connected exits update truthfully after movement

Pass if:

- the player can tell where they are and the UI does not contradict movement truth

### 4. Inventory Loop

Action:

- in `Kitchen`, take the `Medkit`
- open the `Inventory` tab
- equip the `Battered Cricket Bat`
- use the `Medkit` after taking damage later, or manually verify it is present and usable first

Check:

- the medkit disappears from the room after pickup
- the medkit appears in inventory
- equipped state visibly updates for the cricket bat
- using the medkit removes it from inventory and updates HP

Pass if:

- item state changes are visible in structured UI, not just implied by prose

### 5. Character Tab

Action:

- open the `Character` tab before and after meaningful actions

Check:

- HP is visible
- current location is visible
- objective completion state is visible
- rolling summary is populated
- `View Current Location` works
- `View Equipped Item` works once something is equipped

Pass if:

- the tab acts as a truthful mechanical snapshot of the run

### 6. Combat And Objective

Action:

- fight the `Front Gate Shambler` and/or `Crawler In The Weeds`
- reach the `Garage`
- kill the `Garage Brute`

Check:

- attack updates the log and HP
- enemy/boss threat state updates in the UI
- the objective remains visible before completion
- `WIN` appears clearly in UI when the brute dies
- narration still feels flavorful without contradicting the result

Pass if:

- the scenario can be completed through ordinary play and the UI recognizes the win cleanly

### 7. Media Panel

Action:

- look at the media panel during:
  - fresh run start
  - item pickup
  - boss combat

Check:

- the panel exists and stays stable
- focus shifts with reducer-confirmed events
- fallback behavior does not block play
- the panel does not imply fake state changes

Pass if:

- media behaves like presentation attached to truth

### 8. Save And Load

Action:

- after making progress, click `Save`
- click `Load`

Check:

- location restores correctly
- HP restores correctly
- inventory restores correctly
- objective state restores correctly
- the app returns to a coherent playable shell rather than a half-loaded state

Pass if:

- the run can be stopped and resumed without confusion

### 9. Diagnostics Tab

Action:

- open `Diagnostics`

Check:

- the panel renders cleanly
- application/content/run/environment sections are readable
- missing media warnings are understandable rather than cryptic
- recent events and counters make sense relative to what you just did

Pass if:

- a human can use diagnostics to understand content/runtime health without digging into code first

## Fail Conditions

Treat the sweep as failed if any of the following happens:

- crash on launch or during normal play
- movement UI contradicts actual run state
- inventory/HP/objective UI fails to update after valid actions
- save/load returns an incoherent run
- media panel invents state that the reducer did not confirm
- diagnostics panel is unreadable or misleading

## Finish

If every section above passes, update the acceptance audit from:

- `provisionally on-track`

to:

- `v0.1 accepted`
