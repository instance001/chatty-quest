# Controller-neutral action timing

`RunState.turn_index` is the one authoritative integer world-time cursor. It is not player
speed. Runtime state also persists `actor_ready_at[entity_id]`, an absolute timestamp. An actor
may begin an ordinary action only when `turn_index >= actor_ready_at[entity_id]`.

After the reducer successfully commits an ordinary action, it writes:

```text
actor_ready_at[origin] = current_world_time + resolved_action_cost
world_time += resolved_action_cost
```

Rejected and movement-blocked actions do not alter readiness, location, or world time. Missing
readiness values in old saves mean ready at time zero.

Control only selects the origin and request. It never supplies a mechanical speed. The same
runtime entity therefore has identical timing whether driven by a player command, autonomous AI,
a pet command, or a future controller.

## Authored costs

All costs are positive integer world-time units and default to `1` for compatibility:

```toml
[[enemies]]
id = "shambler"
traversal_cost = 15
attack_cost = 10

[[npcs]]
id = "runner"
[npcs.actor]
can_move = true
traversal_cost = 7
combat = true
hp = 8
damage = 2
attack_cost = 7

[[items]]
id = "heavy_hammer"
damage = 5
attack_cost = 18

[[abilities]]
id = "fireball"
action_cost = 12
cooldown_turns = 40
```

`traversal_cost` belongs to an actor profile (or legacy encounter template), native actor combat
uses that actor's `attack_cost`, and an equipped item may supply the controlled actor's basic
attack cost. `AbilityTemplate.action_cost` is the cost of that cast.

## Cooldown is separate

Ability grants retain their own `ready_at_turn`. A successful cast updates both independent
concepts: the actor is unavailable until its action readiness time, while that *specific grant*
is unavailable until its cooldown time. Charges, expiry, and cooldown are grant state; they are
not actor speed.

## Status timing modifiers and gates

Status effects remain evaluated by the generic effect substrate. A status-producing ability may
also declare:

```toml
[abilities.effect]
kind = "status"
trigger = "movement"
duration = "until_used"
timing_cost_delta = 5
timing_action_kinds = ["movement"]
# blocked_action_kinds = ["movement"]
```

`timing_action_kinds` selects the generic action categories (`general`, `movement`, `combat`,
`ability`, `wait`) to adjust. `timing_cost_delta` is additive and the reducer clamps final cost
to at least one tick. `blocked_action_kinds` rejects a matching action before it changes state.
An ability cast is always in the `ability` category; a damage effect additionally has the
`combat` category and a move effect additionally has the `movement` category. Thus a generic
movement gate also governs movement performed through a skill or spell rather than leaving a
template-name loophole.
The preview is read-only; an `until_used` timing status is consumed only after a successful
reducer commit. This is the generic seam for authored haste, slow, root, and stun-like content;
do not create status-specific movement or actor-kind systems.

Status ownership is authoritative scope: legacy `player` means the literal default `player`
entity, `entity` applies only to its exact runtime entity id, and `scene` applies in the current
scene (optionally narrowed to one location id). A controller swap does not silently retarget a
legacy player status; new actor-specific statuses must use `entity`. Ability-created statuses are
entity-owned by their caster. A personal root or haste therefore cannot become a global rule, or
follow a different controller, merely because several actors share a tile.

The same origin-and-trigger resolver is used after a confirmed controlled action, direct actor
action, or autonomous AI action. In particular, movement, wait, and autonomous hazard-combat
effects do not disappear when the actor is driven by AI, and healing resolves against the acting
entity's own health contract rather than implicitly against the player.

A successful move ability is also a movement trigger for ordinary status effects and scene
duration handling. Damage abilities instead resolve their combat effects in the target-aware
combat path exactly once; they are not replayed through the origin-only effect path.
