# Asset Viewer Spec

## Purpose

This document defines the reusable enlarged media viewer component for Chatty Quest.

The goal is to avoid inventing separate popup systems for:

- map tiles
- inventory items
- character panel skills or traits
- enemy or boss inspection
- media panel enlargement

One viewer should serve all of these surfaces.

## Core Rule

The asset viewer is a presentation component, not a state authority.

That means:

- opening it does not mutate canonical game truth
- closing it does not mutate canonical game truth
- it displays already-resolved media or fallback media
- it should work even when the caller only has placeholder-safe content

## `v0.1` Scope

For `v0.1`, the viewer only needs to handle a stable image popup.

Required behavior:

- open from multiple UI surfaces
- show a larger image or placeholder
- show a title
- optionally show supporting text
- include a visible close button
- close cleanly and predictably

Optional future behavior can come later.

## Reuse Targets

The same viewer should be callable from:

- map location tiles
- inventory items
- character panel skills
- character panel traits
- media panel focused image
- future enemy or boss cards

This keeps the app coherent and reduces duplicated UI behavior.

## Caller Contract

The caller should provide a small structured payload.

Suggested fields:

- asset source
- title
- optional subtitle
- optional descriptive text
- optional source label such as `location`, `item`, `skill`, or `enemy`

The viewer should not need to know where the request came from beyond those display hints.

Recommended `v0.1` payload shape:

```text
AssetViewerRequest
  viewer_id: string
  source_kind: string
  title: string
  subtitle: optional string
  description: optional string
  image_path: optional string
  resolved_source_label: optional string
  using_datapack_fallback: bool
  using_engine_fallback: bool
```

Field intent:

- `viewer_id`
  Stable local identifier for the current request instance.
- `source_kind`
  Caller-friendly label such as `location`, `item`, `skill`, `trait`, `enemy`, `boss`, or `media_focus`.
- `title`
  Main display heading.
- `subtitle`
  Smaller supporting line such as location name, item category, or role.
- `description`
  Optional longer text for flavour or context.
- `image_path`
  Already-resolved image path if present.
- `resolved_source_label`
  Optional explanation such as `focused item`, `location fallback`, `datapack placeholder`, or `engine placeholder`.
- `using_datapack_fallback`
  True when the viewer is opening with pack-local placeholder art.
- `using_engine_fallback`
  True when the viewer is opening with engine-owned placeholder art.

The viewer should not be responsible for resolving media paths itself in `v0.1`.

The caller resolves the media.
The viewer displays the result.

## Ownership Split

Recommended ownership boundary:

- runtime or panel logic resolves what image should be shown
- runtime or panel logic builds the viewer request payload
- UI session state tracks whether a viewer is open
- the viewer component only renders the currently active request

This keeps the viewer generic and prevents it from quietly absorbing game rules.

## UI State Suggestion

Recommended UI-side shape:

```text
AssetViewerState
  open_request: optional AssetViewerRequest
```

Useful operations:

- open viewer with request
- close viewer
- replace current request with a new one

That is enough for `v0.1`.

No viewer history or stack is required yet.

## Fallback Rule

The viewer must tolerate:

- pack-owned media
- datapack fallback media
- engine fallback media
- missing descriptive text

If the source is missing, the viewer should still open with placeholder-safe output rather than failing silently.

## UI Principles

The viewer should be:

- easy to open
- easy to dismiss
- visually consistent across callers
- reusable without special-case branching

Good `v0.1` defaults:

- centered modal or popup window
- clear title at top
- close button in the top corner
- image area as the visual priority
- short supporting text below if present

## Non-Goals For `v0.1`

The asset viewer does not need to support:

- advanced zoom controls
- drag repositioning
- galleries
- playlists
- editing
- generation
- annotation

Those can remain future extensions.

## Future Compatibility

Later versions may support:

- zoom
- paging between related assets
- motion or video playback
- audio preview triggers
- keyboard shortcuts
- richer metadata panels

But the main architectural win already happens in `v0.1` if every caller uses one common viewer pattern.
