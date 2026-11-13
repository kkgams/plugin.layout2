use gams_layout2::*;

fn init() -> LayoutResponse {
    initialize(
        1000,
        600,
        LayoutConfig {
            gap: 6,
            min_width: 80,
            min_height: 60,
        },
    )
    .unwrap()
}
fn panel<'a>(d: &'a LayoutResponse, id: &str) -> &'a PlacedPanel {
    d.snapshot.panels.iter().find(|p| p.id == id).unwrap()
}
fn create(
    d: LayoutDocument,
    id: &str,
    target: Option<&str>,
    side: Side,
    share: f64,
) -> LayoutResponse {
    create_panel(d, id.into(), target.map(String::from), side, share).unwrap()
}

#[test]
fn split_move_close_and_stable_roundtrip() {
    let mut d = create(init().document, "a", None, Side::Right, 0.5);
    d = create(d.document, "b", Some("a"), Side::Right, 0.5);
    assert_eq!(panel(&d, "a").w, 497);
    assert_eq!(panel(&d, "b").x, 503);
    let root = d.snapshot.splits[0].id;
    d = create(d.document, "c", None, Side::Down, 0.25);
    assert_eq!(panel(&d, "c").h, 148);
    assert_eq!(get_snapshot(d.document.clone()).unwrap(), d.snapshot);
    d = move_panel(d.document, "c".into(), "a".into(), Side::Left, 0.5).unwrap();
    assert_eq!(d.snapshot.splits[0].id, root);
    assert!(panel(&d, "c").x < panel(&d, "a").x);
    d = close_panel(d.document, "a".into()).unwrap();
    d = close_panel(d.document, "b".into()).unwrap();
    assert_eq!(panel(&d, "c").w, 1000);
    assert!(d.snapshot.splits.is_empty());
}

#[test]
fn workspace_transfer_focus_and_order() {
    let mut d = create(init().document, "a", None, Side::Right, 0.5);
    d = create(d.document, "b", None, Side::Right, 0.5);
    d = create_workspace(d.document, "other".into(), "Other".into()).unwrap();
    assert_eq!(d.snapshot.active_workspace, "other");
    d = rename_workspace(d.document, "other".into(), "Renamed".into()).unwrap();
    assert_eq!(d.snapshot.workspaces[1].name, "Renamed");
    d = switch_workspace(d.document, "main".into()).unwrap();
    d = move_panel_to_workspace(d.document, "b".into(), "other".into()).unwrap();
    assert_eq!(d.snapshot.active_workspace, "main");
    assert_eq!(d.snapshot.focused.as_deref(), Some("a"));
    assert_eq!(d.snapshot.workspaces[1].panels, ["b"]);
    d = switch_workspace(d.document, "other".into()).unwrap();
    assert_eq!(d.snapshot.focused.as_deref(), Some("b"));
    assert_eq!(panel(&d, "b").w, 1000);
}

#[test]
fn constrained_bounds_and_atomic_errors() {
    let mut d = create(init().document, "a", None, Side::Right, 0.5);
    d = create(d.document, "b", None, Side::Right, 0.5);
    d = set_bounds(d.document, 10, 10).unwrap();
    assert!(d.snapshot.constrained);
    assert!(d.snapshot.panels.is_empty());
    let before = d.document.clone();
    let error = create_panel(d.document.clone(), "c".into(), None, Side::Right, 0.5).unwrap_err();
    assert_eq!(error.code, ErrorCode::InsufficientSpace);
    assert_eq!(d.document, before);
    assert_eq!(get_snapshot(d.document.clone()).unwrap(), d.snapshot);
    d = set_bounds(d.document, 1000, 600).unwrap();
    assert!(!d.snapshot.constrained);
    assert_eq!(d.snapshot.panels.len(), 2);
}

#[test]
fn arrange_equalize_focus_resize() {
    let mut d = init();
    for id in ["a", "b", "c", "d"] {
        d = create(d.document, id, None, Side::Down, 0.5);
    }
    d = arrange(d.document, Arrangement::Horizontal).unwrap();
    let widths: Vec<_> = d.snapshot.panels.iter().map(|p| p.w).collect();
    assert!(widths.iter().max().unwrap() - widths.iter().min().unwrap() <= 1);
    d = focus_panel(d.document, "a".into()).unwrap();
    d = focus_direction(d.document, Side::Right).unwrap();
    assert_eq!(d.snapshot.focused.as_deref(), Some("b"));
    let before = panel(&d, "a").w;
    d = resize_panel(d.document, "a".into(), Side::Right, 20).unwrap();
    assert_eq!(panel(&d, "a").w, before + 20);
    d = equalize(d.document).unwrap();
    assert!(d.snapshot.splits.iter().all(|s| s.ratio == 0.5));
    d = arrange(d.document, Arrangement::Vertical).unwrap();
    let heights: Vec<_> = d.snapshot.panels.iter().map(|p| p.h).collect();
    assert!(heights.iter().max().unwrap() - heights.iter().min().unwrap() <= 1);
    d = arrange(d.document, Arrangement::Grid).unwrap();
    assert_eq!(d.snapshot.panels.len(), 4);
}

#[test]
fn tiled_focus_does_not_jump_diagonally_at_an_outer_edge() {
    let mut d = create(init().document, "a", None, Side::Right, 0.5);
    d = create(d.document, "b", Some("a"), Side::Right, 0.5);
    d = create(d.document, "c", Some("b"), Side::Down, 0.5);
    d = focus_panel(d.document, "a".into()).unwrap();
    for side in [Side::Up, Side::Down, Side::Left] {
        let revision = d.document.revision;
        d = focus_direction(d.document, side).unwrap();
        assert_eq!(d.snapshot.focused.as_deref(), Some("a"));
        assert_eq!(d.document.revision, revision + 1);
    }
    let revision = d.document.revision;
    d = focus_panel(d.document, "a".into()).unwrap();
    assert_eq!(d.document.revision, revision + 1);
    d = focus_direction(d.document, Side::Right).unwrap();
    assert_eq!(d.snapshot.focused.as_deref(), Some("b"));
}

#[test]
fn resize_relevant_ancestor_swap_root_and_noop_revision() {
    let mut d = create(init().document, "a", None, Side::Right, 0.5);
    d = create(d.document, "b", None, Side::Right, 0.5);
    d = create(d.document, "c", None, Side::Down, 0.5);
    let root = d.snapshot.splits[0].id;
    d = resize_panel(d.document, "c".into(), Side::Left, 25).unwrap();
    assert_eq!(panel(&d, "a").w, 472);
    assert_eq!(d.snapshot.splits[0].id, root);
    d = swap_panels(d.document, "a".into(), "c".into()).unwrap();
    assert_eq!(d.snapshot.panels[0].id, "c");
    d = move_panel_to_root(d.document, "b".into(), Side::Up, 0.25).unwrap();
    assert_eq!(d.snapshot.panels[0].id, "b");
    let rev = d.document.revision;
    d = move_panel(d.document, "b".into(), "b".into(), Side::Left, 0.5).unwrap();
    assert_eq!(d.document.revision, rev + 1);
    let id = d.snapshot.splits[0].id;
    d = set_split_ratio(d.document, id, 0.4).unwrap();
    assert_eq!(d.snapshot.splits[0].ratio, 0.4);
}

#[test]
fn independent_documents_allocate_identically() {
    let populate = || {
        let d = create(init().document, "a", None, Side::Right, 0.5);
        create(d.document, "b", None, Side::Right, 0.5)
    };
    let first = populate();
    let _unrelated = create(first.document.clone(), "c", None, Side::Down, 0.5);
    assert_eq!(first.document, populate().document);
    assert_eq!(first.snapshot, populate().snapshot);
}
