# Map System Spec

## Purpose

This document defines the intended `v0.1` shape for the Chatty Quest map system.

The goal is to keep the map:

- deterministic in truth
- simple in rendering
- reusable across scenarios
- visually useful even before a full art pass exists

This spec also establishes the intended relationship between:

- scenario location truth
- map generation and layout
- thumbnail presentation
- fog of war
- enlarged image viewing

## Core Rule

The map should be a graph first and a grid second.

The broader engine-wide rule for derived presentation surfaces is documented in [DERIVED_VIEW_SPEC.md](c:/Users/User/Desktop/chatty-quest/docs/DERIVED_VIEW_SPEC.md:1).

That means:

- canonical truth is location ids plus allowed connections
- reducer movement trusts explicit connections
- grid placement is presentation data derived from that truth
- touching map tiles should reflect allowed adjacency, not replace it

This avoids turning UI layout into hidden game logic.

## `v0.1` Direction

For `v0.1`, the simplest useful shape is:

- one location = one square tile
- each tile uses the location image as its thumbnail when available
- missing location imagery falls back cleanly
- every placed tile must touch at least one connected tile orthogonally
- the left-side map panel stays visible in the `Game` tab

This is enough to create:

- a usable readable map
- a future fog-of-war seam
- a future click-to-enlarge seam

## Map Truth Model

Canonical map truth should stay in scenario and run state.

Important truth-bearing fields:

- location ids
- starting location
- known locations
- visited locations
- allowed connections
- lock state if later added

Presentation-only fields may include:

- `grid_x`
- `grid_y`
- tile thumbnail path
- hidden or visible display state

Those presentation fields should never become the sole source of movement truth.

## Data Contract

The map system should be split into three layers:

- canonical run truth
- generated map presentation data
- transient UI interaction state

### Canonical Run Truth

This belongs in scenario data and run state.

Recommended truth-bearing fields:

- `current_location_id`
- `known_locations`
- `visited_locations`
- location connection data
- lock state if used
- scenario boundary behavior

These fields determine where the player actually is and where the player can actually go.

The reducer owns this layer.

### Generated Map Presentation Data

This should be derived from canonical truth and datapack content.

Recommended `v0.1` shape:

```text
GeneratedMapLayout
  width: integer
  height: integer
  tiles: list of MapTileLayout
```

```text
MapTileLayout
  location_id: string
  grid_x: integer
  grid_y: integer
  thumbnail_path: optional string
  using_datapack_fallback: bool
  using_engine_fallback: bool
```

Field intent:

- `location_id`
  Links the tile back to canonical location truth.
- `grid_x` and `grid_y`
  Presentation coordinates only.
- `thumbnail_path`
  Already-resolved image path or placeholder-safe image path.
- `using_datapack_fallback`
  Marks that the tile is using pack-owned placeholder art.
- `using_engine_fallback`
  Marks that the tile is using engine-owned placeholder art.

This layer should be rebuildable from canonical state and pack content.

It should not contain hidden movement permissions or extra location truth.

### Transient UI Interaction State

This belongs in UI session state, not canonical run state.

Recommended `v0.1` examples:

- hovered tile id
- selected tile id
- currently open asset viewer request
- current fog display mode if treated as a local UI option

Suggested shape:

```text
MapPanelState
  layout: optional GeneratedMapLayout
  hovered_location_id: optional string
  selected_location_id: optional string
```

The map panel may use this state for display and interaction, but it must not become the authoritative source of where the player is.

## Generation Pipeline

Recommended pipeline:

1. Select or generate the playable location set.
2. Build or confirm a valid connection graph.
3. Ensure every location connects to at least one other location unless the scenario explicitly allows isolation.
4. Lay the graph onto a simple grid.
5. Assign each placed location a thumbnail source.
6. Render visibility state based on known and visited status plus any fog option.

This keeps logical validity separate from visual arrangement.

## Grid Layout Rule

The grid layout should be intentionally simple.

Recommended `v0.1` layout rules:

- each location occupies one square tile
- tiles connect through orthogonal touch only
- diagonal touch does not count as adjacency
- no location should be placed outside the map bounds
- avoid overlapping tiles
- prefer compact readable layouts over clever procedural shapes

The minimum structural rule is:

- every placed tile must touch at least one connected tile

Better future layout quality can be added later, but this rule alone gives a safe first shape.

## Touching And Exits

The map display should reflect actual exit truth.

That means:

- if two tiles touch orthogonally on the rendered grid, that should correspond to an allowed connection in scenario truth
- if two locations are not connected in scenario truth, the map should not present them as touching neighbors
- the UI should only surface move targets that are real connected exits from the current location

Important boundary:

- touching tiles do not create exits
- explicit connections create exits
- the grid layout must be generated to reflect those connections faithfully

In the current engine shape, movement is already reducer-gated by explicit location connections.

The map system should preserve that rule rather than inventing a separate movement interpretation.

## Thumbnail Rule

The map tile should use the location image as its thumbnail source.

Recommended fallback ladder:

1. location image
2. datapack image placeholder
3. engine placeholder

This should follow the same media fallback philosophy already used elsewhere in the engine.

If location art is missing, the map should still render a stable tile instead of appearing broken.

## Fog Of War

Fog of war should be a visibility layer over the same grid model, not a separate map system.

Current supported modes:

- `Full`
- `Known`
- `Visited`

Current `v0.1` behavior:

- `Full` shows all tiles
- `Known` shows discovered tiles and hints adjacent exits from the current location
- `Visited` shows visited tiles and hints adjacent exits from the current location
- hinted exits are navigable but not fully revealed until discovered through play
- unknown non-adjacent tiles remain hidden

The important part is that fog should depend on deterministic run knowledge, not narrator prose.

## Click To Enlarge

The map system should support a reusable enlarged image viewer.

Initial behavior:

- clicking a map tile opens a larger image view
- the larger view shows the location image or placeholder
- the larger view includes a visible close button
- the map panel should open that view by emitting a generic asset-viewer request payload

This viewer should be designed as a generic UI pattern rather than a map-only one.

Future reuse targets:

- inventory item image enlargement
- character panel skill or trait image enlargement
- enemy or boss inspection image enlargement
- media panel image enlargement

The same component should be reused wherever possible instead of inventing multiple popups.

## Shared Viewer Principles

The enlarged viewer should:

- be purely presentational
- not change state truth on open
- close cleanly
- work with fallback imagery
- tolerate missing media gracefully

If later expanded, it may support:

- captions
- zoom
- paging
- video or motion playback

But `v0.1` only needs a stable image-view box.

## Interaction Expectations

The map panel should support:

- at-a-glance current location awareness
- visible connected space
- future fog-of-war readability
- future click-to-enlarge inspection

The map panel should not become:

- the only movement input surface
- a replacement for reducer truth
- a freeform drag-and-drop builder in `v0.1`

It is a readable operational map, not a level editor.

## Future Compatibility

This direction leaves room for:

- richer procedural layouts
- larger scenario families
- map-scale tuning during setup
- explicit fog-of-war options in world generation
- tile badges for enemies, items, or objectives
- animated thumbnails
- scenario-specific tile styling

The simple square-tile system is not throwaway work if we keep the graph-vs-grid boundary clean.

## Recommended `v0.1` Summary

For `v0.1`, the map should be:

- graph-backed
- grid-rendered
- tile-based
- location-image-driven
- fallback-safe
- fog-ready
- popup-viewer-ready

That gives Chatty Quest a map system that is easy to implement now and still consistent with the future engine shape.
