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
- barricaded waiting lowers noise
- high noise increases authored exposed-route pressure and retaliation
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
- one or two ways noise falls or stabilizes
- two or three explicit gameplay consequences

Avoid in the first pass:

- per-room sound propagation
- line-of-sight simulation
- enemy pathfinding
- hidden dice rolls
- procedural swarm spawning
- complex stealth states

## Runtime Shape

Recommended minimum truth:

- `noise_level: i32`

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
- `take`
- `equip`
- `use medkit`

This keeps the system easy to learn.

## Recommended First-Pass Action Effects

Suggested action contributions:

- `attack` raises noise by `1`
- `unlock` raises noise by `1`
- `barricade` raises noise by `1`
- `wait` lowers noise by `1` only in a barricaded location

Clamp rule:

- `noise_level` never goes below `0`
- `noise_level` never goes above `3`

This gives barricaded spaces a second useful role:

- they are not only safer
- they are also better places to let the run calm down

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

## UI Truth Surfacing

Noise must be visible outside narration.

Required first-pass surfaces:

- `Game` tab
- `Character` tab
- diagnostics

Recommended presentation:

- show `Noise: Quiet / Stirred / Loud / Swarming`
- show the numeric level in diagnostics
- include small helper text such as:
  - `Barricaded spaces help noise settle.`

The player should not have to infer the system only from punishment lines.

## Datapack Relationship

The first noise pass does not need a large new template family.

Recommended first-pass shape:

- keep `noise` mostly engine-owned as a small global state family
- allow room and scenario content to influence where noise consequences matter most

Later extensions may add data fields like:

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

- barricaded rooms are better places to wait and let noise settle
- exposed rooms become more punishing at high noise

This creates a clear tactical contrast:

- barricades manage safety and route control
- noise manages escalation and attention

## Save/Load

Noise state must round-trip through save/load.

Required behavior:

- `noise_level` restores exactly
- restored noise continues to affect subsequent reducer outcomes correctly

## Test Coverage

Minimum automated coverage:

- generated run starts at `noise_level = 0`
- loud actions raise noise deterministically
- noise clamps at the defined maximum
- quiet recovery in a barricaded space lowers noise deterministically
- passive pressure scales correctly at higher noise where authored
- save/load preserves `noise_level`
- diagnostics or derived models show the current noise truth

## Explicit Non-Goals

Do not include these in the first noise pass:

- stealth takedowns
- hearing-radius simulation
- free-roaming horde entities
- random encounter spawning
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
