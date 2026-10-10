# Post-Library Manual Sweep

This is the current live desktop acceptance sweep for the narrator, player library, actor
capabilities, and media pools. It supplements the historical `v0.1` scenario sweep rather than
rewriting its release record.

## Before Starting

1. Run `cargo test --quiet`.
2. Launch with `cargo run`.
3. Start a new run using the Mock backend first. Repeat the narrator checks with a chosen local or
   BYO-key backend when available.
4. Use a disposable save slot for the sweep.

## Setup and Persistence

- Select a datapack, backend, model, play contract, chaos level, and ambient-proposal whitelist.
- Close and relaunch the app before starting a run.
- Confirm those title-screen selections restore without re-entering them.
- Confirm no API key is shown in setup, diagnostics, save data, or log output.

## Narration and Free Text

- With Strict/Tight settings, submit ordinary free text. It may receive narration but must not
  create a proposal card or change state.
- With Guided and Loose settings, submit several proposal-eligible free-text turns. Confirm
  narrator-only turns remain harmless and proposal frequency follows the visible host gate.
- With Wild/Unleashed, submit one proposal-eligible free-text turn. Confirm the host selects an
  allowed kind, surfaces one editable card, and never claims the proposal already exists.
- Submit free text that is not proposal-worthy. Confirm it receives narration without a bland
  command-parser rejection and without a proposal card.
- Trigger one malformed/weak-model proposal if practical. Confirm narration remains available on
  the next turn and the malformed proposal is discarded for that turn only.

## Proposal Admission

- Inspect the proposal card before accepting it. Confirm it shows the complete effective
  capability contract, including host defaults and disabled/inapplicable branches.
- Edit name, description, permitted mechanical values, and capability checkboxes where the host
  allows them.
- Decline one card and confirm no entity, item, exit, effect, or objective becomes real.
- Accept one card and confirm its reducer event, action trace, diagnostics row, and relevant
  runtime surface agree on origin, target/impact, and outcome.
- If an actor-capable NPC or item is admitted, verify its inspection/media row shows its actual
  capability contract and disposition.
- Admit a status proposal. Confirm its card exposes trigger, duration, numeric effect, and optional
  tag gates; approve it, then exercise the matching host action and confirm a reducer-observable
  outcome. A zero-effect or mismatched trigger/effect combination must be rejected.

## Actor and Ownership Checks

- Focus a hostile encounter in the right panel. Confirm the action bar offers inspection, a
  direct named attack, and only compatible player abilities; it must not offer help, insult,
  threaten, betray, or inventory hand-offs. Confirm the generic `Attack` fallback is absent while
  that direct attack is present.
- Use a focused spell button in the form `cast <ability> at <hostile name>`. Confirm it resolves
  as the controlled actor's cast, not as an instruction for the named target to cast. Then verify
  the explicit named-actor cast form still works only when that actor has the matching grant.
- Focus a neutral/friendly inventory-capable actor. Confirm the action bar offers only real
  hand-offs based on the two current inventories; empty, hostile, or incapable endpoints must not
  advertise a give/request action.
- Ask a named combat-capable admitted actor to attack a present hostile using natural language.
  Confirm the log/action trace names the actor, target, and actual reducer-confirmed damage.
- Use `attack <specific hostile>` while more than one encounter actor is present. Confirm the
  reducer hits the named target rather than silently falling back to scenario priority; a bad,
  absent, non-hostile, or unattackable target must reject without damage or success narration.
- Ask an actor without combat to attack. Confirm reducer validation rejects it and narration does
  not roleplay a successful hit.
- Admit or transfer in an actor-capable item, then ask that named item to attack a present hostile.
  Confirm its effective location follows its current owner and the shared actor action path—not an
  item-specific controller—emits the combat trace.
- Ask a movement-capable actor to move to an authored adjacent location. Confirm its move event is
  real and visible.
- Transfer an owned item between the controlled player, a valid inventory-capable actor, and a
  location. Confirm each move uses one `TransferItem` event and inspection shows actual owner
  inventory.
- Use both `give <item> to <actor>` and `<actor>, take my <item>`. Confirm both issue the same
  real ownership move, the item cannot appear in both inventories, and a subsequent named return
  succeeds only after the actor actually owns it. Confirm a successful gift also produces one
  reducer-confirmed social event and that failed/refused transfers do neither.
- Put a real item in a non-hostile inventory-capable actor's inventory, then use
  `steal <item> from <actor>`. Confirm the same `TransferItem` move is recorded before one
  `Theft` social event. Try an absent item and a hostile actor too; neither may move an item or
  produce a social consequence.
- Give or return several items to a tagged actor, then use negative social fixtures where
  available. Confirm the visible relationship score and derived disposition can move in both
  directions, survive save/load, and differ by personality tag without any actor-type special case.
- Ask a plain item, an empty inventory-capable actor, and a hostile encounter for an item. Confirm
  each request is host-rejected from authoritative ownership/disposition and cannot fall through
  to narration that invents a can, rope, peaches, or any other unowned object.

## Library and Media

- Promote an approved session proposal to the player library. Confirm it appears in Library with
  type, tags, enabled state, provenance, and missing-file status where relevant.
- Disable it, begin a compatible new run, and confirm it does not participate. Re-enable it and
  confirm it becomes eligible again.
- Add tagged image, motion, and audio media. Confirm an exact template asset wins over a tagged
  pool, and tagged pools vary without changing game mechanics.
- Confirm an actor-capable dynamic item and an NPC can receive presentation media through the same
  runtime actor inspection surface.

## Ability Grants and Progression

- Confirm a narrator `Ability grants` proposal only offers existing template ids, presents a real
  recipient selector, and becomes one `GrantAbility` reducer event only after approval.
- Confirm a permanent narrator-approved grant is marked `granted`, not `unlocked`; only the
  player XP-gated `learn` route may create `unlocked` provenance.
- Grant a finite ability to an item and inspect it in Inventory. Confirm its independent remaining
  uses, provenance, and expiry are visible; transfer the item and confirm the grant follows that
  item rather than being cloned.
- Earn enough XP to use `Learn` (or `learn <ability>`). Confirm the resulting `unlocked` grant is
  infinite and permanent even if another source of that same template is finite or temporary.
  When multiple templates share one reached positive threshold in the same progress pool, confirm
  only one direct XP choice can be claimed; a later authored threshold creates the next choice.
- Confirm an already permanent player-owned starter or granted ability is not offered as another
  `Learn` unlock, while a finite, expiring, or item-bound copy still does not suppress its proper
  permanent unlock.
- Confirm an authored `learnable = false` scroll/staff/encounter template can still be granted
  and cast through its concrete grant but never appears in `Learn` or succeeds through
  `learn <ability>`.
- Unlock a passive literacy template for one skill-tag family, then use `learn <carried source
  item>` on a real matching scroll. Confirm failure leaves the source intact before literacy;
  after literacy, the exact item is consumed, its item-owned grant disappears, a permanent
  `studied` grant appears without spending an XP choice, and the event/action trace reports both
  consumption and acquisition.
  A mismatched skill-tag source must remain intact.
- Attempt an NPC/item `UnlockAbility` through any developer or future integration surface. Confirm
  it is rejected while progression pools are player-owned; grant its ability explicitly instead.
- For any future trainer/reward integration, confirm the host awards XP through one
  `AwardProgress` action into an authored pool; zero/negative values, unknown pools, and
  non-player recipients must be rejected rather than mutating progression directly.
- Cast a grant as the player and as a named present actor using
  `<actor>, cast <ability> at <target>`. Confirm both produce the same reducer-backed ability
  trace, consume only the selected grant's uses, and award only the configured progress pool.
- Confirm a successful cast creates an `AbilityUsed` presentation cue and exposes ability, origin,
  target, and effect media-pool hooks. These hooks may change media focus or selection only; they
  must not change the cast's reducer-confirmed outcome.
- Try a named actor with no matching grant, an expired grant, and a missing target. Each must be
  rejected without a success narration or an XP award.

## Save and Load

- Save after an admitted proposal, at least one ownership transfer, and an actor action.
- Load the save and verify selected narrator settings, capability contract, actor placement,
  inventories, ability grants and progress pools, current media provenance, and autonomous actor
  blackboard state remain coherent.

## Pass Condition

The sweep passes only when the visible UI agrees with reducer-confirmed state throughout. Narration
and media may decorate that state, but neither may imply an unapproved proposal, a nonexistent
transfer, or an actor action the reducer did not confirm.
