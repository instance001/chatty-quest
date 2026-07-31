# `v0.2` Noise Pressure Spec

## Purpose

This document defines the next deterministic pressure family for `Property Siege Classic`:

- `noise`

The goal is not to build a stealth simulator.

The goal is to give the zombie scenario a second real tension axis that:

- is easy to understand
- is easy to surface in UI
- is reducer-owned truth
- interacts cleanly with the current barricade system

Status note:

- this mechanic is implemented on the current branch
- loud actions raise global `noise_level`
- successful non-noisy actions lower noise over time
- high noise increases authored exposed-route pressure and retaliation
- crossing into max noise spawns one template-backed enemy instance into an outdoor yard location
- UI, diagnostics, save/load, and reducer tests all surface or preserve noise truth

## Why Noise Next

Noise is the best next pressure family after barricades because it answers a different question.

Barricades answer:

- how do I secure a vulnerable space?

Noise answers:

- how much attention am I drawing while I survive and move?

That makes the two systems complementary instead of redundant.

Noise also fits the current pack well because it:

- is immediately legible in a zombie siege
- creates tension without requiring more rooms
- works with movement, combat, waiting, and utility actions
- can stay small and deterministic

## Design Intent

The intended feel is:

- the player can make the property safer through barricades
- but bad decisions can still make the property louder and more dangerous

This creates a better siege rhythm:

- secure space
- manage attention
- choose when to hit hard and when to stay quiet

The important rule is still:

- narration may dramatize the noise
- the reducer owns the noise truth

## First-Pass Scope

The first noise pass should stay very small.

Recommended shape:

- one global `noise_level`
- a short list of actions that raise noise
- one or two ways noise falls or stabilizes over time
- two or three explicit gameplay consequences

Avoid in the first pass:

- per-room sound propagation
- full line-of-sight simulation beyond adjacent legal map spaces
- enemy pathfinding
- hidden dice rolls
- broad procedural swarm spawning
- complex stealth states

## Runtime Shape

Recommended minimum truth:

- `turn_index: u64`
- `noise_level: i32`
- `noise_spawn_count: u32`
- `spawned_enemy_targets: HashMap<String, String>`
- `spawned_enemy_origins: HashMap<String, String>`
- `spawned_enemy_searching: HashSet<String>`
- `spawned_enemy_sight_targets: HashMap<String, String>`
- `spawned_enemy_sight_subjects: HashMap<String, String>`
- `spawned_enemy_sight_delays: HashMap<String, u8>`

Enemy and boss templates expose sense flags:

- `can_hear: bool`
- `can_see: bool`

Both default to `true` for backward compatibility with older datapacks.

Scenario rules expose tuning and finale-security hooks:

- `sight_acquire_chance_percent`
- `sight_chase_delay_chance_percent`
- `spawned_hazard_break_chance_percent`
- `finale_target_location_id`
- `finale_boss_id`
- `finale_secured_location_ids`
- `finale_secured_retaliation_reduction`

Current `Property Siege Classic` percentage values are `70`, `35`, and `35`; its finale-security hook targets `garage`, `brute_in_garage`, and the secured locations `front_verandah` plus `back_garden`.

Suggested first-pass range:

- `0` to `3`

Recommended semantic labels:

- `0` = quiet
- `1` = stirred
- `2` = loud
- `3` = swarming

The numeric level is useful for tests and save/load.

The label is useful for UI and narration.

Scale note:

- a single global `noise_level` is acceptable for a small bounded scenario like `Property Siege Classic`
- larger scenario packs should not blindly treat every loud event as map-wide awareness
- if future zombie or urban-scale packs need broader coverage, that logic should live behind dedicated helper functions or scenario-specific resolver seams rather than being copied as raw global noise assumptions everywhere
- practical reading: a creaky floorboard in one building should not automatically alert an entire city map

## Reducer Rule

Noise should be explicit state, updated through deterministic action handling.

The reducer should:

- raise noise on loud actions
- optionally lower it on quiet recovery actions
- check current noise when resolving specific consequences
- surface state changes clearly in reducer lines

No hidden accumulation should exist outside structured state.

## Recommended Noise Triggers

Good first-pass triggers:

- `attack`
- `unlock`
- `barricade`

Optional trigger:

- `wait` in an unsafe or exposed space if we want ambient escalation

Actions that should stay quiet in the first pass:

- `look`
- `inspect`
- `move`
- `take`
- `equip`
- `use medkit`

This keeps the system easy to learn.

## Recommended First-Pass Action Effects

Suggested action contributions:

- `attack` raises noise by `1`
- `unlock` raises noise by `1`
- `barricade` raises noise by `1`
- any successful non-noisy action lowers noise by `1`
- rejected or blocked actions do not lower noise

Clamp rule:

- `noise_level` never goes below `0`
- `noise_level` never goes above `3`

This gives quieter turns a second useful role:

- they advance the situation without drawing more attention
- they let the run calm down when the player stops making loud moves

## First-Pass Consequences

The first pass should use small authored consequences, not broad simulation.

Recommended consequences:

### 1. Passive Pressure Escalation

At higher noise, passive pressure in exposed rooms becomes harsher.

Recommended first reading:

- `front_verandah` and `back_garden` passive pressure damage can scale with noise when not barricaded

Example:

- low noise: passive pressure damage `1`
- high noise: passive pressure damage `2`

### 2. Combat Pressure Escalation

At higher noise, live threats hit harder in authored situations.

Recommended first reading:

- direct retaliation from non-barricaded room fights can gain `+1` damage at `noise_level >= 2`

This should be used carefully and only where the room is already authored as exposed.

### 3. Objective-Route Tension

The player should feel that loud play creates a rougher route to the garage even if the map itself does not change.

The point is not to punish every action.

The point is to make loud play visibly cost something.

### 4. Max-Noise Spawn

When a loud action raises noise into `Swarming`, the reducer spawns one runtime enemy instance.

Current first reading:

- choose an enemy template from the scenario enemy pool
- only templates with `can_hear = true` are eligible for noise spawning
- choose a yard location from locations tagged `outdoor`, with `yard` or `garden` id fallbacks
- create a runtime enemy id such as `noise_spawn_1_shambler_front_gate`
- copy HP and combat/media/narrator identity from the selected enemy template
- add the instance to `enemies_alive`, `enemy_hp`, and the selected `location_enemies` bucket
- remember the map location where noise crossed into `Swarming` as the spawned instance target
- remember the spawned instance's first placed location as its origin

The selection is deterministic from run state, accepted turn index, and spawn count. It should feel like a random pool pull to the player, while staying replayable for tests, saves, and future handoff traces.

### 5. Spawned-Enemy Movement

Existing spawned enemies get a simple reducer-owned turn after successful player actions.

Current first reading:

- a newly spawned enemy does not move on the same action that created it
- on later successful turns, it may wait or move one connected map tile
- for `Property Siege Classic`, spawned enemies path toward the latest successful noisy action location
- the first target is the location where the player made noise reach `Swarming`
- later successful noisy actions shift existing spawned-enemy targets to the new source, even if noise is already capped
- spawned enemies whose source template has `can_hear = false` do not acquire new noise attractors
- when a spawned enemy reaches its current target, it enters search mode instead of endlessly treating the reached tile as fresh noise
- search mode can wait, move one legal adjacent tile, or move back toward the spawned enemy's origin when no new attractor has appeared
- search and lost-trail lines use deterministic flavor variants so repeated ticks do not all read identically
- a new successful noisy action clears search mode for affected spawned enemies and retargets them to the new source
- spawned enemies can acquire a sight attractor when the player is in the same tile or an adjacent legally visible tile
- spawned enemies whose source template has `can_see = false` cannot acquire sight attractors
- spawned enemies can acquire another live enemy or boss by sight when they are searching or otherwise not committed to an active noise target
- sight acquisition has a deterministic success chance once a valid sightline exists, tuned by `rules.toml`
- a failed sight acquisition emits a structured miss event and does not create a sight target
- if an existing visual chase fails its sight check, the chaser emits a structured lost-sight event and drops into the shared search loop
- player sight takes priority over noise; non-player sight does not override an active noise chase
- sight chases have a deterministic chance to take an extra tick before moving, tuned by `rules.toml`
- if visual contact is broken during that delayed chase window, the chaser drops into the shared search loop
- for broader datapacks, the fallback behavior chooses a deterministic legal adjacent tile
- locked destination locations block movement
- barricaded current or destination locations block movement
- when the next route blocker is a barricade or locked gate, the spawned enemy attacks that hazard with a deterministic break chance tuned by `rules.toml`
- a broken barricade is removed from `barricaded_locations`
- a broken locked gate is removed from `locked_locations` and added to `broken_locked_locations`
- blocked or rejected player actions do not advance spawned enemies

This is not full AI. It is a small map-pressure behavior that proves runtime instances can move without bypassing authored gates.

### Spawned-Enemy Attractor Priority

Spawned enemies resolve their current attractor in one deterministic order.

| Priority | Attractor | Gated by | Notes |
| --- | --- | --- | --- |
| 1 | Player sight | `can_see`, legal sightline, sight acquisition roll | Overrides an active noise target when acquired. |
| 2 | Active sight track | Existing `spawned_enemy_sight_targets` state | Can be delayed by the sight chase delay rule; lost or failed sight drops into search. |
| 3 | Noise target | `can_hear`, `spawned_enemy_targets` state | Fresh successful noisy actions can retarget hearing-capable spawned enemies. |
| 4 | Search fallback | Search state and origin state | Waits, wanders, or returns toward origin when no stronger attractor is active. |

Non-player sight is deliberately weaker than player sight. A spawned enemy can acquire another live enemy or boss by sight only when it is searching or not already committed to an active noise target.

## UI Truth Surfacing

Noise must be visible outside narration.

Required first-pass surfaces:

- `Game` tab
- `Character` tab
- diagnostics

Recommended presentation:

- show `Noise: Quiet / Stirred / Loud / Swarming`
- show the numeric level in diagnostics
- surface template senses on enemy and boss inspect output
- surface active spawned-enemy attractors in sidebar and diagnostics
- include small helper text such as:
  - `Barricaded spaces help noise settle.`

The player should not have to infer the system only from punishment lines.

## Datapack Relationship

The first noise pass does not need a large new template family.

Recommended first-pass shape:

- keep `noise` mostly engine-owned as a small global state family
- allow room and scenario content to influence where noise consequences matter most

Location tags and later extensions may add data fields like:

- `noise_pressure`
- `noise_sensitive = true`
- `noise_decay_bonus = 1`
- `noise_damage_bonus = 1`

If later scenario packs need district-scale, building-scale, or room-cluster-scale sound behavior, the engine should prefer:

- helper-owned scope resolution
- scenario-authored pressure zones
- explicit reducer-visible translation from local event to affected area

It should not rely on ad hoc copies of a single demo-pack global rule.

That should not be required to prove the first pass.

## Barricade Relationship

Noise should reinforce the barricade system rather than replace it.

Recommended interaction:

- wait and other successful non-noisy actions let noise settle
- barricaded rooms remain better places to wait because they reduce or avoid authored pressure
- exposed rooms become more punishing at high noise

This creates a clear tactical contrast:

- barricades manage safety and route control
- noise manages escalation and attention

## Save/Load

Noise state must round-trip through save/load.

Required behavior:

- `noise_level` restores exactly
- `noise_spawn_count` restores exactly
- `spawned_enemy_targets` restores exactly
- `spawned_enemy_origins` restores exactly
- `spawned_enemy_searching` restores exactly
- `spawned_enemy_sight_targets` restores exactly
- `spawned_enemy_sight_subjects` restores exactly
- `spawned_enemy_sight_delays` restores exactly
- broken-open gate state restores exactly
- restored noise continues to affect subsequent reducer outcomes correctly

## Test Coverage

Minimum automated coverage:

- generated run starts at `noise_level = 0`
- loud actions raise noise deterministically
- noise clamps at the defined maximum
- crossing into max noise spawns a template-backed enemy instance in an outdoor yard location
- already-max noise does not spawn another enemy unless noise first drops and then reaches max again
- existing spawned enemies can move one legal tile toward their stored noise source
- successful noisy actions retarget existing spawned enemies to the latest noise source
- spawned enemies enter search mode after reaching the current attractor
- searching spawned enemies can choose to return toward their origin
- spawned enemies can acquire the player by sight and chase that visual attractor
- spawned enemies can acquire another live encounter by sight when not actively chasing noise
- spawned enemy sight acquisition can fail before creating a sight target
- delayed sight chases can be shaken into search when the target is no longer visible
- barricades and locked destinations can make spawned enemies wait
- spawned enemies can attack blocking barricades or locked gates and sometimes break them
- broken locked gates are surfaced separately from normally unlocked gates
- quiet recovery through successful non-noisy actions lowers noise deterministically
- rejected or blocked actions do not lower noise
- passive pressure scales correctly at higher noise where authored
- save/load preserves `noise_level`
- deterministic rolls continue to vary after `rolling_summary` reaches its cap
- diagnostics or derived models show the current noise truth

## Explicit Non-Goals

Do not include these in the first noise pass:

- stealth takedowns
- hearing-radius simulation
- free-roaming horde entities
- broad random encounter systems beyond the max-noise yard spawn
- per-room audio graphs
- NPC hearing logic
- procedural swarm AI

## Recommended Implementation Order

1. add `noise_level` to `RunState`
2. add reducer helpers for raising, lowering, and labeling noise
3. apply noise updates to a small set of existing actions
4. add one or two authored consequence checks in exposed rooms
5. surface noise truth in UI and diagnostics
6. add tests and save/load coverage
7. update manual sweep and milestone docs once stable

## Recommended First Content Reading

If we want the smallest honest first implementation, the best reading is:

- loud play makes exposed spaces rougher
- barricaded spaces help the run settle down

That is enough to prove the system cleanly before deciding whether later noise work should become broader or stay tightly authored.

## Current Role In `v0.2`

Noise is now a completed first-pass foundation.

The next noise work should wait until another scenario or authored content beat proves that global noise is too coarse.
