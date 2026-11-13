//! Headless, caller-owned typed BSP layout documents.
mod document;
mod engine;
mod geometry;
mod model;

mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "layout-plugin",
        additional_derives: [PartialEq],
    });
}
pub use bindings::exports::gams::layout2::layout::{
    Arrangement, ErrorCode, LayoutConfig, LayoutDocument, LayoutError, LayoutNode, LayoutResponse,
    LayoutSnapshot, PlacedPanel, Side, SplitGuide, SplitNode, Workspace, WorkspaceSummary,
};
use model::{Command, State};

impl From<model::Error> for LayoutError {
    fn from(error: model::Error) -> Self {
        let code = match error.code {
            "invalid-command" => ErrorCode::InvalidArgument,
            "invalid-state" => ErrorCode::InvalidDocument,
            "unknown-panel" => ErrorCode::UnknownPanel,
            "unknown-workspace" => ErrorCode::UnknownWorkspace,
            "unknown-split" => ErrorCode::UnknownSplit,
            "duplicate-panel" => ErrorCode::DuplicatePanel,
            "duplicate-workspace" => ErrorCode::DuplicateWorkspace,
            "insufficient-space" => ErrorCode::InsufficientSpace,
            "limit-exceeded" => ErrorCode::LimitExceeded,
            "last-workspace" => ErrorCode::LastWorkspace,
            _ => ErrorCode::InternalError,
        };
        Self {
            code,
            message: error.message,
        }
    }
}
impl From<Side> for model::Side {
    fn from(side: Side) -> Self {
        match side {
            Side::Left => Self::Left,
            Side::Right => Self::Right,
            Side::Up => Self::Up,
            Side::Down => Self::Down,
        }
    }
}
impl From<Arrangement> for model::Arrangement {
    fn from(kind: Arrangement) -> Self {
        match kind {
            Arrangement::Horizontal => Self::Horizontal,
            Arrangement::Vertical => Self::Vertical,
            Arrangement::Grid => Self::Grid,
        }
    }
}

fn finish(mut state: State) -> Result<LayoutResponse, LayoutError> {
    state.validate()?;
    state.revision = state
        .revision
        .checked_add(1)
        .filter(|revision| *revision <= model::MAX_SAFE_INTEGER)
        .ok_or_else(|| model::Error::new("limit-exceeded", "Revision exhausted"))?;
    let snapshot = geometry::snapshot(&state)?;
    Ok(LayoutResponse {
        document: document::export(state),
        snapshot,
    })
}

fn mutate(document: LayoutDocument, command: Command) -> Result<LayoutResponse, LayoutError> {
    command.validate()?;
    let mut state = document::import(document)?;
    engine::apply(&mut state, &command)?;
    finish(state)
}

/// Create an empty Main workspace with revision one.
pub fn initialize(
    width: u32,
    height: u32,
    config: LayoutConfig,
) -> Result<LayoutResponse, LayoutError> {
    Command::Init {
        width,
        height,
        gap: config.gap,
        min_width: config.min_width,
        min_height: config.min_height,
    }
    .validate()?;
    finish(State::new(
        width,
        height,
        config.gap,
        config.min_width,
        config.min_height,
    ))
}

/// Validate and compute geometry without incrementing the revision.
pub fn get_snapshot(document: LayoutDocument) -> Result<LayoutSnapshot, LayoutError> {
    Ok(geometry::snapshot(&document::import(document)?)?)
}

pub fn create_panel(
    document: LayoutDocument,
    id: String,
    target: Option<String>,
    side: Side,
    share: f64,
) -> Result<LayoutResponse, LayoutError> {
    mutate(
        document,
        Command::Create {
            id,
            target,
            side: side.into(),
            ratio: share,
        },
    )
}

pub fn close_panel(document: LayoutDocument, id: String) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::Close { id })
}

pub fn focus_panel(document: LayoutDocument, id: String) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::Focus { id })
}

pub fn focus_direction(
    document: LayoutDocument,
    side: Side,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::FocusDirection { side: side.into() })
}

pub fn move_panel(
    document: LayoutDocument,
    id: String,
    target: String,
    side: Side,
    share: f64,
) -> Result<LayoutResponse, LayoutError> {
    mutate(
        document,
        Command::Move {
            id,
            target,
            side: side.into(),
            ratio: share,
        },
    )
}

pub fn move_panel_to_root(
    document: LayoutDocument,
    id: String,
    side: Side,
    share: f64,
) -> Result<LayoutResponse, LayoutError> {
    mutate(
        document,
        Command::MoveRoot {
            id,
            side: side.into(),
            ratio: share,
        },
    )
}

pub fn swap_panels(
    document: LayoutDocument,
    first: String,
    second: String,
) -> Result<LayoutResponse, LayoutError> {
    mutate(
        document,
        Command::Swap {
            id: first,
            target: second,
        },
    )
}

pub fn resize_panel(
    document: LayoutDocument,
    id: String,
    side: Side,
    delta: i32,
) -> Result<LayoutResponse, LayoutError> {
    mutate(
        document,
        Command::Resize {
            id,
            side: side.into(),
            delta,
        },
    )
}

pub fn set_split_ratio(
    document: LayoutDocument,
    split_id: u64,
    ratio: f64,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::SetSplitRatio { split_id, ratio })
}

pub fn resize_divider(
    document: LayoutDocument,
    split_id: u64,
    position: i32,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::ResizeDivider { split_id, position })
}

pub fn equalize(document: LayoutDocument) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::Equalize {})
}

pub fn arrange(document: LayoutDocument, kind: Arrangement) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::Arrange { kind: kind.into() })
}

pub fn set_bounds(
    document: LayoutDocument,
    width: u32,
    height: u32,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::SetBounds { width, height })
}

pub fn create_workspace(
    document: LayoutDocument,
    id: String,
    name: String,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::WorkspaceCreate { id, name })
}

pub fn switch_workspace(
    document: LayoutDocument,
    id: String,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::WorkspaceSwitch { id })
}

/// Remove a workspace and all its panels, retaining at least one workspace.
pub fn remove_workspace(
    document: LayoutDocument,
    id: String,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::WorkspaceRemove { id })
}

pub fn rename_workspace(
    document: LayoutDocument,
    id: String,
    name: String,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::WorkspaceRename { id, name })
}

pub fn move_panel_to_workspace(
    document: LayoutDocument,
    id: String,
    workspace: String,
) -> Result<LayoutResponse, LayoutError> {
    mutate(document, Command::WorkspaceMove { id, workspace })
}

#[cfg(target_arch = "wasm32")]
mod component {
    use super::*;
    struct Layout;
    impl bindings::exports::gams::layout2::layout::Guest for Layout {
        fn initialize(
            width: u32,
            height: u32,
            config: LayoutConfig,
        ) -> Result<LayoutResponse, LayoutError> {
            super::initialize(width, height, config)
        }
        fn get_snapshot(document: LayoutDocument) -> Result<LayoutSnapshot, LayoutError> {
            super::get_snapshot(document)
        }
        fn create_panel(
            document: LayoutDocument,
            id: String,
            target: Option<String>,
            side: Side,
            share: f64,
        ) -> Result<LayoutResponse, LayoutError> {
            super::create_panel(document, id, target, side, share)
        }
        fn close_panel(
            document: LayoutDocument,
            id: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::close_panel(document, id)
        }
        fn focus_panel(
            document: LayoutDocument,
            id: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::focus_panel(document, id)
        }
        fn focus_direction(
            document: LayoutDocument,
            side: Side,
        ) -> Result<LayoutResponse, LayoutError> {
            super::focus_direction(document, side)
        }
        fn move_panel(
            document: LayoutDocument,
            id: String,
            target: String,
            side: Side,
            share: f64,
        ) -> Result<LayoutResponse, LayoutError> {
            super::move_panel(document, id, target, side, share)
        }
        fn move_panel_to_root(
            document: LayoutDocument,
            id: String,
            side: Side,
            share: f64,
        ) -> Result<LayoutResponse, LayoutError> {
            super::move_panel_to_root(document, id, side, share)
        }
        fn swap_panels(
            document: LayoutDocument,
            first: String,
            second: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::swap_panels(document, first, second)
        }
        fn resize_panel(
            document: LayoutDocument,
            id: String,
            side: Side,
            delta: i32,
        ) -> Result<LayoutResponse, LayoutError> {
            super::resize_panel(document, id, side, delta)
        }
        fn set_split_ratio(
            document: LayoutDocument,
            split_id: u64,
            ratio: f64,
        ) -> Result<LayoutResponse, LayoutError> {
            super::set_split_ratio(document, split_id, ratio)
        }
        fn resize_divider(
            document: LayoutDocument,
            split_id: u64,
            position: i32,
        ) -> Result<LayoutResponse, LayoutError> {
            super::resize_divider(document, split_id, position)
        }
        fn equalize(document: LayoutDocument) -> Result<LayoutResponse, LayoutError> {
            super::equalize(document)
        }
        fn arrange(
            document: LayoutDocument,
            kind: Arrangement,
        ) -> Result<LayoutResponse, LayoutError> {
            super::arrange(document, kind)
        }
        fn set_bounds(
            document: LayoutDocument,
            width: u32,
            height: u32,
        ) -> Result<LayoutResponse, LayoutError> {
            super::set_bounds(document, width, height)
        }
        fn create_workspace(
            document: LayoutDocument,
            id: String,
            name: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::create_workspace(document, id, name)
        }
        fn switch_workspace(
            document: LayoutDocument,
            id: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::switch_workspace(document, id)
        }
        fn remove_workspace(
            document: LayoutDocument,
            id: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::remove_workspace(document, id)
        }
        fn rename_workspace(
            document: LayoutDocument,
            id: String,
            name: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::rename_workspace(document, id, name)
        }
        fn move_panel_to_workspace(
            document: LayoutDocument,
            id: String,
            workspace: String,
        ) -> Result<LayoutResponse, LayoutError> {
            super::move_panel_to_workspace(document, id, workspace)
        }
    }
    bindings::export!(Layout with_types_in bindings);
}
