# Ability Templates and Grants

An ability template is immutable authored pack data. It defines the action's identity, access-tag
requirements, progress pool, unlock threshold, XP award, use policy, and one host-supported
effect. It never stores an owner, charge counter, or expiry.

```toml
[[abilities]]
id = "grave_bolt"
name = "Grave Bolt"
description = "A basic necromantic strike."
tags = ["magic", "necromancy"]
required_tags = ["can_necromancy"]
progress_pool = "necromancy"
required_xp = 0
xp_on_success = 1
# `learnable` defaults to true. Set false to keep a source spell out of direct XP selection.
# learnable = false
# `scroll_learnable` independently defaults to true. Set false for a source that cannot be studied.
# scroll_learnable = false
# Omit default_uses for an infinite grant; set it for scroll/staff-style copies.
# default_uses = 3
# duration_turns = 5
# cooldown_turns = 4
# action_cost = 1

[abilities.effect]
kind = "damage"
amount = 2
```

Runtime possession is a separate `AbilityGrant`:

- `template_id` points to the immutable definition;
- `owner` is the player, any runtime actor, or an item entity;
- `remaining_uses = None` means infinite, while a number belongs only to that grant;
- `expires_at_turn` supports temporary access;
- `ready_at_turn` is the absolute world turn when this particular grant may next be used;
- `source` records `innate`, XP-choice `unlocked`, source-study `studied`, approved/reward
  `granted`, `item`, or `temporary` provenance.

An item-owned grant is available to whichever actor currently carries that item. Moving a scroll
or staff uses ordinary ownership transfer; the grant is not cloned or rewritten. A cast changes
only its selected grant's remaining uses and readiness.

## Cooldowns

`action_cost` is separate from `cooldown_turns`. A successful cast consumes the immutable
template's positive integer action cost through the common actor-readiness clock; its selected
grant then independently receives `ready_at_turn = current_world_turn + cooldown_turns`.
See [controller-neutral action timing](ACTION_TIMING_SPEC.md).

`cooldown_turns` belongs to the immutable ability template and defaults to `0`. On a successful
cast the reducer writes `ready_at_turn = current_world_turn + cooldown_turns` on only that owned
grant. The grant is usable exactly when `world_turn >= ready_at_turn`; inspection reports
`max(0, ready_at_turn - world_turn)`. No counter is decremented and no encounter boundary resets
anything: ordinary committed world actions advance the one authoritative clock, while menus and
rejected actions do not. Charges, expiry, and cooldown therefore compose naturally on a scroll,
staff, actor, or future source without sharing mutable state with any other copy of the template.

An item template can materialize an item-owned grant:

```toml
[[items.ability_grants]]
template_id = "grave_bolt"
remaining_uses = 1
```

The override is local to each materialized item instance. Omit `remaining_uses` to inherit the
ability template's default; an omitted default means infinite uses.

`learn <ability>` is deliberately different: it spends one reached XP milestone choice and creates
an infinite, permanent `unlocked` grant. Each positive authored `required_xp` value in a progress
pool is one milestone; reaching it makes one direct-unlock choice available in that pool. If two
templates both require 10 necromancy XP, the player chooses one at that milestone rather than
claiming both. Authors add later positive thresholds to create later choices. `required_xp = 0`
is for starter/source material, not a free recurring choice.
Template defaults are for a source materializing a concrete copy, never a hidden charge or expiry
policy on a learned skill. The host does not offer or commit `learn` when that owner already has
an infinite, non-expiring copy of the same template; finite, temporary, and item-bound copies do
not block a real permanent unlock.

`learnable = false` keeps a template out of the direct XP unlock tree without affecting the
generic grant/cast path. `scroll_learnable` independently controls whether a concrete item source
can teach it. That separation lets a low-level necromancy scroll be cast by any actor with
`can_necromancy`, while only a player who has permanently unlocked a matching high-level literacy
template may consume the physical source to learn it forever.

A literacy template is a passive ability template: omit `effect`, then declare the tag families it
can study. It is still acquired through the same progress pool and immutable template path:

```toml
[[abilities]]
id = "necromantic_literacy"
name = "Necromantic Literacy"
tags = ["skill", "necromancy", "scroll_literacy"]
required_tags = ["can_necromancy"]
progress_pool = "necromancy"
required_xp = 10
scroll_learning_tags = ["necromancy"]
```

`learn <carried source item>` resolves one actual item-owned grant. The host checks that the
player owns the item, the source grant is live, the source template is scroll-learnable, and the
player has a permanent usable literacy grant whose `scroll_learning_tags` intersects the source
template's descriptive `tags`. Then the reducer consumes that exact item instance, retires its
item-owned grants, creates a permanent `studied` grant without spending an XP choice, and emits `ItemConsumed` plus
`AbilityGranted`. No tag is mutated and no new ability template is invented at runtime.

`0` is never a valid authored finite value: omit `remaining_uses` or `duration_turns` for an
unbounded grant, or provide a positive count. The datapack validator and `GrantAbility` reducer
boundary both reject zero so content cannot quietly create an already-spent or immediately-expired
grant.

The reducer validates possession, expiry, finite uses, access tags, unlocked threshold, and target
before applying an effect. It emits `AbilityUsed`, any effect event, and `ProgressAwarded` in that
order. `learn <ability>` creates a permanent grant only when the owner has the template's required
tags and enough XP in its named pool.

Progress pools are owned per runtime actor. The legacy `progress_pools` field remains the literal
default player's save-compatible ledger; every other actor uses `actor_progress_pools[actor_id]`.
Control selects which actor may receive ordinary player-driven awards and unlock choices, but does
not transfer XP, milestones, or learned grants between actors. An NPC, pet, or actor-item can use
the same template and grant path without inheriting the default player's progression; when it is
controlled, its own ledger is used.

`AwardProgress { recipient_id, pool, amount }` is the corresponding host reducer primitive for
trainers, quest rewards, practice, and future systems. It accepts only a positive amount, the
currently controlled actor, and a pool named by an authored ability template, then emits one
`ProgressAwarded` event. Ability casts use that same primitive rather than writing the XP ledger
directly.

Each reached distinct positive direct-unlock milestone is also one real skill level. When a
progress award crosses one or more such milestones, the reducer emits `SkillLevelGained` and adds
the pack rule `hp_per_skill_level` to that actor's authoritative `max_hp` once per level, emitting
`MaxHpIncreased`. This automatic survival gain does not spend or replace the existing one-choice
unlock entitlement. It increases maximum HP only; current HP is deliberately unchanged, so a
level-up never doubles as an unannounced heal. Both progress and maximum HP are saved runtime
state.

The currently supported host effects are `damage`, `healing`, and `move`. Passive templates may
omit an effect when they declare another host-owned contract such as `scroll_learning_tags`; they
are not castable. Damage resolves against a valid hostile target; healing resolves a present
runtime actor by id or name (a blank target or `self` means the caster) and restores that actor's
canonical health state. A `move` cast uses its command target as the destination and delegates to
the ordinary actor-movement reducer: movement capability, legal adjacent route, locks, and
controlled-actor location rules all still apply. It emits the usual movement event plus
`AbilityUsed`; it does not gain a blink/dash-specific location mutation or silently bypass a
route. A failed movement cast spends no charge, starts no cooldown, and consumes no world time.
Neither effect family is player-only, and successful healing emits `HealthRestored`.

`status` is also a host-owned effect family. It creates an entity-owned status instance for its
caster using the existing trigger/duration evaluator rather than a spell-specific buff record.
Authors may choose
`combat`, `movement`, `inspect`, `wait`, or `failure` triggers and `scene`, `run`, or
`until_used` duration, with additive damage/healing and ordinary target/context tag gates. A
successful cast emits `StatusEffectApplied`; later ordinary reducer actions apply or consume it.
Status templates may additionally use `timing_cost_delta`, `timing_action_kinds`, and
`blocked_action_kinds` to modify or deny generic actor action categories through the shared
readiness resolver. Timing is previewed before dispatch and consumed only after a successful
commit, so an invalid action cannot burn an `until_used` timing status. A caster-owned effect does
not leak to other present actors; host-authored scene statuses remain the separate scope for
location-wide hazards and rules. A `scene`-duration caster status expires only when that caster
actually changes scene through a reducer-confirmed movement event; another actor moving cannot
clear it.

Host systems award all other possession through one `GrantAbility` action. It takes a real owner,
template id, concrete use count, optional duration, and provenance, then emits one
`AbilityGranted` event. Training, loot, temporary blessings, and future content must call that
same action rather than create noun-specific spell ownership paths.

When narrator-generated mechanical additions are enabled, `ability_grant` is a bounded approval
form rather than a new spell-definition path. The host supplies the complete list of immutable
template ids; the narrator may nominate only one of those ids plus finite/infinite uses and an
optional duration. The approval card restricts the recipient to a real player or present runtime
actor that satisfies the selected template's required access tags, and accepting it dispatches the
same `GrantAbility` reducer action. Unknown templates, incompatible recipients, locations, and
prose-only outcomes are never admitted.

Ability templates deliberately use tags such as `can_necromancy` as their school/access onramp.
Do not encode mutable quantities into tags such as `can_fireball_amount_3`: that is grant state.
