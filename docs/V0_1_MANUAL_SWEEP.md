# `v0.1` Manual Sweep

This is the current short live click-through for Chatty Quest.

Refresh note:

- refreshed on `2026-07-16` for the current post-`v0.1` branch
- this sweep now includes barricade-state, noise-state, guided-command, and siege-route forecast checks
- refreshed again on `2026-07-28` to include epilogue, narrator-context, and command-boundary checks

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

- `cargo test` passes with `67` automated tests
- `cargo clippy --all-targets --all-features -- -D warnings` passes
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

- type a bare target verb such as `go`, `inspect`, or `use`
- from `Front Verandah`, use the `Torch`
- move from `Front Verandah` to `Kitchen`
- try `go garage` from `Front Verandah` before unlocking it
- try one invalid move by typing `go laundry` from `Front Verandah` before moving, or another obviously disconnected destination from your current room

Check:

- bare target verbs are rejected clearly before any reducer or narrator outcome is produced
- using the torch reveals connected routes without moving the player
- valid movement updates the current location in the map panel
- the chat log reflects the move
- the sidebar now surfaces room-role hints for major spaces such as `Front Verandah`, `Back Garden`, and `Garage`
- the sidebar also surfaces objective progress, a suggested next command, and a readable threat forecast where relevant
- locked movement is rejected with the garage-lock response
- invalid movement is rejected with the boundary response
- connected exits update truthfully after movement

Pass if:

- the player can tell where they are and the UI does not contradict movement truth

### 4. Inventory Loop

Action:

- in `Kitchen`, take the `Medkit`
- continue to `Laundry`
- take the `House Keys`
- return to `Front Verandah`
- verify `use house_keys` now asks for explicit targeting when more than one gate matches
- run `unlock garage`
- run `unlock back_garden`
- open the `Inventory` tab
- equip the `Battered Cricket Bat`
- use the `Medkit` after taking damage later, or manually verify it is present and usable first

Check:

- the medkit disappears from the room after pickup
- the medkit appears in inventory
- the house keys appear in inventory
- the command bar hint updates toward likely next verbs such as `unlock garage` or `unlock back_garden` once keys are held
- the torch remains usable and can report when no new nearby routes are left to reveal
- `use house_keys` does not guess when both `Garage` and `Back Garden` are valid nearby gates
- `unlock garage` unlocks only the garage
- `unlock back_garden` unlocks only the back garden
- equipped state visibly updates for the cricket bat
- using the medkit removes it from inventory and updates HP

Pass if:

- item state changes are visible in structured UI, not just implied by prose

### 5. Barricade Loop

Action:

- unlock and enter `Back Garden`
- take the `Barricade Kit`
- wait once in `Back Garden` before barricading it
- run `barricade back_garden`
- wait again in `Back Garden`
- return to `Front Verandah`
- wait once before barricading it if the `Front Gate Shambler` is still alive
- run `barricade front_verandah`
- wait again at `Front Verandah`

Check:

- `Back Garden` and `Front Verandah` both surface barricade state in inspection or sidebar truth
- both rooms also surface a readable threat forecast before and after securing them
- waiting in `Back Garden` before barricading causes passive pressure damage
- barricading `Back Garden` suppresses that pressure
- barricading `Back Garden` grants the authored HP recovery bonus
- waiting at `Front Verandah` before barricading causes passive shambler pressure if the shambler is still alive
- barricading `Front Verandah` suppresses that pressure
- if noise has climbed, the forecast and helper text make the higher exposed-route pressure visible before another wait
- map and known-location UI visibly show barricaded spaces
- exit labels and room-role hints make the two siege routes feel intentionally different

Pass if:

- the player can tell that `Front Verandah` is the threshold-defense lane and `Back Garden` is the risky flank with recovery payoff

### 6. Character Tab

Action:

- open the `Character` tab before and after meaningful actions

Check:

- HP is visible
- noise level and label are visible
- current location is visible
- objective completion state is visible
- rolling summary is populated
- `View Current Location` works
- `View Equipped Item` works once something is equipped

Pass if:

- the tab acts as a truthful mechanical snapshot of the run

### 7. Combat And Objective

Action:

- fight the `Front Gate Shambler` and/or `Crawler In The Weeds`
- optionally unlock the `Back Garden` and verify it opens as a second valid key gate
- unlock the `Garage`
- reach the `Garage`
- kill the `Garage Brute`

Check:

- the garage is visibly locked before unlock
- the back garden is visibly locked before optional unlock
- the garage becomes visibly unlocked after `unlock garage`
- if `Back Garden` was barricaded earlier, the UI continues to report it as barricaded
- if `Garage` is the next meaningful move, the suggested command surface points at it once the route is open
- if both `Front Verandah` and `Back Garden` were barricaded before entering the `Garage`, the forecast or inspection text acknowledges the secured-property payoff
- the objective condition lines show `House Keys (house_keys)` held before the boss dies
- torch-driven reveal behavior never invents movement or objective progress by itself
- objective progress lines surface when `house_keys` is acquired and when the boss condition completes
- attack updates the log and HP
- enemy/boss threat state updates in the UI
- the objective remains visible before completion
- `WIN` appears clearly in UI when the brute dies
- `Run phase: Epilogue` appears in the sidebar, character tab, or diagnostics surfaces after the win
- the win banner explains that aftermath exploration is available
- after `WIN`, the action bar calms down to epilogue-safe suggestions such as `look` and `help`
- after `WIN`, `attack` or `wait` returns a clear epilogue line instead of changing HP, noise, or combat state
- after `WIN`, looking, inspecting, moving through open routes, saving, and loading remain possible
- after `WIN`, `look` or `inspect room` can surface authored aftermath hooks without creating new mechanics
- narration still feels flavorful without contradicting the result
- narrator output stays grounded in the structured action outcome rather than interpreting raw typed command text directly

Pass if:

- the scenario can be completed through ordinary play, the UI recognizes the win cleanly, and the aftermath stays explorable without continuing the main danger loop

### 8. Media Panel

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

### 9. Save And Load

Action:

- after making progress, click `Save`
- click `Load`

Check:

- location restores correctly
- HP restores correctly
- inventory restores correctly
- locked/unlocked gate state restores correctly for the garage and any other changed gate
- barricaded state restores correctly for the verandah and any barricaded flank
- noise state restores correctly
- objective state restores correctly
- the app returns to a coherent playable shell rather than a half-loaded state

Pass if:

- the run can be stopped and resumed without confusion

### 10. Diagnostics Tab

Action:

- open `Diagnostics`

Check:

- the panel renders cleanly
- application/content/run/environment sections are readable
- lock-state truth is visible and understandable
- barricade-state truth is visible and understandable
- noise truth is visible and understandable
- run phase truth is visible and switches to `Epilogue` after a completed objective
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

If every section above passes, record the current branch sweep result in the acceptance audit or release notes without changing the historical `v0.1` acceptance date.

Current note:

- this runbook is current as of `2026-07-28`
- current-branch live desktop sweep passed on `2026-07-28`
- supporting screenshots for release documentation live under `assets/ui/screenshots/`
