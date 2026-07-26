# Chatty Quest Zero-Spoiler User Manual

Last updated: `2026-07-16`

## What This Manual Is

This guide is for someone who:

- has never played `Chatty Quest`
- does not know the current scenario
- wants to understand the game without seeing route answers or solution details

This manual avoids:

- item-location spoilers
- route-order spoilers
- objective-answer spoilers
- fight-phase spoilers

If you want the explicit answers instead, use:

- [docs/FULL_SPOILERS_USER_MANUAL.md](FULL_SPOILERS_USER_MANUAL.md)

## What Chatty Quest Is

`Chatty Quest` is a desktop adventure game where you:

- read the current situation
- type simple commands
- watch the world update through the log, map, inventory, and status views

The current playable scenario is:

- `Property Siege Classic`

At a high level, it is a short survival-horror style scenario about:

- exploring a dangerous space
- finding useful tools
- managing pressure
- making route decisions
- surviving the end of the run

## How To Start

1. Launch the app.
2. On the setup screen, make sure `Property Siege Classic` is selected.
3. Click `Generate Game`.

If you already have a save you want to continue:

- click `Load Game`

## What You Are Looking At

Once the run begins, the app is split into a few important areas.

### Left Panel

This is your route-and-status panel.

It shows things like:

- current location
- route information
- current noise state
- nearby exits
- known locations
- a suggested next command

If you do not know what to do next, start here.

### Center Panel

This is the main log.

It shows:

- what you typed
- what the game resolved
- descriptive feedback
- damage and objective updates

### Right Panel

This is the media/context panel.

It is presentation, not a second hidden rules system.

Use it as atmosphere, not as your only source of truth.

### Bottom Action Bar

This contains:

- quick action buttons
- quick exits
- the command input field

If you prefer clicking over typing, this helps a lot.

### Inventory Tab

Use this when you want to check:

- what you are carrying
- what is equipped
- what can be used

### Character Tab

Use this for a compact mechanical snapshot.

It shows:

- HP
- noise
- objective state
- current location
- recent summary information

### Diagnostics Tab

This is the most technical screen.

You do not need it for ordinary play, but it is useful if you want exact truth about:

- lock state
- barricade state
- noise state
- recent events

## The Basic Command Set

The current build uses a small command language.

You can type:

- `help`
- `look`
- `go <location>`
- `unlock <location>`
- `barricade <location>`
- `inspect <thing>`
- `take <item>`
- `equip <item>`
- `use <item>`
- `attack`
- `wait`

Examples:

- `look`
- `go kitchen`
- `inspect torch`
- `take medkit`
- `attack`

The game also understands a few aliases:

- `move <location>`
- `walk <location>`
- `open <location>`
- `fortify <location>`
- `secure <location>`

## What The Important Commands Mean

### `look`

Use `look` often.

It refreshes your read of the current room and is one of the best anti-confusion tools in the game.

### `go <location>`

This tries to move you to a connected location.

If it fails, the usual reasons are:

- the route is not connected
- the route is blocked
- the route is locked

### `inspect <thing>`

Use this when you want more detail on:

- rooms
- items
- enemies
- the main threat in a fight

Inspection is useful because it often gives more than flavor. It can also clarify live threat state and current danger.

### `take <item>`

Picks up an item in the current room.

If something looks obviously useful, taking it is usually sensible.

### `equip <item>`

Equips a carried item.

If combat starts feeling rough, check whether you forgot to equip something better.

### `use <item>`

Uses a carried item directly.

Current items can do things like:

- heal
- reveal more route information
- interact with gated progression

### `unlock <location>`

Use this when you want to open a specific route or gate.

It is the clearest command when you know exactly what you are trying to open.

### `barricade <location>`

Use this to secure a route from inside the room, if that room supports it and you have what you need.

### `attack`

Attacks a live threat in your current room.

You do not manually choose a target in the current build.

### `wait`

Passes time in the current room.

This can be useful, but it can also be dangerous. Do not wait blindly in exposed spaces.

## How To Read The Game Without Spoilers

You do not need to know the map layout in advance.

The safest way to learn the scenario is:

1. use `look`
2. read the left panel
3. inspect unfamiliar things
4. pay attention to route warnings
5. only then commit to movement or combat

The game gives you a few helper surfaces that are worth trusting.

### Suggested Next Command

This is a hint, not an order.

It often points at the most immediately useful next move.

If you feel stuck, read it.

### Route Role

Some locations are described in terms of their tactical role.

This helps you understand whether a space feels like:

- a safer room
- a risky route
- a contested lane
- a likely objective space

### Threat Forecast

This warns you about likely punishment before it happens.

It is especially useful when deciding whether to:

- push forward
- wait
- secure a space
- take a side route

## The Four Core Ideas You Need

### 1. Some Routes Are Locked

You will not be able to go everywhere immediately.

Part of the scenario is opening the map through ordinary play.

### 2. Some Spaces Can Be Secured

Certain dangerous routes can be made safer.

This is not cosmetic. Securing space can change how the run feels and what kinds of risks are tolerable.

Some preparation can also matter later, so do not assume a secured route only affects the room where you built it.

### 3. Noise Matters

The game tracks how loud and unstable the situation has become.

Noise can make exposed play more punishing.

If the game says things are getting louder, take that seriously.

### 4. The Objective Has Multiple Parts

The run is not always solved by one single action.

To finish cleanly, you may need a combination of:

- being in the right place
- holding the right thing
- defeating the right threat

## Good Habits For New Players

1. Use `look` often.
2. Read the left panel before taking risky actions.
3. Pick up useful items unless you have a reason not to.
4. Do not treat all routes as equally safe.
5. Do not ignore noise.
6. Inspect threats if you want a better read before committing.
7. Use the `Character` tab when the log feels too busy.

## Beginner Checklist

If you want a low-spoiler way to orient yourself, do this:

1. Start the run.
2. Read the opening text.
3. Use `look`.
4. Use `inspect` on anything unclear.
5. Follow obvious opportunities to gather tools.
6. Open up more of the scenario carefully.
7. Secure dangerous spaces if the situation feels unstable.
8. Commit to the endgame only when your health and route state look reasonable.

## Combat Basics

Combat is deterministic.

That means:

- damage is real game state
- enemy and boss health is tracked
- your HP is tracked
- equipment matters
- location state can matter

If you are unsure how bad a fight currently is:

- inspect the threat

## Healing Basics

Healing resources are valuable.

A simple rule:

- do not waste recovery when you are already stable
- do not hoard recovery until it becomes useless

Good times to think about healing:

- after a rough trade
- before committing to a dangerous push
- when the route state is getting unstable

## Common New-Player Mistakes

### Waiting In Bad Places

Waiting is not harmless. If the route forecast looks ugly, believe it.

### Ignoring The Suggested Command

It is not always right, but it is often useful.

### Treating Every Path As Equivalent

They are not. Some are safer, some are faster, and some pay off differently.

### Forgetting To Inspect Threats

Inspection gives mechanical context, not just prose.

### Thinking The Right Panel Is The Only Truth

The structured UI and the log are the real authoritative read.

## Saving And Loading

The game supports save/load.

Use it if you want to:

- stop and resume
- test different approaches
- recover a run later

The current build preserves important state such as:

- location
- inventory
- health
- route state
- escalation state
- objective state

## If You Feel Lost

Try this order:

1. `look`
2. read the suggested next command
3. inspect anything unclear
4. check the `Character` tab
5. check the route forecast

That is usually enough to get unstuck without outside help.

## Quick Reference

### Best Commands For First-Time Players

- `look`
- `inspect <thing>`
- `take <item>`
- `go <location>`
- `unlock <location>`
- `attack`

### Best UI Surfaces For Clarity

- left route panel
- `Character` tab
- `Inventory` tab
- `Diagnostics` tab if you want exact truth

## Want The Full Answers?

If you want the explicit route, item, and scenario details, use:

- [docs/FULL_SPOILERS_USER_MANUAL.md](FULL_SPOILERS_USER_MANUAL.md)

## Related Documents

- [README.md](../README.md)
- [docs/V0_1_RELEASE_NOTES.md](V0_1_RELEASE_NOTES.md)
- [docs/V0_1_MANUAL_SWEEP.md](V0_1_MANUAL_SWEEP.md)
- [docs/V0_1_ACCEPTANCE_AUDIT.md](V0_1_ACCEPTANCE_AUDIT.md)
