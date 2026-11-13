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

The separate **Close workspace** toolbar action closes the active workspace and
all its panel Views; tabs remain navigation-only. The action is disabled for the
last workspace, and the component also rejects that operation with
`last-workspace`. Removing the active workspace selects the previous neighbor,
or the next when removing the first, preserving that neighbor's remembered
layout and focus. Removing an inactive workspace does not switch the active one.

## Component API

The component exports **`gams:layout2/layout@0.2.0`**. Every operation has its own
function and typed WIT arguments, results and errors. Full signatures and field
documentation are in `wit/layout.wit`; there is no `execute` export or JSON-string
command protocol.

Mutations accept a caller-owned `layout-document` and return
`result<layout-response, layout-error>`. The response contains `document` and
`snapshot`; pass the returned document to the next function. `initialize` creates
the initial document, while `get-snapshot` validates and reads without changing
revision. IDs, enums, numeric arguments and optional targets are WIT values.

```js
const initialized = await runtime.invoke("layout2/layout::initialize", 1200, 800, {
    gap: 0, "min-width": 100, "min-height": 80,
})
// Successful calls return { ok: { document, snapshot } }; failures return
// { err: { code, message } }. Check the result before using its document.
const first = await runtime.invoke("layout2/layout::create-panel",
    initialized.ok.document, "files", null, "right", 0.5)
const split = await runtime.invoke("layout2/layout::create-panel",
    first.ok.document, "inspector", "files", "right", 0.5)
```

Arguments below follow the required `document`, except for `initialize`.

| Function | Arguments / behavior |
| --- | --- |
| `initialize` | `width: u32`, `height: u32`, `config: layout-config`; empty Main workspace |
| `get-snapshot` | Document only; return `layout-snapshot`, not a mutation response |
| `create-panel` | `id: string`, `target: option<string>`, `side`, `share: f64` |
| `close-panel` | `id: string`; collapse the parent and repair focus |
| `focus-panel` | `id: string` |
| `focus-direction` | `side`: left, right, up or down; no diagonal jump or wrapping |
| `move-panel` | `id: string`, `target: string`, `side`, `share: f64` |
| `move-panel-to-root` | `id: string`, `side`, `share: f64`; outer edge |
| `swap-panels` | `first: string`, `second: string`; exchange slots |
| `resize-panel` | `id: string`, `side`, `delta: s32`; positive pixels grow that edge |
| `set-split-ratio` | `split-id: u64`, `ratio: f64`; strict absolute left/top fraction |
| `resize-divider` | `split-id: u64`, `position: s32`; clamped absolute viewport x/y pixels |
| `equalize` | Document only; set every split ratio to one half |
| `arrange` | `kind`: horizontal, vertical or grid |
| `set-bounds` | `width: u32`, `height: u32` |
| `create-workspace` | `id: string`, `name: string`; create empty and activate |
| `switch-workspace` | `id: string` |
| `rename-workspace` | `id: string`, `name: string` |
| `remove-workspace` | `id: string`; close all its panels, retaining at least one workspace |
| `move-panel-to-workspace` | `id: string`, `workspace: string`; keep active workspace |

`share` is the new/moved panel's fraction, strictly between zero and one. Divider
`ratio` is the left/top child's fraction. Equalizing split ratios does not
necessarily produce equal final panel sizes; use an arrangement for that.
Geometry uses integer pixels, not terminal cells or terminal aspect ratios.

### Documents and errors

Version-2 documents hold workspace-local flat `nodes` lists. Each node is a
`layout-node` variant: `panel(string)` or `split(split-node)`. Workspace `root`
and split `first`/`second` are node-list indices; split `id` is a separate stable
identity. This expresses BSP trees without recursive WIT types. JS callers use
Host variant values such as `{ case: "panel", value: "files" }` and
`{ case: "split", value: { id, vertical, ratio, first, second } }`. WIT record
field names remain hyphenated in JS, for example `document["active-workspace"]`.

Typed `layout-error` contains an `error-code` enum and message. Codes distinguish
invalid arguments/documents, unknown or duplicate identities, insufficient space,
limits and internal failures. Malformed WIT argument types are rejected by the
Host before layout execution. Imported documents reject cycles, shared children,
invalid indices, orphan nodes, duplicate IDs, bad focus and excessive depth/count.

Version 0.2 is a **breaking component interface change**: update this component
and its paired UI Service together. Existing `plugin.layout` / `ui-service.layout`
are unaffected. The paired service explicitly converts version-1 saved Project
layout trees to version-2 flat documents on load, preserving split IDs, panel
membership, ratios and workspace focus. Project Save persists the new document;
no legacy string API is retained. Unknown saved versions fail explicitly.

Drag the shared `view--handle` dividers to resize adjacent panels live. The same
Host/theme CSS used by the original layout controls their thickness, cursor and
stacking. Both divider directions support pointer capture; Escape or pointer
cancellation restores the starting ratio. Closing/rearranging panels, changing
workspaces or resizing the viewport ends an active drag and discards stale
pending moves. Focused separators also accept arrows (10 pixels, or 1 with
Shift) and Home/End for their minimum/maximum position. Divider clamping retains
nested split ratios while respecting every affected panel's minimum size.

## Constraints and persistence

Operations validate IDs, tree structure, ratios and minimum geometry before
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
