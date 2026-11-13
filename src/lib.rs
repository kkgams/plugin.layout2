//! Headless, caller-owned BSP documents. Geometry uses square pixels and internal gaps.
mod engine;
mod geometry;
mod model;

use model::{Command, Error, State};

/// Apply one JSON command to a versioned JSON document. Errors are JSON
/// `{code,message}` strings; the caller's document is never mutated on failure.
pub fn execute(state: &str, command: &str) -> Result<String, String> {
    execute_inner(state, command)
        .map_err(|e| serde_json::to_string(&e).expect("serializable error"))
}

fn execute_inner(state: &str, command: &str) -> Result<String, Error> {
    if command.len() > 65_536 {
        return Err(Error::new("invalid-command", "Command exceeds size limit"));
    }
    let command: Command =
        serde_json::from_str(command).map_err(|e| Error::new("invalid-command", e.to_string()))?;
    command.validate()?;
    let mut state = if state.is_empty() {
        match command {
            Command::Init {
                width,
                height,
                gap,
                min_width,
                min_height,
            } => State::new(width, height, gap, min_width, min_height),
            _ => return Err(Error::new("invalid-state", "Empty state requires init")),
        }
    } else {
        if state.len() > 2_097_152 {
            return Err(Error::new("invalid-state", "State exceeds size limit"));
        }
        let mut parsed: State =
            serde_json::from_str(state).map_err(|e| Error::new("invalid-state", e.to_string()))?;
        parsed.validate()?;
        if matches!(command, Command::Init { .. }) {
            return Err(Error::new("invalid-command", "init requires empty state"));
        }
        engine::apply(&mut parsed, &command)?;
        parsed
    };
    state.validate()?;
    if !matches!(command, Command::Snapshot { .. }) {
        state.revision = state
            .revision
            .checked_add(1)
            .filter(|v| *v <= model::MAX_SAFE_INTEGER)
            .ok_or_else(|| Error::new("limit-exceeded", "Revision exhausted"))?;
    }
    let snapshot = geometry::snapshot(&state)?;
    serde_json::to_string(&serde_json::json!({"state":state,"snapshot":snapshot}))
        .map_err(|e| Error::new("internal-error", e.to_string()))
}

#[cfg(target_arch = "wasm32")]
mod component {
    wit_bindgen::generate!({ path: "wit", world: "layout-plugin" });
    struct Layout;
    impl exports::gams::layout2::layout::Guest for Layout {
        fn execute(state: String, command: String) -> Result<String, String> {
            super::execute(&state, &command)
        }
    }
    export!(Layout);
}
