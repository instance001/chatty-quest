# Media Panel Spec

## Purpose

This document defines the active media-panel contract for Chatty Quest. Historical `v0.1` wording
is retained only where it records the original release boundary.

The media panel exists to:

- reinforce current reducer-confirmed focus
- add scenario atmosphere
- surface media fallbacks cleanly
- stay compatible with the shared asset viewer system

It should never become a hidden authority for game truth.

## Core Rule

The media panel is driven by truth, but it is not truth.

The broader engine-wide rule for this pattern is documented in [DERIVED_VIEW_SPEC.md](DERIVED_VIEW_SPEC.md).

That means:

- reducer-confirmed events determine focus opportunities
- runtime state determines current location and encounter context
- media resolution decorates that truth
- the media panel never invents canonical state

Short version:

- event truth in
- presentation out

## Current Role

The media panel currently:

- stay visible beside the chat area in the `Game` tab
- follow the current visual focus rule
- show whether focused media is present, missing, or placeholder-backed
- support enlargement through the shared asset viewer

It is not a full cinematic system and it does not own game truth.

## Focus Model

The media panel should resolve its focus from reducer-confirmed context.

Current practical drivers:

- recent action events
- current location
- live local threats
- item or enemy focus from inspect, equip, take, use, or combat actions

Current focus direction:

- location-forward beats prefer place after override
- item-forward beats prefer the focused item after override
- combat-forward beats prefer the focused threat after override

This should follow the same dual-context and fallback rules already documented elsewhere.

The panel also exposes focus controls for host-confirmed entities visible in the current map tile.
Choosing one changes only presentation focus; it does not issue an action, change control, or
alter encounter state. This makes multiple enemies, NPCs, bosses, and actor-capable items
inspectable without inventing a map-specific interaction system.

The Game action bar may then derive a short, focus-specific set of valid-looking commands from
the selected entity and current runtime contracts. This is deliberately a separate presentation
step from focus selection: a hostile focus can surface combat and compatible player abilities,
while a non-hostile inventory endpoint can surface applicable hand-offs. Focus never bypasses the
command parser or reducer, and impossible social, transfer, healing, or revival actions must not
be advertised merely because an entity is visible.

## Role And Pool Resolution

Media is selected by semantic role (`scene_location`, `encounter_portrait`, `combat_reaction`,
`inspection_image`, `ambient_sound`, or `short_motion`) rather than by one mandatory file per
template. Resolution is hierarchical:

```text
exact template asset
-> template-preferred tag/pool
-> subtype or family pool
-> broad category pool
-> global fallback
```

Eligible pools vary their selections and avoid immediate repeats when possible. Selection can use
the run's deterministic cursor so a save/load or replay remains coherent. Proposal-created and
player-library templates use this same resolver; absent bespoke media is a normal fallback case,
not missing mechanics. The inspection surface reports the selected asset, role, pool/source, and
fallback status.

## Data Contract

The media system should be split into three layers:

- canonical trigger truth
- resolved media presentation state
- transient UI interaction state

### Canonical Trigger Truth

This belongs in run state and reducer event output.

Important truth-bearing sources:

- `current_location_id`
- known live enemies or bosses
- inventory and equipped state where relevant
- recent reducer-confirmed events
- objective completion or run end state

This layer tells the media system what happened and what currently exists.

It does not tell the UI exactly how to render.

### Resolved Media Presentation State

This should be derived from canonical truth plus datapack media references.

Recommended `v0.1` shape:

```text
ResolvedMediaPanelState
  title: string
  subtitle: string
  narrator_brief: optional string
  image_asset: optional ResolvedPanelAsset
  motion_asset: optional ResolvedPanelAsset
  audio_asset: optional ResolvedPanelAsset
  display_role: optional string
  resolved_source_label: string
  has_missing_media: bool
  using_datapack_fallback: bool
  using_engine_fallback: bool
```

```text
ResolvedPanelAsset
  relative_path: string
  resolved_path: string
  present: bool
```

Field intent:

- `title` and `subtitle`
  The current media-facing focus summary.
- `narrator_brief`
  Optional flavour guidance already attached to the focused template.
- `image_asset`
  The resolved still-image lane after fallback logic.
- `motion_asset`
  The resolved gif or video lane if present.
- `audio_asset`
  The resolved audio lane if present.
- `display_role`
  Caller-friendly role such as `location`, `item`, `enemy`, or `boss`.
- `resolved_source_label`
  A short explanation such as `dual override`, `focused item`, `location fallback`, `datapack placeholder`, or `engine placeholder`.
- `has_missing_media`
  Indicates that some referenced assets for the current focus were missing.
- `using_datapack_fallback`
  Indicates that pack-owned placeholder art is currently in use.
- `using_engine_fallback`
  Indicates that engine-owned placeholder art is currently in use.

This layer should be rebuildable from run state, pack content, and recent events.

### Transient UI Interaction State

This belongs in UI session state, not canonical run truth.

Recommended `v0.1` examples:

- whether the asset viewer is open from the media panel
- which media slot the user clicked
- temporary hover or expansion state

Suggested shape:

```text
MediaPanelUiState
  selected_slot: optional string
  open_viewer_request: optional AssetViewerRequest
```

The media panel may use this state to drive interactions, but it must not become the source of canonical focus truth.

## Ownership Split

Recommended ownership boundary:

- reducer and run state define what actually happened
- media resolver selects the current focus and fallback assets
- UI panel renders the resolved state
- asset viewer opens from a viewer request emitted by the media panel

This keeps the media lane inspectable and replaceable.

## Fallback Visibility

The media panel should make fallback state visible instead of hiding it.

Useful `v0.1` signals:

- referenced asset present
- referenced asset missing
- datapack placeholder active
- engine placeholder active

This helps both players and modders understand what the engine is doing.

## Asset Viewer Bridge

The media panel should open the shared asset viewer by building an `AssetViewerRequest` from the resolved image state.

Recommended source kind:

- `media_focus`

Suggested request contents:

- resolved image path or placeholder source
- focus title
- focus subtitle
- optional narrator brief or description
- fallback flags

The media panel should not create its own custom popup contract.

## Future Compatibility

This structure leaves room for:

- richer motion playback
- audio cue controls
- explicit source badges
- focus history
- alternate focus modes
- scenario-specific presentation skins

The important thing for `v0.1` is not feature count.

It is keeping the boundary clean between:

- what happened
- what is being focused
- how that focus is rendered
