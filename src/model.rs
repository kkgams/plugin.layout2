use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub(crate) const MAX_PANELS: usize = 1024;
pub(crate) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub(crate) const MAX_DEPTH: usize = 64;
const MAX_DIM: u32 = 1_000_000;
#[derive(Debug, Serialize)]
pub(crate) struct Error {
    pub code: &'static str,
    pub message: String,
}
impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
pub(crate) type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Side {
    Left,
    Right,
    Up,
    Down,
}
impl Side {
    pub fn vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
    pub fn first(self) -> bool {
        matches!(self, Self::Left | Self::Up)
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Arrangement {
    Horizontal,
    Vertical,
    Grid,
}
fn half() -> f64 {
    0.5
}
fn right() -> Side {
    Side::Right
}
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Command {
    Init {
        width: u32,
        height: u32,
        gap: u32,
        min_width: u32,
        min_height: u32,
    },
    Create {
        id: String,
        target: Option<String>,
        #[serde(default = "right")]
        side: Side,
        #[serde(default = "half")]
        ratio: f64,
    },
    Split {
        id: String,
        target: Option<String>,
        #[serde(default = "right")]
        side: Side,
        #[serde(default = "half")]
        ratio: f64,
    },
    Close {
        id: String,
    },
    Focus {
        id: String,
    },
    FocusDirection {
        side: Side,
    },
    Move {
        id: String,
        target: String,
        side: Side,
        #[serde(default = "half")]
        ratio: f64,
    },
    MoveRoot {
        id: String,
        side: Side,
        #[serde(default = "half")]
        ratio: f64,
    },
    Swap {
        id: String,
        target: String,
    },
    Resize {
        id: String,
        side: Side,
        delta: i32,
    },
    SetSplitRatio {
        split_id: u64,
        ratio: f64,
    },
    ResizeDivider {
        split_id: u64,
        position: i32,
    },
    Equalize {},
    Arrange {
        kind: Arrangement,
    },
    SetBounds {
        width: u32,
        height: u32,
    },
    WorkspaceCreate {
        id: String,
        name: String,
    },
    WorkspaceSwitch {
        id: String,
    },
    WorkspaceRename {
        id: String,
        name: String,
    },
    WorkspaceMove {
        id: String,
        workspace: String,
    },
    Snapshot {},
}
impl Command {
    pub fn validate(&self) -> Result<()> {
        let fail = |m| Error::new("invalid-command", m);
        match self {
            Self::Init {
                width,
                height,
                gap,
                min_width,
                min_height,
            } => {
                if !config_valid(*width, *height, *gap, *min_width, *min_height) {
                    return Err(fail("Invalid bounds, gap or minima"));
                }
            }
            Self::SetBounds { width, height } if *width > MAX_DIM || *height > MAX_DIM => {
                return Err(fail("Bounds exceed 1000000 pixels"))
            }
            Self::Create {
                id, target, ratio, ..
            }
            | Self::Split {
                id, target, ratio, ..
            } => {
                if !valid_id(id)
                    || target.as_ref().is_some_and(|v| !valid_id(v))
                    || !valid_ratio(*ratio)
                {
                    return Err(fail("Invalid ID, target or ratio"));
                }
            }
            Self::Move {
                id, target, ratio, ..
            } => {
                if !valid_id(id) || !valid_id(target) || !valid_ratio(*ratio) {
                    return Err(fail("Invalid ID, target or ratio"));
                }
            }
            Self::MoveRoot { id, ratio, .. } => {
                if !valid_id(id) || !valid_ratio(*ratio) {
                    return Err(fail("Invalid ID or ratio"));
                }
            }
            Self::SetSplitRatio { split_id, .. } | Self::ResizeDivider { split_id, .. }
                if *split_id == 0 || *split_id > MAX_SAFE_INTEGER =>
            {
                return Err(fail("Split ID must be a positive JavaScript-safe integer"))
            }
            Self::SetSplitRatio { ratio, .. } if !valid_ratio(*ratio) => {
                return Err(fail(
                    "Ratio must be finite and strictly between zero and one",
                ))
            }
            Self::WorkspaceCreate { id, name } | Self::WorkspaceRename { id, name } => {
                if !valid_id(id) || !valid_name(name) {
                    return Err(fail("Invalid workspace ID or name"));
                }
            }
            Self::WorkspaceMove { id, workspace } => {
                if !valid_id(id) || !valid_id(workspace) {
                    return Err(fail("Invalid panel or workspace ID"));
                }
            }
            Self::Swap { id, target } => {
                if !valid_id(id) || !valid_id(target) {
                    return Err(fail("Invalid ID"));
                }
            }
            Self::Close { id }
            | Self::Focus { id }
            | Self::Resize { id, .. }
            | Self::WorkspaceSwitch { id } => {
                if !valid_id(id) {
                    return Err(fail("Invalid ID"));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
pub(crate) fn valid_ratio(r: f64) -> bool {
    r.is_finite() && r > 0.0 && r < 1.0
}
pub(crate) fn valid_id(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
pub(crate) fn valid_name(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control)
}
pub(crate) fn config_valid(w: u32, h: u32, g: u32, mw: u32, mh: u32) -> bool {
    w <= MAX_DIM
        && h <= MAX_DIM
        && g <= MAX_DIM
        && (1..=MAX_DIM).contains(&mw)
        && (1..=MAX_DIM).contains(&mh)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Node {
    Leaf {
        id: String,
    },
    Split {
        id: u64,
        vertical: bool,
        ratio: f64,
        first: Box<Node>,
        second: Box<Node>,
    },
}
impl Node {
    pub fn leaf(id: String) -> Self {
        Self::Leaf { id }
    }
    pub fn contains(&self, id: &str) -> bool {
        match self {
            Self::Leaf { id: own } => own == id,
            Self::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }
    pub fn panels(&self, out: &mut Vec<String>) {
        match self {
            Self::Leaf { id } => out.push(id.clone()),
            Self::Split { first, second, .. } => {
                first.panels(out);
                second.panels(out);
            }
        }
    }
    pub fn first_id(&self) -> String {
        match self {
            Self::Leaf { id } => id.clone(),
            Self::Split { first, .. } => first.first_id(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "required_option")]
    pub focused: Option<String>,
    #[serde(deserialize_with = "required_option")]
    pub tree: Option<Node>,
}
fn required_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

impl Workspace {
    pub fn panels(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if let Some(t) = &self.tree {
            t.panels(&mut ids);
        }
        ids
    }
    pub fn contains(&self, id: &str) -> bool {
        self.tree.as_ref().is_some_and(|t| t.contains(id))
    }
    pub fn repair_focus(&mut self) {
        if self.focused.as_ref().is_none_or(|id| !self.contains(id)) {
            self.focused = self.tree.as_ref().map(Node::first_id);
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub version: u32,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub gap: u32,
    pub min_width: u32,
    pub min_height: u32,
    pub next_split_id: u64,
    pub active_workspace: String,
    pub workspaces: Vec<Workspace>,
}
impl State {
    pub fn new(width: u32, height: u32, gap: u32, min_width: u32, min_height: u32) -> Self {
        Self {
            version: 1,
            revision: 0,
            width,
            height,
            gap,
            min_width,
            min_height,
            next_split_id: 1,
            active_workspace: "main".into(),
            workspaces: vec![Workspace {
                id: "main".into(),
                name: "Main".into(),
                focused: None,
                tree: None,
            }],
        }
    }
    pub fn active_index(&self) -> usize {
        self.workspaces
            .iter()
            .position(|w| w.id == self.active_workspace)
            .expect("validated active workspace")
    }
    pub fn allocate(&mut self) -> Result<u64> {
        let id = self.next_split_id;
        self.next_split_id = id
            .checked_add(1)
            .filter(|v| *v <= MAX_SAFE_INTEGER)
            .ok_or_else(|| Error::new("limit-exceeded", "Split IDs exhausted"))?;
        Ok(id)
    }
    pub fn validate(&self) -> Result<()> {
        let bad = |m| Error::new("invalid-state", m);
        if self.version != 1
            || self.revision > MAX_SAFE_INTEGER
            || self.next_split_id > MAX_SAFE_INTEGER
            || !config_valid(
                self.width,
                self.height,
                self.gap,
                self.min_width,
                self.min_height,
            )
        {
            return Err(bad("Unsupported version or invalid dimensions"));
        }
        if self.workspaces.is_empty() || self.workspaces.len() > 128 {
            return Err(bad("Workspace count out of bounds"));
        }
        let mut workspaces = HashSet::new();
        let mut panels = HashSet::new();
        let mut splits = HashSet::new();
        for w in &self.workspaces {
            if !valid_id(&w.id) || !valid_name(&w.name) || !workspaces.insert(&w.id) {
                return Err(bad("Invalid or duplicate workspace"));
            }
            if let Some(t) = &w.tree {
                validate_node(t, 1, &mut panels, &mut splits)?;
            }
            match (&w.tree, &w.focused) {
                (None, None) => {}
                (Some(t), Some(id)) if t.contains(id) => {}
                _ => return Err(bad("Workspace focus does not match tree")),
            }
        }
        if !workspaces.contains(&self.active_workspace)
            || self.next_split_id == 0
            || splits.iter().any(|id| **id >= self.next_split_id)
        {
            return Err(bad("Invalid active workspace or split counter"));
        }
        Ok(())
    }
}
fn validate_node<'a>(
    node: &'a Node,
    depth: usize,
    panels: &mut HashSet<&'a String>,
    splits: &mut HashSet<&'a u64>,
) -> Result<()> {
    let bad = |m| Error::new("invalid-state", m);
    if depth > MAX_DEPTH {
        return Err(bad("Tree depth exceeds 64"));
    }
    match node {
        Node::Leaf { id } => {
            if !valid_id(id) || !panels.insert(id) || panels.len() > MAX_PANELS {
                return Err(bad("Invalid, duplicate or excessive panels"));
            }
        }
        Node::Split {
            id,
            ratio,
            first,
            second,
            ..
        } => {
            if *id == 0 || !valid_ratio(*ratio) || !splits.insert(id) || splits.len() >= MAX_PANELS
            {
                return Err(bad("Invalid, duplicate or excessive splits"));
            }
            validate_node(first, depth + 1, panels, splits)?;
            validate_node(second, depth + 1, panels, splits)?;
        }
    }
    Ok(())
}
