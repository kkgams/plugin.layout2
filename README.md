# BSP layouts for GAMS

A Rust BSP layout component paired with its browser UI Service. Workspaces have
independent trees and focus; moving or resizing panels preserves their Views.
Floating windows are not implemented in this first slice.

## Try it

Install Nix and enable direnv, or run commands through `nix develop --command`.
The local Project uses the built component and local UI Service together, with
pinned released Host, Files View, theme and supporting UI Services.

```sh
nix develop --command make test
nix develop --command make open
```

`make open` builds the component and launches this checkout's `gams.json` in
`tools/gams.app`. It downloads the pinned macOS Apple Silicon GAMS v3.0.0 Host
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

## Development

| Command | Output / behavior |
| --- | --- |
| `make build` | `build.nosync/plugin.layout2.wasm`, with embedded notices |
| `make test` | Native Rust behavior tests and dependency-free JavaScript checks/tests |
| `make package` | `build.nosync/plugin.layout2.zip` and `SHA256SUMS` |
| `make open` | Build and open this local Project in the released Host |
| `make serve-docs DOCS_PORT=8082` | Independent documentation preview |
| `make docs` / `make validate-docs` | Build / validate with pinned justgook/wiki v1.6.0 |
| `make clean` | Remove declared generated output, downloaded Host and currently configured Unit downloads |

Builds, Cargo caches, temporary files and generated documentation stay under
ignored `build.nosync/`. Clean refuses tracked files and symlinks in deletion
paths; it preserves authored files and unrelated installed Units.

The ZIP contains the component, `ui-plugins/layout2.js`, README and licenses.
Install both entries together: the service is not compatible with the old
`plugin.layout` interface. This repository does not replace either existing Unit.
No remote, tag, release or publication is configured by local development.

See [the command/state contract](docs/index.md). Apache-2.0 for authored code;
TUIOS MIT attribution and original terms are retained in `licenses/TUIOS-MIT.txt`.
