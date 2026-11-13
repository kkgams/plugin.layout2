use gams_layout2::*;

const MAX_SAFE: u64 = 9_007_199_254_740_991;
fn initial() -> LayoutDocument {
    initialize(
        1000,
        1000,
        LayoutConfig {
            gap: 6,
            min_width: 10,
            min_height: 10,
        },
    )
    .unwrap()
    .document
}
fn populated() -> LayoutDocument {
    let d = create_panel(initial(), "a".into(), None, Side::Right, 0.5).unwrap();
    create_panel(d.document, "b".into(), None, Side::Right, 0.5)
        .unwrap()
        .document
}
fn invalid(d: LayoutDocument) {
    let before = d.clone();
    let error = get_snapshot(d.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidDocument, "{}", error.message);
    assert_eq!(d, before);
}
fn rejected(d: &LayoutDocument, result: Result<LayoutResponse, LayoutError>, code: ErrorCode) {
    let before = d.clone();
    let error = result.unwrap_err();
    assert_eq!(error.code, code, "{}", error.message);
    assert!(!error.message.is_empty());
    assert_eq!(*d, before);
}
fn split_mut(d: &mut LayoutDocument) -> &mut SplitNode {
    d.workspaces[0]
        .nodes
        .iter_mut()
        .find_map(|node| match node {
            LayoutNode::Split(s) => Some(s),
            _ => None,
        })
        .unwrap()
}

#[test]
fn flat_graph_rejects_cycles_shared_children_out_of_range_orphans_and_invalid_root() {
    let base = populated();
    let mut d = base.clone();
    let root = d.workspaces[0].root.unwrap();
    split_mut(&mut d).first = root;
    invalid(d);
    let mut d = base.clone();
    let split = split_mut(&mut d);
    split.second = split.first;
    invalid(d);
    let mut d = base.clone();
    split_mut(&mut d).second = u32::MAX;
    invalid(d);
    let mut d = base.clone();
    d.workspaces[0]
        .nodes
        .push(LayoutNode::Panel("orphan".into()));
    invalid(d);
    let mut d = base.clone();
    d.workspaces[0].root = Some(u32::MAX);
    invalid(d);
    let mut d = base;
    d.workspaces[0].root = None;
    invalid(d);
    let mut d = initial();
    d.workspaces[0].root = Some(0);
    invalid(d);
}

#[test]
fn duplicate_ids_bad_focus_and_counter_rejected() {
    let base = populated();
    let mut d = base.clone();
    for node in &mut d.workspaces[0].nodes {
        if let LayoutNode::Panel(id) = node {
            *id = "a".into();
        }
    }
    invalid(d);
    let mut d = base.clone();
    d.workspaces[0].focused = Some("missing".into());
    invalid(d);
    let mut d = base.clone();
    d.next_split_id = 1;
    invalid(d);
    let mut d = base.clone();
    let mut workspace = d.workspaces[0].clone();
    workspace.id = "other".into();
    d.workspaces.push(workspace);
    invalid(d);
    let mut d = base.clone();
    d.workspaces.push(Workspace {
        id: "main".into(),
        name: "Duplicate".into(),
        focused: None,
        root: None,
        nodes: Vec::new(),
    });
    invalid(d);
    let mut d = base.clone();
    d.active_workspace = "missing".into();
    invalid(d);
    let mut d = base.clone();
    d.version = 900;
    invalid(d);
    let mut d = base.clone();
    d.revision = MAX_SAFE + 1;
    invalid(d);
    let mut d = base.clone();
    d.next_split_id = MAX_SAFE + 1;
    invalid(d);
    let mut d = base.clone();
    d.revision = MAX_SAFE;
    assert!(get_snapshot(d.clone()).is_ok());
    rejected(
        &d,
        focus_panel(d.clone(), "a".into()),
        ErrorCode::LimitExceeded,
    );
    let mut d = base;
    d.next_split_id = MAX_SAFE;
    assert!(get_snapshot(d.clone()).is_ok());
    rejected(
        &d,
        create_panel(d.clone(), "c".into(), None, Side::Right, 0.5),
        ErrorCode::LimitExceeded,
    );
}

#[test]
fn invalid_identifiers_minima_fractions_and_extreme_resize_are_safe() {
    for config in [
        LayoutConfig {
            gap: 6,
            min_width: 0,
            min_height: 10,
        },
        LayoutConfig {
            gap: 6,
            min_width: 10,
            min_height: 0,
        },
        LayoutConfig {
            gap: 1_000_001,
            min_width: 10,
            min_height: 10,
        },
    ] {
        assert_eq!(
            initialize(1000, 1000, config).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
    }
    let d = populated();
    rejected(
        &d,
        set_bounds(d.clone(), 1_000_001, 1),
        ErrorCode::InvalidArgument,
    );
    rejected(
        &d,
        create_panel(d.clone(), "".into(), None, Side::Right, 0.5),
        ErrorCode::InvalidArgument,
    );
    rejected(
        &d,
        create_panel(d.clone(), "a".into(), None, Side::Right, 0.5),
        ErrorCode::DuplicatePanel,
    );
    rejected(
        &d,
        create_panel(
            d.clone(),
            "c".into(),
            Some("missing".into()),
            Side::Right,
            0.5,
        ),
        ErrorCode::UnknownPanel,
    );
    rejected(
        &d,
        create_workspace(d.clone(), "".into(), "Other".into()),
        ErrorCode::InvalidArgument,
    );
    rejected(
        &d,
        create_workspace(d.clone(), "main".into(), "Other".into()),
        ErrorCode::DuplicateWorkspace,
    );
    rejected(
        &d,
        switch_workspace(d.clone(), "missing".into()),
        ErrorCode::UnknownWorkspace,
    );
    rejected(
        &d,
        rename_workspace(d.clone(), "missing".into(), "Other".into()),
        ErrorCode::UnknownWorkspace,
    );
    for fraction in [0.0, 1.0, -0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        rejected(
            &d,
            create_panel(d.clone(), "c".into(), None, Side::Right, fraction),
            ErrorCode::InvalidArgument,
        );
        rejected(
            &d,
            set_split_ratio(d.clone(), 1, fraction),
            ErrorCode::InvalidArgument,
        );
        let mut malformed = d.clone();
        split_mut(&mut malformed).ratio = fraction;
        // NaN is not reflexively equal; assert the error without comparing it.
        assert_eq!(
            get_snapshot(malformed).unwrap_err().code,
            ErrorCode::InvalidDocument
        );
    }
    for (side, id) in [(Side::Right, "a"), (Side::Left, "b")] {
        for delta in [i32::MIN, i32::MAX] {
            rejected(
                &d,
                resize_panel(d.clone(), id.into(), side, delta),
                ErrorCode::InsufficientSpace,
            );
        }
    }
    rejected(
        &d,
        create_panel(d.clone(), "c".into(), None, Side::Right, 0.000001),
        ErrorCode::InsufficientSpace,
    );
    rejected(
        &d,
        move_panel(d.clone(), "a".into(), "missing".into(), Side::Left, 0.5),
        ErrorCode::UnknownPanel,
    );
    let before = d.clone();
    assert!(get_snapshot(d.clone()).is_ok());
    assert_eq!(d, before);
}
