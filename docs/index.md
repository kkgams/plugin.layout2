---
title: BSP layouts and workspaces
status: reference
---

# BSP layouts and workspaces

`plugin.layout2` owns a Rust geometry component and its paired browser UI
Service. GAMS supplies the runtime, View registry, shared UI helpers and remaining
UI Services. Projects select Views, theme and shortcuts through `gams.json`.

## Try the local Project

Run `nix develop --command make open` on macOS Apple Silicon. The local
`gams.json` loads `build.nosync/plugin.layout2.wasm` and
`src/ui-plugins/layout2.js`. Both are canonical local artifacts, not sibling
copies. Supporting Units use explicit published pins. Use the toolbar or the
shortcuts in the README. Save Project writes layout state back to the selected
Project configuration.

## Ownership

| Object | Owner |
| --- | --- |
| BSP trees, geometry, split IDs, workspace membership and focus | Rust layout document |
| Current document and command ordering | One browser UI Service instance per UI client |
| Panel chrome, View instances, tabs and pointer input | Paired UI Service |
| Shortcut bindings and initial View selection | Project configuration |
| View content and content-specific saving | Each View |

A workspace is a named layout with its own BSP tree. It is not a GAMS Project,
Runtime Session or UI client. Switching workspaces hides existing panel nodes;
it does not destroy their Views. Closing removes a panel slot, not a Project
or application process. Closing the last panel leaves an empty workspace.

## Component API

The component exports `gams:layout2/layout@0.1.0`:

```text
execute(state: string, command: string) -> result<string, string>
```

Both arguments contain JSON. Empty state is accepted only for `init`. Success
contains `{state, snapshot}`; pass the complete returned state to the next call.
Errors contain JSON `{code,message}` and leave the caller's prior state unchanged.
The state is versioned and includes stable split IDs; do not edit tree internals.

```json
{"op":"init","width":1200,"height":800,"gap":6,"min_width":100,"min_height":80}
{"op":"create","id":"a"}
{"op":"create","id":"b","target":"a","side":"right","ratio":0.5}
{"op":"workspace-create","id":"tools","name":"Tools"}
{"op":"workspace-switch","id":"main"}
{"op":"workspace-move","id":"b","workspace":"tools"}
```

| Command | Fields / meaning |
| --- | --- |
| `create` / `split` | `id`, optional `target`, `side`, `ratio`; insert a new panel |
| `close` / `focus` | `id` |
| `focus-direction` | `side`: left, right, up or down |
| `move` | `id`, `target`, `side`, optional `ratio` |
| `move-root` | `id`, `side`, optional `ratio`; place at an outer edge |
| `swap` | `id`, `target`; exchange slots without reshaping the tree |
| `resize` | `id`, `side`, `delta`; positive pixels grow toward that side |
| `set-split-ratio` | `split_id`, `ratio`; absolute left/top fraction |
| `resize-divider` | `split_id`, `position`; absolute x/y viewport pixel coordinate, clamped to valid panel minima |
| `equalize` | Set all split ratios to one half, not necessarily equal final panel sizes |
| `arrange` | `kind`: horizontal, vertical or grid; rebuild active tree |
| `set-bounds` | `width`, `height`; viewport pixels |
| `workspace-create` | `id`, `name`; create empty and activate |
| `workspace-switch` | `id` |
| `workspace-rename` | `id`, `name` |
| `workspace-move` | Panel `id`, destination `workspace`; keep active workspace |
| `snapshot` | Validate and read without incrementing revision |

For insertion/movement, `ratio` is the **new/moved panel's share**. For divider
resizing it is the **left/top child's fraction**. Geometry uses integer pixels,
not terminal cells or terminal aspect ratios.

Drag the shared `view--handle` dividers to resize adjacent panels live. The same
Host/theme CSS used by the original layout controls their thickness, cursor and
stacking. Both divider directions support pointer capture; Escape or pointer
cancellation restores the starting ratio. Closing/rearranging panels, changing
workspaces or resizing the viewport ends an active drag and discards stale
pending moves. Focused separators also accept arrows (10 pixels, or 1 with
Shift) and Home/End for their minimum/maximum position. Divider clamping retains
nested split ratios while respecting every affected panel's minimum size.

## Constraints and persistence

Commands validate IDs, tree structure, ratios and minimum geometry before
returning a new document. Successful mutations increment revision once.
Snapshots contain only active-workspace rectangles and split guides, plus all
workspace memberships and focus. An undersized viewport is represented as
`constrained: true` with no active rectangles/guides, retaining the layout and
View instances until enough space is available. It never silently overlaps tiles.

The UI Service registers `ui.layout` commands and a `projectSave` hook. Its save
data includes the engine document and panel View selections. This is layout
persistence, not a guarantee that every View has saved its edited content.
Project Save uses the Host's existing ordered save hooks and config publication.

## Scope

BSP and named workspaces are implemented first. Floating windows, tiled/floating
mode conversion, animations, PTYs and terminal rendering are outside this slice.
The existing layout component/service and their configured consumers are unchanged.

## Source and licensing

The algorithm reference is TUIOS commit
`a086dffc310597c5bf675512384bfbdb4acc55de`. Its original MIT terms remain in
`licenses/TUIOS-MIT.txt`. The Rust component and paired UI Service are authored
under Apache-2.0. Packaging retains dependency license texts and embeds complete
notices in the WASM component.
