# Template Capability Reference

Last updated: `2026-09-29`

## Purpose

This is the practical, current-state authoring reference for a modder asking: “If I add this thing, what can it actually do?” It describes mechanics implemented by the engine today, not planned fields or narration-only ideas.

Use it with [DATAPACK_SPEC.md](DATAPACK_SPEC.md) for folder layout and validation detail, and with [MODDING_GUIDE.md](MODDING_GUIDE.md) for media and narrator boundaries.

## First: Authored Templates vs Runtime Proposals

An authored datapack can currently define these files:

| You want to add… | Author it in | What it is for |
| --- | --- | --- |
| A place | `templates/locations.toml` | map connections, locks, barricades, placement, pressure, and presentation |
| An item | `templates/items.toml` | equipment damage, healing, route reveal, keys, and barricade materials |
| A regular hostile | `templates/enemies.toml` | combat encounter, senses, placement, and combat text |
| A major hostile | `templates/bosses.toml` | boss combat, wounded phase, finale hooks, and placement |
| A scenario goal | `templates/objectives.toml` | required item, location, and/or boss for completion |
| A non-hostile or optionally hostile actor | `templates/npcs.toml` | placement, disposition, dialogue/narration presence, quest references, and optional actor capabilities |

`templates/npcs.toml` is optional so older packs remain compatible. Statuses and temporary exits
remain runtime-only proposal types; they are not full authored datapack template files.

## Tags: What Is Mechanical Today

Tags are free-form strings. Most are useful for search, media, diagnostics, or narration context; they do nothing on their own. The following table is the complete set of tag meanings currently implemented by the engine.

| Template | Tag | Actual effect |
| --- | --- | --- |
| Item | `starter_item` | The item enters starting inventory. The first starter item in file order begins equipped. |
| Item | `healing` | `use <item>` restores 4 HP up to the maximum, then consumes that item. |
| Location | `outdoor` | Makes the location eligible as a deterministic noise-spawn location. |
| Location | `noise_pressure` | At noise 2 or higher, an unbarricaded fight here gives a normal enemy +1 retaliation damage. |
| Enemy or boss | any tag named by an active status effect | Can satisfy that effect's required target tags during combat. |
| Location | any tag named by an active status effect | Can satisfy that effect's required context tags during combat. |

`starter_item`, `healing`, `outdoor`, and `noise_pressure` are exact current engine spellings. Status-effect tag matching is case-insensitive. The current noise-spawn selector also treats location ids containing `yard` or `garden` as eligible, so `outdoor` is the clear portable choice but not its only fallback. Other tag lists—such as `weapon`, `consumable`, `single_use`, `boss`, `ambush`, `loot_possible`, or `objective_target`—are currently descriptive unless a specific future system begins to consume them. In particular, `weapon` does not make an item deal damage; the item's `damage` field does.

### Host-Owned Engine Capabilities

#### Dependent-contract invariant

Every capability listed below is a schema root. Its dependent configuration is applicable only
while the capability is enabled, in authored templates, runtime proposals, editable approval
cards, persistence, and future mutations. The registry—not an entity kind—owns the prerequisite,
default, and validation rule. Disabling a capability retracts its dependent runtime mechanics.
This is the extension seam for future capability families; it does not imply that unimplemented
systems such as trade or quest rewards already exist.

Actor-capable templates may select `capabilities = [...]`. This is a host-owned, validated
vocabulary—not a descriptive tag list and not executable pack logic. Every listed capability is
implemented and validated by the Chatty-Quest engine; packs only opt a template in and configure
its documented parameters. `behavior_tags` is accepted as a legacy input alias, but new packs
must use `capabilities`.

For an explicit `capabilities` selection, both sides of a dependency must be present: setting
`actor.can_hear = true` alone does not satisfy `noise_track`; the template must also select the
`hearing` root. Likewise, stale HP, sight, or movement values do not keep child mechanics alive
after their parent capability is removed. Empty capability lists are the narrowly scoped
compatibility case for older saves and templates that predate this registry.

At materialization, a non-empty list replaces inferred legacy capabilities; it is not merged
with them. This means an authored `capabilities = ["inventory"]` actor may retain old scalar
fields for save compatibility, but it is inventory-only in the reducer.

| Capability | Required configuration | Implemented effect |
| --- | --- | --- |
| `health` | positive `actor.hp` | Gives a materialized actor host-owned health state. |
| `combat` | `actor.hp` and non-negative `actor.damage` | Enables the shared combat state contract. Any active runtime actor may use host-validated actor combat; a friendly actor in the player's location automatically attacks a valid hostile during a `wait` tick. Disposition still decides hostility. |
| `hearing` | `actor.can_hear = true` | Enables host-owned hearing checks. |
| `sight` | `actor.can_see = true` | Enables host-owned sight checks. |
| `movement` | `actor.can_move = true` | Enables the shared actor movement reducer. An active actor may receive a bounded named instruction such as `Lira, go kitchen`; pool actors additionally use the same reducer path for deterministic autonomous movement. |
| `inventory` | no numeric parameter | Gives the admitted runtime actor an authoritative inventory endpoint. Existing item instances may move to or from it only through host-validated `TransferItem`; it does not create loot or permit narrated handoffs. |
| `noise_spawn` | hearing | Makes an eligible actor a candidate for the deterministic maximum-noise pool. |
| `noise_track` | hearing and movement | Lets a pool actor retarget toward the latest noise source. |
| `sight_track` | sight and movement | Lets a pool actor acquire visible attractors through deterministic sight checks. |

Existing enemies with no `capabilities` retain their old implicit noise/sight/movement behavior for
pack compatibility. Once an enemy or boss supplies any capability selection, that envelope is
persisted on its runtime encounter instance and governs health, combat, senses, movement, and
autonomous behavior alike. NPCs and bosses must opt in explicitly. A hostile with `health` but
without `combat` can be damaged but cannot retaliate or attack hazards. Any active runtime NPC or
world-item actor that selects `noise_track` or `sight_track` joins the shared autonomous loop;
`movement` alone is an action permission, not an instruction to wander. An item that selects
`noise_spawn` and meets its actor prerequisites materializes as a distinct world-item actor; a
plain item remains inert.

## Item Capabilities

Every item needs `id`, `name`, `description`, `tags`, and may have `narrator_brief` and `media`.

| Field or combination | What it actually does |
| --- | --- |
| `damage = N` | When equipped, adds `N` to the currently controlled actor's ordinary attack damage. The tag list is irrelevant to this. |
| `tags = ["starter_item"]` | Places the item in starting inventory. |
| `tags = ["healing"]` | Makes `use <item>` heal 4 HP and remove the item. It can coexist with other tags. |
| `utility_effect = "reveal_connections"` | `use <item>` marks all authored connections from the current room as known. |
| Location `locked = true` plus `unlock_item_id = "this_item"` | The item can unlock that reachable location through `unlock <location>` or a sufficiently unambiguous `use <item>`. |
| Location `barricadable = true` plus `barricade_item_id = "this_item"` | The item is required for `barricade <location>`. Barricading does not consume it. |
| `inspect_lines`, `pickup_line`, `objective_required_line`, `objective_not_required_line` | Presentation text only, shown at the matching inspect, pickup, or objective context. |
| `utility_success_line`, `utility_empty_line` | Presentation text for `reveal_connections`. |

Only two `utility_effect` values validate today: `reveal_connections` and `barricade`. `reveal_connections` has an implemented `use` action. `barricade` is recognized for authoring and the utility UI, but the actual state change is still driven by the target location and `barricade <location>`; it is not a separate generic `use` effect.

## Location Capabilities

Every location needs `id`, `name`, `description`, `tags`, and normally `connections`. Locations also own initial item/enemy/boss placement through `items`, `enemies`, and `bosses` lists.

| Field or combination | What it actually does |
| --- | --- |
| `connections = ["other_id"]` | Creates an authored movement edge to an existing location id. Connections are validated. |
| `locked = true` + `unlock_item_id` | Starts the destination locked and requires the named authored item. |
| `barricadable = true` + `barricade_item_id` | Enables `barricade <location>` while standing there and holding the named item. |
| `barricade_heal = N` | Restores up to `N` HP once when that location is successfully barricaded. |
| `barricade_blocks_retaliation = true` | Prevents normal-enemy retaliation in that barricaded location. |
| `barricade_attack_bonus = N` | Adds `N` to player damage against normal enemies while the location is barricaded. |
| `passive_pressure_enemy_id` | `wait` hurts the player while that named enemy is alive there and the location is not barricaded. Damage is 1, or 2 at noise 2+. |
| `passive_pressure_*_line` | Text shown for that wait-pressure system; the fields do not create pressure by themselves. |
| `route_note`, `threat_forecast_*`, `movement_context_*`, `situation_*`, `boss_*_line` | Contextual UI/log presentation only. Listed placeholders are rendered where supported. |
| `epilogue_description`, `epilogue_hook` | Post-win presentation only; no objective, item, route, or state is created. |

Lock and barricade references must point to existing item ids. A non-barricadable location may not declare barricade configuration. Use explicit fields rather than a tag when you need one of these mechanics.

## Enemy Capabilities

Enemies need `id`, `name`, `description`, `tags`, `hp`, and `damage`. Put their ids in a location's `enemies` list to create an authored encounter there.

| Field | What it actually does |
| --- | --- |
| `hp` | Starting health. Attacks reduce it deterministically; zero removes the hostile. |
| `damage` | Retaliation damage after the player attacks and does not defeat the enemy. |
| `can_hear = false` | Excludes this enemy template from noise-spawn selection and prevents its spawned copies from using hearing-based attraction. Default is `true`. |
| `can_see = false` | Prevents spawned copies from using sight acquisition/chase behavior. Default is `true`. |
| `capabilities` | Optional explicit selection of host-owned behavior; see Host-Owned Engine Capabilities. |
| `retaliation_line`, `defeat_line`, `inspect_alive_line`, `inspect_defeated_line` | Matching combat/inspection presentation only. |
| `tags` | Available to active status effects as target tags; otherwise descriptive. |

An enemy's presence is created by location placement. Merely declaring it in `enemies.toml` does not place it in a room, though it may still be selected as a noise-spawn template if `can_hear` is true.

## NPC Capabilities

NPCs need `id`, `name`, `description`, and `tags`; they may use `narrator_brief` and `media` like
other presentation templates. Add their ids to a location's `npcs` list for authored placement.

| Field | What it actually does |
| --- | --- |
| `disposition = "friendly"`, `"neutral"`, or `"hostile"` | Explicitly controls hostility. The default is `neutral`; HP alone never makes an NPC attackable or hostile. |
| `faction` | A visible/narrator-facing affiliation label. It is not a diplomacy or reputation system. |
| `quest_objective_ids` | Explicit references to existing authored objective ids. They are shown in NPC context and narration; they do not create a separate quest engine. |
| `[npcs.actor] hp`, `damage`, `combat` | Opts the NPC into shared health/combat state. A hostile NPC with `combat = true`, positive `hp`, and a non-negative `damage` can be attacked and retaliate. |
| `[npcs.actor] can_hear`, `can_see` | Shared perception assignment. With matching host capabilities, a materialized pool NPC uses the same deterministic noise and sight checks as other mobile actors. |
| `[npcs.actor] can_move` | Enables movement only for a materialized pool NPC with `capabilities = ["movement", ...]`; ordinary authored placement remains stationary. |
| `capabilities = ["noise_spawn", "noise_track", "sight_track", "movement"]` | Opts an NPC into the deterministic noise pool and shared tracking loop. `noise_track` requires hearing; `sight_track` requires sight; both require `movement`. |

An NPC with no actor combat configuration is a non-combatant. Friendly and neutral NPCs remain
non-hostile even if they have health or perception values. A friendly actor that does have the combat
contract may independently attack a valid hostile in the player's current location on a host `wait`
tick; the resulting `AttackResolved` event names that actor as the origin. NPC death is deterministic:
a defeated hostile combat NPC is removed from its location bucket.

## Runtime Ownership and Transfers

Ownership is entity-agnostic. The controlled player, NPCs, and actor-capable item instances with
the `inventory` capability are eligible inventory endpoints; locations are world endpoints. An
actor-capable item can therefore be a real container without introducing a backpack-specific
mechanic. The host
validates every move as one `TransferItem` action, then emits one `ItemTransferred` reducer event.
The narrator may describe that confirmed event but cannot create a handoff with prose.

Current player commands are `take <item>`, `drop <item>`, `give <item> to <actor>`, a named
recipient request such as `<actor>, take my <item>`, and a clear request for a named
inventory-capable actor to give an item it actually owns. All forms resolve to the same
`TransferItem` reducer action; wording never creates a second NPC-only handoff path. Dropping an
equipped item clears the old equipment slot. Inspecting an inventory-capable actor shows its
authoritative contents, so a player can verify what it can genuinely offer.

A completed player-to-actor transfer emits a generic `Gift` social event; actor-to-player return
emits `ReturnItem`. The host stores a reversible relationship score and derives the visible
Friendly/Neutral/Hostile disposition from central personality thresholds. Personality is supplied
by generic tags (`cheerful`, `dour`, `trusting`, `suspicious`, `stubborn`, `volatile`) rather than
by NPC classes. The same threshold helper is the extension point for future negotiation and
forgiveness mechanics; it does not make those mechanics available today.

Current bounded social language is `help`, `assist`, or `aid` a named present actor; `insult`,
`mock`, `taunt`, or clear targeted hostile language one; `threaten` one; or `betray` one. Those
commands identify an existing actor and emit a reducer event; they do not set disposition directly.
`steal <item> from <actor>` uses the same authoritative `TransferItem` primitive, but only when
the named co-located, non-hostile inventory-capable actor actually owns that item. A successful
reducer-confirmed move emits `Theft` after `ItemTransferred`; an unknown item, unavailable actor,
or hostile endpoint changes neither ownership nor social state.
`attack <target>` and `hit <target>` preserve the named target through parsing. The reducer
resolves the currently controlled actor, validates its combat contract, and applies ordinary
combat only to a valid present hostile. A non-hostile named target does not become a free assault
path; the host may instead resolve explicitly bounded social language such as `threaten <actor>`.
No target form is allowed to narrate a hit without a reducer-confirmed combat event.

## Actor-Capable Items

Items may opt into the same state shape with an `[items.actor]` table containing `hp`, `damage`,
`can_hear`, `can_see`, `can_move`, `combat`, and optional `disposition`, plus host-owned
`capabilities` at item level. This preserves capability composition without a sentient-item special
case. An item with `noise_spawn` materializes as a world-item instance and uses the shared
movement/perception pool when its selected capabilities and prerequisites permit it. A plain item
with no `[items.actor]` remains inert. An actor-capable item with the `inventory` capability owns
its nested contents through the same `TransferItem` action as every other endpoint; its contents
persist with the run and are never implied by narration. Its effective world location follows its
authoritative owner: a carried actor-capable item is co-located with the carrier, so it can receive
the same bounded named combat instruction and friendly `wait`-tick combat path as a world actor.
It never receives an item-specific combat controller. `noise_spawn` remains a world-materialization
capability; merely carrying an item does not make it enter the noise-spawn pool.

## Boss Capabilities

Bosses use the same core combat fields as enemies and are placed through a location's `bosses` list.

| Field | What it actually does |
| --- | --- |
| `hp`, `damage` | Starting boss health and retaliation damage. Boss retaliation never falls below 1. |
| `wounded_phase_hp_threshold = N` | Activates the wounded phase while boss HP is above 0 and at or below `N`. `N` must be positive and less than starting HP. |
| `wounded_phase_damage_bonus = N` | Adds `N` retaliation damage during the wounded phase. It cannot be negative. |
| `wounded_phase_*` fields | Text for wounded combat and inspection states. |
| `defeat_line`, `retaliation_line` | Boss combat presentation. `{boss_name}` and `{damage}` are supported in the relevant lines. |
| `finale_security_retaliation_line` | Presentation for the pack-level finale security reduction; supports `{boss_name}`, `{damage}`, and `{reduction}`. |
| `can_hear`, `can_see`, `tags` | Same current meanings as enemy fields. |
| `capabilities` | Optional explicit opt-in to the shared noise pool and movement/tracking loop. Bosses are placed encounters unless this is configured. |

The mechanical finale-security reduction is configured in `rules.toml`, not on the boss itself: `finale_target_location_id`, `finale_boss_id`, `finale_secured_location_ids`, and `finale_secured_retaliation_reduction` must agree with real authored ids.

## Objective Capabilities

Objectives need `id`, `name`, `description`, and `tags`. The current generator selects the first objective in `objectives.toml` as the active authored objective.

| Field | What it actually does |
| --- | --- |
| `required_item_id` | The named item must still be in inventory for completion. |
| `required_location_id` | The player must be at that location for completion. |
| `target_boss_id` | The named boss must be defeated for completion. |
| `tags` | Currently descriptive only. |

Any combination of these three requirements is supported. Omit a field to omit that requirement. There is no authored reward, objective scripting, quest chain, or arbitrary completion expression field yet.

## Runtime-Only Proposal Kinds

With a compatible play contract and player approval, the narrator proposal system can create
run-scoped items, statuses, NPCs, enemies, bosses, objectives, or temporary exits. Narrator-created
NPCs use the existing runtime NPC path; they do not rewrite `templates/npcs.toml`. The host selects
the allowed kind and fields, validates them, lets the player edit the form, then sends an accepted
proposal through a reducer.

Proposal approval is also the effective-mechanics inspection point. The host displays the complete
resulting contract rather than only narrator-filled prose. Item and NPC proposals expose the shared
actor capability envelope as editable host-owned fields; accepted runtime entities and saved player
library templates keep that same `ActorState`/disposition. Other proposal kinds show their currently
implemented, locked runtime contract. A narrator never silently grants an unshown capability.

This is useful for emergent play, but should not be used to replace authored scenario design. Runtime proposals never rewrite a datapack. A temporary exit can only connect existing authored locations; a proposal cannot manufacture a new room id.

### Status Proposal Contract

Status proposals are bounded effects, not narrative labels. The current host exposes `trigger`,
`duration`, additive damage/healing, and optional target/context-tag gates on the approval card.
`combat` effects may add damage to the controlled player's combat action; `movement`, `inspect`,
`wait`, and `failure` effects may add healing when their host event occurs. The reducer rejects a
zero-effect status and combinations the current evaluator cannot execute. This deliberately small
contract leaves room for future status branches without admitting decorative state today.

## Rules And Narration Settings

`rules.toml` owns scenario-wide configuration such as start location, boundaries, noise/sight percentages, spawned-actor movement policy, finale security, and narration routing. It does not declare new mechanics. If a desired mechanic is absent from the host capability table, it is an engine capability gap—not a reason to put private gameplay logic in a pack.

The player, not the datapack, selects narrator backend/model/provider, API key, narration-event preferences, and proposal looseness. Packs may route each narration hook to `none`, `mock`, or `selected_narrator`, but cannot give the narrator direct state authority.

## Authoring Checklist

1. Give every template a stable lowercase `snake_case` id and a nonblank name.
2. Place each item, enemy, and boss from a location list; definitions alone do not place them.
3. Use an explicit mechanical field for every intended behavior. Do not assume a descriptive tag adds behavior.
4. Reference only existing ids in connections, locks, barricades, placement lists, objectives, and finale settings.
5. Test the exact player command that should exercise the feature.
6. Run `cargo test` after changing engine behavior; use the setup diagnostics for datapack errors.
