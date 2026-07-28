# Chatty Quest Full-Spoilers User Manual

Last updated: `2026-07-28`

## Spoiler Warning

This document contains:

- route-order spoilers
- item-location spoilers
- lock-and-key spoilers
- barricade target spoilers
- boss-behavior spoilers
- objective-answer spoilers

If you want to discover the scenario naturally, stop here and use:

- [docs/ZERO_KNOWLEDGE_USER_MANUAL.md](ZERO_KNOWLEDGE_USER_MANUAL.md)

## What This Guide Is For

This is the practical answer-sheet version of the manual.

Use it if you want:

- a clear explanation of how the current scenario is structured
- an explicit walkthrough
- exact item and route answers
- boss and threat details

## Current Scenario Overview

The current playable scenario is:

- `Property Siege Classic`

The run begins at:

- `Front Verandah`

The important locations are:

- `Front Verandah`
- `Kitchen`
- `Laundry`
- `Back Garden`
- `Garage`

The current map logic is:

- `Front Verandah` connects to `Kitchen`, `Garage`, and `Back Garden`
- `Kitchen` connects to `Front Verandah` and `Laundry`
- `Laundry` connects to `Kitchen`
- `Back Garden` connects to `Front Verandah`
- `Garage` connects to `Front Verandah`

## The Full Objective Answer

To finish the scenario cleanly, all of these must be true:

- you are holding `House Keys`
- you reach `Garage`
- you defeat `brute_in_garage`

This means the run is not solved by only reaching the boss room or only killing the boss.

## Full Item Locations

Current item placement:

- `Torch` is a starter item
- `Battered Cricket Bat` is a starter item
- `Medkit` is in `Kitchen`
- `House Keys` are in `Laundry`
- `Barricade Kit` is in `Back Garden`

## Full Threat Placement

Current threat placement:

- `Front Gate Shambler` starts in `Front Verandah`
- `Crawler In The Weeds` starts in `Back Garden`
- `Garage Brute` starts in `Garage`

## Locked Routes

The locked locations at run start are:

- `Garage`
- `Back Garden`

Both are opened by:

- `House Keys`

If you use:

- `use house_keys`

while more than one nearby gate is valid, the game does not guess for you.

The clearer commands are:

- `unlock garage`
- `unlock back_garden`

Bare target verbs such as `unlock`, `go`, `inspect`, or `use` are rejected before the reducer runs. The game asks what target you mean instead of handing that ambiguity to narration.

## Barricade Targets

The current barricadable locations are:

- `Front Verandah`
- `Back Garden`

Both require:

- `Barricade Kit`

Important rule:

- you must be standing in the room you want to barricade

## What Each Route Means

### Front Verandah

This is the direct threshold-defense lane.

If `Front Gate Shambler` is alive and the route is not barricaded:

- waiting here causes passive pressure damage

If noise is high:

- that passive damage gets worse

If `Front Verandah` is barricaded:

- passive threshold pressure is suppressed
- direct retaliation from the shambler can be blocked
- attacks against the shambler gain `+1` damage from the better angle

### Back Garden

This is the risky flank lane.

If `Crawler In The Weeds` is alive and the route is not barricaded:

- waiting here causes passive pressure damage

If noise is high:

- that passive damage gets worse

If `Back Garden` is barricaded:

- passive pressure is suppressed
- you gain a small recovery payoff through `barricade_heal = 2`

### Garage

This is the objective room and boss finale.

It is locked until:

- you have `House Keys`
- and explicitly unlock it

If both `Front Verandah` and `Back Garden` are barricaded before the garage fight:

- `Garage Brute` retaliation is reduced by `1`
- the garage forecast and inspection text acknowledge that the exposed property approaches are secured

## Character Truth Rows

The Character tab can now spell out two useful deterministic summaries:

- utility relevance, such as the `Torch` revealing connected exits or the `Barricade Kit` listing remaining barricade targets
- siege security, including secured approaches, open approaches, finale-security payoff, and whether barricaded rooms can help noise settle

## Noise, Fully Explained

The current noise scale is:

- `0` = `Quiet`
- `1` = `Stirred`
- `2` = `Loud`
- `3` = `Swarming`

Noise is raised by:

- `attack`
- `unlock`
- `barricade`

Noise is lowered by:

- `wait` in a barricaded location

At `noise_level >= 2`:

- exposed passive pressure in `Front Verandah` and `Back Garden` becomes `2` damage instead of `1`
- retaliation in exposed outdoor fights can gain `+1` damage

## Threat Forecast Meanings

The left panel route forecast is already giving you direct answers.

Examples:

- `Front Verandah` will explicitly tell you when waiting there will cost HP
- `Back Garden` tells you when it is gated, exposed, secured, or cleared
- `Garage` tells you whether it is locked, live, or stabilized

If you want the shortest safe rule:

- trust the threat forecast more than your optimism

## Threat Inspection, Fully Explained

Inspecting an enemy or boss now tells you:

- whether it is active or defeated
- remaining HP
- whether it is present in your current room
- route-specific or finale-specific context

That means:

- `inspect crawler_in_weeds`
- `inspect shambler_front_gate`
- `inspect brute_in_garage`

are all useful commands, not flavor-only commands.

## Side-Threat Combat Identity

The two side threats now have slightly different authored fight texture.

### Front Gate Shambler

Fight identity:

- direct threshold pressure
- heavier front-lane contest
- interacts strongly with the `Front Verandah` barricade

If defeated:

- the front threshold is mechanically calmer

### Crawler In The Weeds

Fight identity:

- flank tax
- lower-to-the-ground nuisance threat
- keeps the side route actively unpleasant until removed

If defeated:

- the back route stops being actively contested by that threat

## Garage Brute Finale

The `Garage Brute` starts with:

- `8 HP`
- `3 damage`

When it drops to `4 HP` or less and is still alive:

- it enters a wounded final phase
- retaliation becomes `4` instead of `3`
- combat text and inspection text both warn you that the end phase is live

If both siege lanes are secured first:

- ordinary brute retaliation becomes `2` instead of `3`
- wounded final-phase retaliation becomes `3` instead of `4`

This is not random.

It is deterministic and tied directly to remaining boss HP.

## Best Low-Risk Route Through The Scenario

If you want a practical route with good stability:

1. Start at `Front Verandah`.
2. Use `look`.
3. Move to `Kitchen`.
4. Take the `Medkit`.
5. Move to `Laundry`.
6. Take the `House Keys`.
7. Return toward `Front Verandah`.
8. Unlock `Back Garden`.
9. Move to `Back Garden`.
10. Take the `Barricade Kit`.
11. Decide whether to barricade `Back Garden`, `Front Verandah`, or both.
12. Unlock `Garage`.
13. Enter `Garage`.
14. Fight the `Garage Brute`.

That is not the only route, but it is the most readable and least confusing one.

## Strong Safer Play Pattern

If you want the cleanest tactical read:

1. Get the `Medkit`.
2. Get the `House Keys`.
3. Open `Back Garden`.
4. Get the `Barricade Kit`.
5. Barricade `Front Verandah` if you want the direct threshold fight to become much safer.
6. Barricade `Back Garden` if you want the recovery payoff and flank stabilization.
7. Barricade both lanes if you want the garage brute's retaliation reduced.
8. Let noise settle in a secured space if the route is getting messy.
9. Only then enter the `Garage`.

## Fastest Progression Route

If you care more about speed than safety:

1. Get the `House Keys` as fast as possible.
2. Unlock `Garage`.
3. Enter `Garage`.
4. Fight the boss immediately.

This works, but it is less forgiving.

## Exact Beginner Questions

### Where is the key?

- `Laundry`

### Where is the healing item?

- `Kitchen`

### Where is the barricade material?

- `Back Garden`

### What should I unlock first?

Usually:

- `Back Garden` first if you want the full route-control toolkit
- `Garage` first only if you want to rush the finale

### Which barricade is better?

They are good for different reasons:

- `Front Verandah` is better for safer direct combat
- `Back Garden` is better for flank stabilization and the recovery bonus

### What should I save the medkit for?

Best answers:

- before the `Garage`
- after a bad outdoor exchange
- before entering the brute's wounded final phase if your HP is shaky

## Full Win Checklist

Before expecting the run to end, make sure:

- `House Keys` are still in inventory
- you are in `Garage`
- `Garage Brute` is defeated

If one of those is missing, the objective is not complete.

## After You Win

Winning does not immediately close the run.

After `WIN`:

- `look`, `inspect`, open-route movement, save, and load still work
- the UI reports the run phase as `Epilogue`
- the win banner explains that aftermath exploration is available
- the action bar shifts toward epilogue-safe commands
- rooms can show authored aftermath descriptions when the datapack provides them
- rooms can also surface small post-credits hooks for future content or media, including sidebar and media-panel cues
- `attack`, `wait`, `take`, `use`, `unlock`, and `barricade` no longer mutate the main run
- this keeps room for future datapack-authored post-credits content without letting the completed siege keep damaging or spending resources

## Recommended Commands By Situation

### You Just Spawned

- `look`
- `inspect room`

### You Found A New Item

- `inspect <item>`
- `take <item>`

### You Have Keys And A Locked Route

- `unlock <location>`

### The Forecast Says The Route Is Exposed

- consider barricading
- consider leaving
- do not `wait` carelessly

### You Are Unsure About A Threat

- `inspect <enemy_or_boss>`

### You Are Ready For The End

- `go garage`
- `attack`

### You Already Won

- `look`
- `inspect room`
- save from the top bar

## Related Documents

- [docs/ZERO_KNOWLEDGE_USER_MANUAL.md](ZERO_KNOWLEDGE_USER_MANUAL.md)
- [README.md](../README.md)
- [docs/V0_1_MANUAL_SWEEP.md](V0_1_MANUAL_SWEEP.md)
- [docs/V0_1_ACCEPTANCE_AUDIT.md](V0_1_ACCEPTANCE_AUDIT.md)
