use crate::geometry::{self, Rect};
use crate::model::{Arrangement, Command, Error, Node, Result, Side, State, Workspace, MAX_PANELS};

fn arrange(s: &mut State, ids: &[String], kind: Arrangement, r: Rect) -> Result<Option<Node>> {
    if ids.is_empty() {
        return Ok(None);
    }
    if matches!(kind, Arrangement::Grid) {
        let cols = (ids.len() as f64).sqrt().ceil() as usize;
        let rows: Vec<&[String]> = ids.chunks(cols).collect();
        Ok(Some(build_rows(s, &rows, r)?))
    } else {
        Ok(Some(build_line(
            s,
            ids,
            matches!(kind, Arrangement::Horizontal),
            r,
        )?))
    }
}
fn balanced_divide(
    r: Rect,
    vertical: bool,
    a: usize,
    n: usize,
    gap: u32,
) -> Result<(f64, Rect, Rect)> {
    let extent = if vertical { r.w } else { r.h };
    let content = i64::from(extent) - i64::from(gap) * (n as i64 - 1);
    let available = i64::from(extent) - i64::from(gap);
    if content <= 0 || available <= 0 {
        return Err(Error::new(
            "insufficient-space",
            "Arrangement cannot fit gaps",
        ));
    }
    let first = content as f64 * a as f64 / n as f64 + f64::from(gap) * (a - 1) as f64;
    let ratio = first / available as f64;
    let (left, right, _) =
        geometry::divide(r, vertical, ratio, gap).expect("positive available extent");
    Ok((ratio, left, right))
}
fn build_line(s: &mut State, ids: &[String], vertical: bool, r: Rect) -> Result<Node> {
    if ids.len() == 1 {
        return Ok(Node::leaf(ids[0].clone()));
    }
    let a = ids.len() / 2;
    let (ratio, left, right) = balanced_divide(r, vertical, a, ids.len(), s.gap)?;
    let id = s.allocate()?;
    Ok(Node::Split {
        id,
        vertical,
        ratio,
        first: Box::new(build_line(s, &ids[..a], vertical, left)?),
        second: Box::new(build_line(s, &ids[a..], vertical, right)?),
    })
}
fn build_rows(s: &mut State, rows: &[&[String]], r: Rect) -> Result<Node> {
    if rows.len() == 1 {
        return build_line(s, rows[0], true, r);
    }
    let a = rows.len() / 2;
    let (ratio, top, bottom) = balanced_divide(r, false, a, rows.len(), s.gap)?;
    let id = s.allocate()?;
    Ok(Node::Split {
        id,
        vertical: false,
        ratio,
        first: Box::new(build_rows(s, &rows[..a], top)?),
        second: Box::new(build_rows(s, &rows[a..], bottom)?),
    })
}
fn focus_direction(s: &mut State, i: usize, side: Side) -> Result<()> {
    let Some(focused) = s.workspaces[i].focused.clone() else {
        return Ok(());
    };
    let (panels, _) = geometry::layout(s, i)?;
    let current = panels
        .iter()
        .find(|p| p.id == focused)
        .expect("validated focused panel")
        .rect;
    let mut best: Option<((i64, i64), String)> = None;
    for p in &panels {
        if p.id == focused {
            continue;
        }
        let (a, b) = if side.vertical() {
            (
                (current.x, current.w, current.y, current.h),
                (p.rect.x, p.rect.w, p.rect.y, p.rect.h),
            )
        } else {
            (
                (current.y, current.h, current.x, current.w),
                (p.rect.y, p.rect.h, p.rect.x, p.rect.w),
            )
        };
        let ac = i64::from(a.0) * 2 + i64::from(a.1);
        let bc = i64::from(b.0) * 2 + i64::from(b.1);
        if (side.first() && bc >= ac) || (!side.first() && bc <= ac) {
            continue;
        }
        let overlap = a.2 < b.2 + b.3 && b.2 < a.2 + a.3;
        // Tiled navigation crosses a facing edge, never jumps diagonally at
        // an outer boundary. Floating layouts can choose a separate policy.
        if !overlap {
            continue;
        }
        let perpendicular =
            (i64::from(a.2) * 2 + i64::from(a.3) - i64::from(b.2) * 2 - i64::from(b.3)).abs();
        let score = ((bc - ac).abs(), perpendicular);
        if best.as_ref().is_none_or(|(old, _)| score < *old) {
            best = Some((score, p.id.clone()));
        }
    }
    if let Some((_, id)) = best {
        s.workspaces[i].focused = Some(id);
    }
    Ok(())
}
/// Descend first: the nearest matching ancestor divider owns the selected edge.
fn resize(n: &mut Node, r: Rect, panel: &str, side: Side, delta: i32, gap: u32) -> Result<bool> {
    let Node::Split {
        vertical,
        ratio,
        first,
        second,
        ..
    } = n
    else {
        return Ok(false);
    };
    let (a, b, _) = geometry::divide(r, *vertical, *ratio, gap)
        .ok_or_else(|| Error::new("insufficient-space", "Cannot resize constrained layout"))?;
    let in_first = first.contains(panel);
    let changed = if in_first {
        resize(first, a, panel, side, delta, gap)?
    } else {
        resize(second, b, panel, side, delta, gap)?
    };
    if changed {
        return Ok(true);
    }
    if *vertical == side.vertical() && in_first != side.first() {
        let available = i64::from(if *vertical { r.w } else { r.h }) - i64::from(gap);
        let old = i64::from(if *vertical { a.w } else { a.h });
        let shift = if side.first() {
            -i64::from(delta)
        } else {
            i64::from(delta)
        };
        let new = old + shift;
        if new <= 0 || new >= available {
            return Err(Error::new(
                "insufficient-space",
                "Resize exceeds available space",
            ));
        }
        *ratio = new as f64 / available as f64;
        return Ok(true);
    }
    Ok(false)
}

fn require_panel(s: &State, i: usize, id: &str) -> Result<()> {
    if s.workspaces[i].contains(id) {
        Ok(())
    } else {
        Err(Error::new(
            "unknown-panel",
            format!("Panel {id:?} does not exist in active workspace"),
        ))
    }
}
fn workspace_index(s: &State, id: &str) -> Result<usize> {
    s.workspaces
        .iter()
        .position(|w| w.id == id)
        .ok_or_else(|| Error::new("unknown-workspace", format!("Unknown workspace {id:?}")))
}
fn insert(
    s: &mut State,
    i: usize,
    id: String,
    target: Option<&str>,
    side: Side,
    ratio: f64,
    root: bool,
) -> Result<()> {
    let leaf = Node::leaf(id.clone());
    if s.workspaces[i].tree.is_none() {
        s.workspaces[i].tree = Some(leaf);
    } else {
        let split_id = s.allocate()?;
        let tree = s.workspaces[i].tree.as_mut().expect("nonempty tree");
        if root {
            wrap(tree, leaf, split_id, side, ratio);
        } else {
            insert_at(
                tree,
                target.expect("validated insertion target"),
                leaf,
                split_id,
                side,
                ratio,
            );
        }
    }
    s.workspaces[i].focused = Some(id);
    Ok(())
}
fn wrap(node: &mut Node, new: Node, id: u64, side: Side, share: f64) {
    let old = std::mem::replace(node, Node::leaf(String::new()));
    let (first, second, ratio) = if side.first() {
        (new, old, share)
    } else {
        (old, new, 1.0 - share)
    };
    *node = Node::Split {
        id,
        vertical: side.vertical(),
        ratio,
        first: Box::new(first),
        second: Box::new(second),
    };
}
fn insert_at(node: &mut Node, target: &str, new: Node, id: u64, side: Side, ratio: f64) {
    match node {
        Node::Leaf { id: own } => {
            assert_eq!(own, target);
            wrap(node, new, id, side, ratio);
        }
        Node::Split { first, second, .. } => {
            if first.contains(target) {
                insert_at(first, target, new, id, side, ratio);
            } else {
                insert_at(second, target, new, id, side, ratio);
            }
        }
    }
}
fn remove(w: &mut Workspace, id: &str) {
    w.tree = remove_node(w.tree.take().expect("validated removal"), id);
    w.repair_focus();
}
fn remove_node(n: Node, id: &str) -> Option<Node> {
    match n {
        Node::Leaf { id: own } => {
            if own == id {
                None
            } else {
                Some(Node::leaf(own))
            }
        }
        Node::Split {
            id: split_id,
            vertical,
            ratio,
            first,
            second,
        } => {
            if first.contains(id) {
                match remove_node(*first, id) {
                    None => Some(*second),
                    Some(first) => Some(Node::Split {
                        id: split_id,
                        vertical,
                        ratio,
                        first: Box::new(first),
                        second,
                    }),
                }
            } else {
                match remove_node(*second, id) {
                    None => Some(*first),
                    Some(second) => Some(Node::Split {
                        id: split_id,
                        vertical,
                        ratio,
                        first,
                        second: Box::new(second),
                    }),
                }
            }
        }
    }
}
fn swap(n: &mut Node, a: &str, b: &str) {
    match n {
        Node::Leaf { id } => {
            if id == a {
                *id = b.into();
            } else if id == b {
                *id = a.into();
            }
        }
        Node::Split { first, second, .. } => {
            swap(first, a, b);
            swap(second, a, b);
        }
    }
}
/// Find a split in the active tree together with its viewport rectangle.
/// Called only after layout has validated every divide and leaf minimum.
fn find_split(n: &Node, r: Rect, split: u64, gap: u32) -> Option<(&Node, Rect)> {
    let Node::Split {
        id,
        vertical,
        ratio,
        first,
        second,
    } = n
    else {
        return None;
    };
    if *id == split {
        return Some((n, r));
    }
    let (a, b, _) = geometry::divide(r, *vertical, *ratio, gap).expect("validated layout");
    find_split(first, a, split, gap).or_else(|| find_split(second, b, split, gap))
}

fn set_ratio(n: Option<&mut Node>, split: u64, value: f64) -> bool {
    match n {
        Some(Node::Split {
            id,
            ratio,
            first,
            second,
            ..
        }) => {
            if *id == split {
                *ratio = value;
                true
            } else {
                set_ratio(Some(first), split, value) || set_ratio(Some(second), split, value)
            }
        }
        _ => false,
    }
}
fn equalize(n: &mut Node) {
    if let Node::Split {
        ratio,
        first,
        second,
        ..
    } = n
    {
        *ratio = 0.5;
        equalize(first);
        equalize(second);
    }
}

pub(crate) fn apply(s: &mut State, c: &Command) -> Result<()> {
    let i = s.active_index();
    match c {
        Command::Init { .. } => unreachable!("init handled by API"),
        Command::Snapshot {} => {}
        Command::Create {
            id,
            target,
            side,
            ratio,
        }
        | Command::Split {
            id,
            target,
            side,
            ratio,
        } => {
            if s.workspaces.iter().any(|w| w.contains(id)) {
                return Err(Error::new("duplicate-panel", "Panel ID already exists"));
            }
            if s.workspaces.iter().map(|w| w.panels().len()).sum::<usize>() >= MAX_PANELS {
                return Err(Error::new("limit-exceeded", "Panel limit reached"));
            }
            let target = target
                .as_ref()
                .or(s.workspaces[i].focused.as_ref())
                .cloned();
            if let Some(t) = &target {
                require_panel(s, i, t)?;
            }
            insert(s, i, id.clone(), target.as_deref(), *side, *ratio, false)?;
            geometry::layout(s, i)?;
        }
        Command::Close { id } => {
            require_panel(s, i, id)?;
            remove(&mut s.workspaces[i], id);
        }
        Command::Focus { id } => {
            require_panel(s, i, id)?;
            s.workspaces[i].focused = Some(id.clone());
        }
        Command::FocusDirection { side } => focus_direction(s, i, *side)?,
        Command::Move {
            id,
            target,
            side,
            ratio,
        } => {
            require_panel(s, i, id)?;
            require_panel(s, i, target)?;
            if id != target {
                remove(&mut s.workspaces[i], id);
                insert(s, i, id.clone(), Some(target), *side, *ratio, false)?;
            }
            geometry::layout(s, i)?;
        }
        Command::MoveRoot { id, side, ratio } => {
            require_panel(s, i, id)?;
            remove(&mut s.workspaces[i], id);
            insert(s, i, id.clone(), None, *side, *ratio, true)?;
            geometry::layout(s, i)?;
        }
        Command::Swap { id, target } => {
            require_panel(s, i, id)?;
            require_panel(s, i, target)?;
            swap(
                s.workspaces[i].tree.as_mut().expect("existing panel"),
                id,
                target,
            );
        }
        Command::Resize { id, side, delta } => {
            require_panel(s, i, id)?;
            geometry::layout(s, i)?;
            let r = geometry::bounds(s);
            let gap = s.gap;
            resize(
                s.workspaces[i].tree.as_mut().expect("existing panel"),
                r,
                id,
                *side,
                *delta,
                gap,
            )?;
            geometry::layout(s, i)?;
        }
        Command::ResizeDivider { split_id, position } => {
            geometry::layout(s, i)?;
            let (node, r) = s.workspaces[i]
                .tree
                .as_ref()
                .and_then(|tree| find_split(tree, geometry::bounds(s), *split_id, s.gap))
                .ok_or_else(|| {
                    Error::new("unknown-split", "Split does not exist in active workspace")
                })?;
            let Node::Split {
                vertical,
                first,
                second,
                ..
            } = node
            else {
                unreachable!("find_split returns a split");
            };
            let (origin, extent, minimum) = if *vertical {
                (r.x, r.w, s.min_width)
            } else {
                (r.y, r.h, s.min_height)
            };
            let available = extent - s.gap;
            let lower = geometry::minimum_span(first, available, *vertical, s.gap, minimum);
            let upper =
                available - geometry::minimum_span(second, available, *vertical, s.gap, minimum);
            let requested = i64::from(*position) - i64::from(origin);
            let span = requested.clamp(i64::from(lower), i64::from(upper));
            let ratio = span as f64 / f64::from(available);
            assert!(set_ratio(s.workspaces[i].tree.as_mut(), *split_id, ratio));
            geometry::layout(s, i)?;
        }
        Command::SetSplitRatio { split_id, ratio } => {
            if !set_ratio(s.workspaces[i].tree.as_mut(), *split_id, *ratio) {
                return Err(Error::new(
                    "unknown-split",
                    "Split does not exist in active workspace",
                ));
            }
            geometry::layout(s, i)?;
        }
        Command::Equalize {} => {
            if let Some(t) = &mut s.workspaces[i].tree {
                equalize(t);
            }
            geometry::layout(s, i)?;
        }
        Command::Arrange { kind } => {
            let ids = s.workspaces[i].panels();
            let r = geometry::bounds(s);
            s.workspaces[i].tree = arrange(s, &ids, *kind, r)?;
            geometry::layout(s, i)?;
        }
        Command::SetBounds { width, height } => {
            s.width = *width;
            s.height = *height;
        }
        Command::WorkspaceCreate { id, name } => {
            if s.workspaces.iter().any(|w| w.id == *id) {
                return Err(Error::new(
                    "duplicate-workspace",
                    "Workspace ID already exists",
                ));
            }
            if s.workspaces.len() >= 128 {
                return Err(Error::new("limit-exceeded", "Workspace limit reached"));
            }
            s.workspaces.push(Workspace {
                id: id.clone(),
                name: name.clone(),
                tree: None,
                focused: None,
            });
            s.active_workspace = id.clone();
        }
        Command::WorkspaceSwitch { id } => {
            workspace_index(s, id)?;
            s.active_workspace = id.clone();
        }
        Command::WorkspaceRemove { id } => {
            let j = workspace_index(s, id)?;
            if s.workspaces.len() == 1 {
                return Err(Error::new(
                    "last-workspace",
                    "Cannot remove the last workspace",
                ));
            }
            if s.active_workspace == *id {
                let neighbor = if j > 0 { j - 1 } else { j + 1 };
                s.active_workspace = s.workspaces[neighbor].id.clone();
            }
            s.workspaces.remove(j);
        }
        Command::WorkspaceRename { id, name } => {
            let j = workspace_index(s, id)?;
            s.workspaces[j].name = name.clone();
        }
        Command::WorkspaceMove { id, workspace } => {
            require_panel(s, i, id)?;
            let j = workspace_index(s, workspace)?;
            if i != j {
                remove(&mut s.workspaces[i], id);
                let target = s.workspaces[j].focused.clone();
                insert(s, j, id.clone(), target.as_deref(), Side::Right, 0.5, false)?;
            }
            geometry::layout(s, i)?;
            geometry::layout(s, j)?;
        }
    }
    Ok(())
}
