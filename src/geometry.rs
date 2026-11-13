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
use crate::{LayoutSnapshot, PlacedPanel, SplitGuide, WorkspaceSummary};
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
/// Test one axis without changing descendant ratios or their pixel rounding.
fn fits_axis(n: &Node, span: u32, axis: bool, gap: u32, minimum: u32) -> bool {
    match n {
        Node::Leaf { .. } => span >= minimum,
        Node::Split {
            vertical,
            ratio,
            first,
            second,
            ..
        } => {
            if *vertical != axis {
                return fits_axis(first, span, axis, gap, minimum)
                    && fits_axis(second, span, axis, gap, minimum);
            }
            let r = Rect {
                x: 0,
                y: 0,
                w: span,
                h: span,
            };
            let Some((a, b, _)) = divide(r, axis, *ratio, gap) else {
                return false;
            };
            let first_span = if axis { a.w } else { a.h };
            let second_span = if axis { b.w } else { b.h };
            fits_axis(first, first_span, axis, gap, minimum)
                && fits_axis(second, second_span, axis, gap, minimum)
        }
    }
}

/// The current valid parent guarantees that `available` fits this subtree.
/// Rounded child spans are monotone, so binary search finds the exact minimum.
pub(crate) fn minimum_span(n: &Node, available: u32, axis: bool, gap: u32, minimum: u32) -> u32 {
    let (mut low, mut high) = (0, available);
    while low < high {
        let mid = low + (high - low) / 2;
        if fits_axis(n, mid, axis, gap, minimum) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    low
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
pub(crate) fn snapshot(s: &State) -> Result<LayoutSnapshot> {
    let i = s.active_index();
    let (constrained, panels, splits) = match layout(s, i) {
        Ok((p, g)) => (false, p, g),
        Err(e) if e.code == "insufficient-space" => (true, Vec::new(), Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(LayoutSnapshot {
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
        panels: panels
            .into_iter()
            .map(|p| PlacedPanel {
                id: p.id,
                x: p.rect.x,
                y: p.rect.y,
                w: p.rect.w,
                h: p.rect.h,
                focused: p.focused,
            })
            .collect(),
        splits: splits
            .into_iter()
            .map(|s| SplitGuide {
                id: s.id,
                vertical: s.vertical,
                ratio: s.ratio,
                x: s.rect.x,
                y: s.rect.y,
                w: s.rect.w,
                h: s.rect.h,
            })
            .collect(),
    })
}
