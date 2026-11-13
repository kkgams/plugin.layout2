use crate::model::{Error, Node, Result, State};
use serde::Serialize;
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
#[derive(Debug, Serialize)]
pub(crate) struct Panel {
    pub id: String,
    #[serde(flatten)]
    pub rect: Rect,
    pub focused: bool,
}
#[derive(Debug, Serialize)]
pub(crate) struct Split {
    pub id: u64,
    pub vertical: bool,
    #[serde(flatten)]
    pub rect: Rect,
    pub ratio: f64,
}
#[derive(Debug, Serialize)]
struct WorkspaceSummary {
    id: String,
    name: String,
    focused: Option<String>,
    panels: Vec<String>,
}
#[derive(Debug, Serialize)]
pub(crate) struct Snapshot {
    revision: u64,
    width: u32,
    height: u32,
    gap: u32,
    constrained: bool,
    active_workspace: String,
    focused: Option<String>,
    workspaces: Vec<WorkspaceSummary>,
    panels: Vec<Panel>,
    splits: Vec<Split>,
}
pub(crate) fn bounds(s: &State) -> Rect {
    Rect {
        x: 0,
        y: 0,
        w: s.width,
        h: s.height,
    }
}
/// First child owns the rounded ratio of available pixels; gap is not pane area.
pub(crate) fn divide(r: Rect, vertical: bool, ratio: f64, gap: u32) -> Option<(Rect, Rect, Rect)> {
    let available = (if vertical { r.w } else { r.h }).checked_sub(gap)?;
    let first = (f64::from(available) * ratio).round() as u32;
    let second = available.checked_sub(first)?;
    if vertical {
        Some((
            Rect { w: first, ..r },
            Rect {
                x: r.x + first + gap,
                w: second,
                ..r
            },
            Rect {
                x: r.x + first,
                w: gap,
                ..r
            },
        ))
    } else {
        Some((
            Rect { h: first, ..r },
            Rect {
                y: r.y + first + gap,
                h: second,
                ..r
            },
            Rect {
                y: r.y + first,
                h: gap,
                ..r
            },
        ))
    }
}
pub(crate) fn layout(s: &State, index: usize) -> Result<(Vec<Panel>, Vec<Split>)> {
    let w = &s.workspaces[index];
    let mut panels = Vec::new();
    let mut splits = Vec::new();
    if let Some(t) = &w.tree {
        place(
            t,
            bounds(s),
            s,
            w.focused.as_deref(),
            &mut panels,
            &mut splits,
        )?;
    }
    Ok((panels, splits))
}
fn place(
    n: &Node,
    r: Rect,
    s: &State,
    focus: Option<&str>,
    panels: &mut Vec<Panel>,
    splits: &mut Vec<Split>,
) -> Result<()> {
    let insufficient = || {
        Error::new(
            "insufficient-space",
            "Layout cannot fit pane minimum dimensions",
        )
    };
    match n {
        Node::Leaf { id } => {
            if r.w < s.min_width || r.h < s.min_height {
                return Err(insufficient());
            }
            panels.push(Panel {
                id: id.clone(),
                rect: r,
                focused: focus == Some(id),
            });
        }
        Node::Split {
            id,
            vertical,
            ratio,
            first,
            second,
        } => {
            let (a, b, guide) = divide(r, *vertical, *ratio, s.gap).ok_or_else(insufficient)?;
            splits.push(Split {
                id: *id,
                vertical: *vertical,
                rect: guide,
                ratio: *ratio,
            });
            place(first, a, s, focus, panels, splits)?;
            place(second, b, s, focus, panels, splits)?;
        }
    }
    Ok(())
}
pub(crate) fn snapshot(s: &State) -> Result<Snapshot> {
    let i = s.active_index();
    let (constrained, panels, splits) = match layout(s, i) {
        Ok((p, g)) => (false, p, g),
        Err(e) if e.code == "insufficient-space" => (true, Vec::new(), Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(Snapshot {
        revision: s.revision,
        width: s.width,
        height: s.height,
        gap: s.gap,
        constrained,
        active_workspace: s.active_workspace.clone(),
        focused: s.workspaces[i].focused.clone(),
        workspaces: s
            .workspaces
            .iter()
            .map(|w| WorkspaceSummary {
                id: w.id.clone(),
                name: w.name.clone(),
                focused: w.focused.clone(),
                panels: w.panels(),
            })
            .collect(),
        panels,
        splits,
    })
}
