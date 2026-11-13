use gams_layout2::*;

fn initial() -> LayoutResponse {
    initialize(
        1000,
        700,
        LayoutConfig {
            gap: 0,
            min_width: 40,
            min_height: 30,
        },
    )
    .unwrap()
}
fn panel(document: LayoutDocument, id: &str) -> LayoutResponse {
    create_panel(document, id.into(), None, Side::Right, 0.5).unwrap()
}

#[test]
fn remove_active_workspace_closes_its_panels_and_restores_previous_focus() {
    let mut d = panel(initial().document, "main-a");
    d = panel(d.document, "main-b");
    let main = d.document.workspaces[0].clone();
    d = create_workspace(d.document, "tools".into(), "Tools".into()).unwrap();
    d = panel(d.document, "tools-a");
    d = panel(d.document, "tools-b");
    let before = d.document.clone();
    let d = remove_workspace(d.document, "tools".into()).unwrap();
    assert_eq!(d.document.revision, before.revision + 1);
    assert_eq!(d.document.next_split_id, before.next_split_id);
    assert_eq!(d.document.workspaces, [main]);
    assert_eq!(d.snapshot.active_workspace, "main");
    assert_eq!(d.snapshot.focused.as_deref(), Some("main-b"));
    assert_eq!(d.snapshot.workspaces[0].panels, ["main-a", "main-b"]);
    assert!(d.snapshot.panels.iter().all(|p| !p.id.starts_with("tools")));
    assert_eq!(get_snapshot(d.document.clone()).unwrap(), d.snapshot);
}

#[test]
fn removal_chooses_previous_neighbor_or_next_when_removing_first() {
    let mut d = create_workspace(initial().document, "middle".into(), "Middle".into()).unwrap();
    d = create_workspace(d.document, "last".into(), "Last".into()).unwrap();
    d = switch_workspace(d.document, "middle".into()).unwrap();
    let after_middle = remove_workspace(d.document.clone(), "middle".into()).unwrap();
    assert_eq!(after_middle.snapshot.active_workspace, "main");
    assert_eq!(
        after_middle
            .snapshot
            .workspaces
            .iter()
            .map(|w| w.id.as_str())
            .collect::<Vec<_>>(),
        ["main", "last"]
    );
    d = switch_workspace(d.document, "main".into()).unwrap();
    let after_first = remove_workspace(d.document, "main".into()).unwrap();
    assert_eq!(after_first.snapshot.active_workspace, "middle");
    assert_eq!(after_first.snapshot.focused, None);
    assert!(after_first.snapshot.panels.is_empty());
}

#[test]
fn remove_inactive_workspace_preserves_active_tree_focus_and_order() {
    let mut d = panel(initial().document, "a");
    d = create_workspace(d.document, "inactive".into(), "Inactive".into()).unwrap();
    d = panel(d.document, "deleted");
    d = create_workspace(d.document, "active".into(), "Active".into()).unwrap();
    d = panel(d.document, "b");
    d = panel(d.document, "c");
    let active = d.document.workspaces[2].clone();
    let d = remove_workspace(d.document, "inactive".into()).unwrap();
    assert_eq!(d.snapshot.active_workspace, "active");
    assert_eq!(d.snapshot.focused.as_deref(), Some("c"));
    assert_eq!(d.document.workspaces[1], active);
    assert_eq!(
        d.snapshot
            .workspaces
            .iter()
            .map(|w| w.id.as_str())
            .collect::<Vec<_>>(),
        ["main", "active"]
    );
}

#[test]
fn last_unknown_and_invalid_workspace_removals_reject_atomically() {
    for d in [initial(), panel(initial().document, "a")] {
        let before = d.document.clone();
        let error = remove_workspace(d.document.clone(), "main".into()).unwrap_err();
        assert_eq!(error.code, ErrorCode::LastWorkspace);
        assert_eq!(d.document, before);
        assert_eq!(get_snapshot(d.document.clone()).unwrap(), d.snapshot);
        assert_eq!(
            remove_workspace(d.document.clone(), "missing".into())
                .unwrap_err()
                .code,
            ErrorCode::UnknownWorkspace
        );
        assert_eq!(
            remove_workspace(d.document, "".into()).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }
}

#[test]
fn removal_works_for_empty_and_constrained_workspaces_without_resurrecting_ids() {
    let mut d = panel(initial().document, "keep");
    d = create_workspace(d.document, "remove".into(), "Remove".into()).unwrap();
    d = panel(d.document, "gone");
    d = set_bounds(d.document, 1, 1).unwrap();
    assert!(d.snapshot.constrained);
    d = remove_workspace(d.document, "remove".into()).unwrap();
    assert!(d.snapshot.constrained);
    assert_eq!(d.snapshot.workspaces[0].panels, ["keep"]);
    d = set_bounds(d.document, 1000, 700).unwrap();
    d = create_workspace(d.document, "remove".into(), "New empty workspace".into()).unwrap();
    assert!(d.snapshot.panels.is_empty());
    assert!(d.snapshot.workspaces[1].panels.is_empty());
    d = remove_workspace(d.document, "remove".into()).unwrap();
    assert_eq!(d.snapshot.workspaces.len(), 1);
    assert_eq!(d.snapshot.panels[0].id, "keep");
}

#[test]
fn removal_preserves_document_on_revision_limit_failure() {
    let mut d = create_workspace(initial().document, "other".into(), "Other".into()).unwrap();
    d.document.revision = 9_007_199_254_740_991;
    let before = d.document.clone();
    assert_eq!(
        remove_workspace(d.document.clone(), "other".into())
            .unwrap_err()
            .code,
        ErrorCode::LimitExceeded
    );
    assert_eq!(d.document, before);
    assert_eq!(get_snapshot(d.document).unwrap().workspaces.len(), 2);
}
