# BSP layouts for GAMS

A Rust BSP layout component paired with its browser UI Service for
[GAMS](https://github.com/kkgams/gams). Workspaces have
independent trees and focus; moving or resizing panels preserves their Views.
Floating windows are not implemented in this first slice.

The component exposes individual typed functions through `gams:layout2/layout@0.2.0`,
not a JSON-string dispatcher. The paired service and component must be updated
together; existing saved version-1 layouts convert on load.

## Included Units

| Item | Source / ZIP entry | Screenshot | License / original work |
| --- | --- | --- | --- |
| BSP component | `src/*.rs`, `wit/layout.wit` → `plugin.layout2.wasm` | Not applicable | [Apache-2.0](LICENSE); algorithm reference: [TUIOS / Gaurav Gosain](https://github.com/Gaurav-Gosain/tuios), [MIT](licenses/TUIOS-MIT.txt) |
| Paired layout UI Service | `src/ui-plugins/layout2.js` → `ui-plugins/layout2.js` | Not yet captured | [Apache-2.0](LICENSE), kkgams contributors |

## Try it

Install Nix and enable direnv, or run commands through `nix develop --command`.
The local Project uses the built component and local UI Service together, with
pinned released Host, Files View, theme and supporting UI Services.

```sh
nix develop --command make test
nix develop --command make open
```

`make open` builds the component and launches this checkout's `gams.json` in
`tools/gams.app`. It downloads the pinned macOS Apple Silicon GAMS v3.2.0 Host
when absent, verifies its published checksum and keeps the accompanying notices.
Existing downloads are never updated implicitly; macOS security checks remain
in effect. Other systems can load this Project with their installed GAMS Host.
No neighboring checkout is required.

## Controls

Use the toolbar and workspace tabs, or these Project-configured shortcuts.
`mod` means Command on macOS and Control elsewhere.

| Shortcut | Action |
| --- | --- |
| `mod+alt+Enter` / `mod+alt+shift+Enter` | New panel to the right / below |
| `mod+alt+W` | Close focused panel |
| `mod+alt+Arrow` | Focus in that direction |
| `mod+alt+shift+Arrow` | Move panel in that direction |
| `mod+alt+shift+H/J/K/L` | Grow left/down/up/right by 24 pixels |
| `mod+alt+E` | Equalize split ratios |
| `mod+alt+G` | Arrange as a grid |
| `mod+alt+T` | Create and activate a workspace |
| `mod+alt+[` / `mod+alt+]` | Previous / next workspace |
| `mod+shift+S` | Save Project, including layout and View selection |

**Close workspace** in the toolbar closes the active workspace and all its
panels. It is disabled when only one workspace remains; tabs have no close buttons.

Drag the shared resize handles between panels for live resizing. Escape cancels
a drag. Focused handles support arrows (10 pixels, Shift for 1) and Home/End;
all resizing respects nested panel minimum sizes without recreating Views.

## Development

| Command | Output / behavior |
| --- | --- |
| `make build` | `build.nosync/plugin.layout2.wasm`, with embedded notices |
| `make test` | Native Rust behavior tests and dependency-free JavaScript checks/tests |
| `make package` | `build.nosync/plugin.layout2.zip` and `SHA256SUMS` |
| `make open` | Build and open this local Project in the released Host |
| `make serve-docs DOCS_PORT=8082` | Independent documentation preview |
| `make docs` / `make validate-docs` | Build / validate with pinned justgook/wiki v1.8.0 |
| `make clean` | Remove all of `build.nosync/`, `tools/` and `gams_modules/`, including unmarked contents; refuse tracked files and symlinks before deleting anything. |

Builds, Cargo caches, temporary files and generated documentation stay under
ignored `build.nosync/`. Clean checks all deletion paths for tracked files and
symlinks before removing anything. No ownership or resource markers are required;
other paths are untouched.

The ZIP contains the component, `ui-plugins/layout2.js`, README and licenses.
Install both entries together: the service is not compatible with the old
`plugin.layout` interface. This repository does not replace either existing Unit.
Local commands do not tag or publish. Release assets use stable names:
`plugin.layout2.zip` and `SHA256SUMS`.

See [the typed API and document contract](docs/index.md). Apache-2.0 for authored code;
TUIOS MIT attribution and original terms are retained in `licenses/TUIOS-MIT.txt`.
