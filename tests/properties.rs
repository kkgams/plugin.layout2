use gams_layout2::*;
use std::collections::HashSet;

fn init() -> LayoutResponse {
    initialize(
        1000,
        800,
        LayoutConfig {
            gap: 6,
            min_width: 10,
            min_height: 10,
        },
    )
    .unwrap()
}
fn balanced(nodes: &mut Vec<LayoutNode>, start: usize, n: usize, counter: &mut u64) -> u32 {
    if n == 1 {
        let index = nodes.len() as u32;
        nodes.push(LayoutNode::Panel(format!("p{start}")));
        return index;
    }
    let id = *counter;
    *counter += 1;
    let a = n / 2;
    let first = balanced(nodes, start, a, counter);
    let second = balanced(nodes, start + a, n - a, counter);
    let index = nodes.len() as u32;
    nodes.push(LayoutNode::Split(SplitNode {
        id,
        vertical: true,
        ratio: 0.5,
        first,
        second,
    }));
    index
}
fn import_tree(nodes: Vec<LayoutNode>, root: u32, next: u64) -> LayoutDocument {
    let mut d = init().document;
    d.workspaces[0].nodes = nodes;
    d.workspaces[0].root = Some(root);
    d.workspaces[0].focused = Some("p0".into());
    d.next_split_id = next;
    d
}

#[test]
fn depth_and_count_are_bounded_but_maximum_documents_import() {
    let mut nodes = vec![LayoutNode::Panel("p0".into())];
    let mut root = 0;
    for i in 1..64 {
        let second = nodes.len() as u32;
        nodes.push(LayoutNode::Panel(format!("p{i}")));
        nodes.push(LayoutNode::Split(SplitNode {
            id: i,
            vertical: true,
            ratio: 0.5,
            first: root,
            second,
        }));
        root = nodes.len() as u32 - 1;
    }
    let snapshot = get_snapshot(import_tree(nodes.clone(), root, 100)).unwrap();
    assert!(snapshot.constrained);
    assert_eq!(snapshot.workspaces[0].panels.len(), 64);
    let second = nodes.len() as u32;
    nodes.push(LayoutNode::Panel("last".into()));
    nodes.push(LayoutNode::Split(SplitNode {
        id: 64,
        vertical: false,
        ratio: 0.5,
        first: root,
        second,
    }));
    root = nodes.len() as u32 - 1;
    assert_eq!(
        get_snapshot(import_tree(nodes, root, 100))
            .unwrap_err()
            .code,
        ErrorCode::InvalidDocument
    );
    for count in [1024, 1025] {
        let mut nodes = Vec::new();
        let mut counter = 1;
        let root = balanced(&mut nodes, 0, count, &mut counter);
        let result = get_snapshot(import_tree(nodes, root, counter));
        if count == 1024 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::InvalidDocument);
        }
    }
}

#[test]
fn workspace_move_failure_is_atomic_and_inactive_constraints_are_retained() {
    let mut d = init();
    for id in ["a", "b"] {
        d = create_panel(d.document, id.into(), None, Side::Right, 0.5).unwrap();
    }
    d = create_workspace(d.document, "other".into(), "Other".into()).unwrap();
    d = create_panel(d.document, "c".into(), None, Side::Right, 0.5).unwrap();
    d = set_bounds(d.document, 10, 10).unwrap();
    let before = d.document.clone();
    assert_eq!(
        move_panel_to_workspace(d.document.clone(), "c".into(), "main".into())
            .unwrap_err()
            .code,
        ErrorCode::InsufficientSpace
    );
    assert_eq!(d.document, before);
    assert_eq!(get_snapshot(d.document.clone()).unwrap(), d.snapshot);
    d = set_bounds(d.document, 0, 0).unwrap();
    assert!(d.snapshot.constrained);
    d = switch_workspace(d.document, "main".into()).unwrap();
    assert_eq!(d.snapshot.workspaces[0].panels, ["a", "b"]);
    assert!(d.snapshot.panels.is_empty());
}

fn replay() -> LayoutResponse {
    let mut d = init();
    let mut seed = 123456789_u64;
    let mut membership = HashSet::new();
    for turn in 0..300 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let ids = &d.snapshot.workspaces[0].panels;
        let id = ids
            .get(seed as usize % ids.len().max(1))
            .cloned()
            .unwrap_or_default();
        let side = [Side::Left, Side::Right, Side::Up, Side::Down][(seed >> 8) as usize % 4];
        let before = d.document.clone();
        let action = seed % 9;
        let result = match action {
            0 | 1 => create_panel(d.document.clone(), format!("p{turn}"), None, side, 0.5),
            2 if !ids.is_empty() => close_panel(d.document.clone(), id.clone()),
            3 if !ids.is_empty() => move_panel_to_root(d.document.clone(), id.clone(), side, 0.3),
            4 if !ids.is_empty() => resize_panel(d.document.clone(), id.clone(), side, 13),
            5 => arrange(
                d.document.clone(),
                [
                    Arrangement::Horizontal,
                    Arrangement::Vertical,
                    Arrangement::Grid,
                ][(seed >> 16) as usize % 3],
            ),
            6 => set_bounds(
                d.document.clone(),
                50 + ((seed >> 16) % 1000) as u32,
                50 + ((seed >> 32) % 800) as u32,
            ),
            7 => equalize(d.document.clone()),
            _ => focus_direction(d.document.clone(), side),
        };
        match result {
            Ok(next) => {
                if action <= 1 {
                    assert!(membership.insert(format!("p{turn}")));
                }
                if action == 2 && !ids.is_empty() {
                    assert!(membership.remove(&id));
                }
                assert_eq!(next.document.revision, before.revision + 1);
                d = next;
            }
            Err(e) => {
                assert_eq!(e.code, ErrorCode::InsufficientSpace, "{}", e.message);
                assert_eq!(d.document, before);
            }
        }
        assert_eq!(get_snapshot(d.document.clone()).unwrap(), d.snapshot);
        assert_eq!(
            d.snapshot.workspaces[0]
                .panels
                .iter()
                .cloned()
                .collect::<HashSet<_>>(),
            membership
        );
        assert_eq!(d.snapshot.workspaces[0].panels.len(), membership.len());
        if let Some(focused) = &d.snapshot.focused {
            assert!(membership.contains(focused));
        }
        for (a, p) in d.snapshot.panels.iter().enumerate() {
            assert!(p.w >= 10 && p.h >= 10);
            assert!(p.x + p.w <= d.snapshot.width && p.y + p.h <= d.snapshot.height);
            for q in &d.snapshot.panels[a + 1..] {
                assert!(
                    p.x + p.w <= q.x || q.x + q.w <= p.x || p.y + p.h <= q.y || q.y + q.h <= p.y
                );
            }
        }
    }
    d
}
#[test]
fn deterministic_replay_preserves_membership_and_nonoverlap() {
    let first = replay();
    let second = replay();
    assert_eq!(first.document, second.document);
    assert_eq!(first.snapshot, second.snapshot);
}
