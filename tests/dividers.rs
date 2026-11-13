use gams_layout2::*;

fn initial(width: u32, height: u32, gap: u32) -> LayoutResponse {
    let d = initialize(
        width,
        height,
        LayoutConfig {
            gap,
            min_width: 20,
            min_height: 15,
        },
    )
    .unwrap();
    create_panel(d.document, "a".into(), None, Side::Right, 0.5).unwrap()
}
fn split(d: LayoutResponse, id: &str, target: &str, side: Side) -> LayoutResponse {
    create_panel(d.document, id.into(), Some(target.into()), side, 0.5).unwrap()
}
fn panel<'a>(d: &'a LayoutResponse, id: &str) -> &'a PlacedPanel {
    d.snapshot.panels.iter().find(|p| p.id == id).unwrap()
}
fn axis(p: &PlacedPanel, vertical: bool) -> (u32, u32) {
    if vertical {
        (p.x, p.w)
    } else {
        (p.y, p.h)
    }
}
fn valid_geometry(d: &LayoutResponse) {
    assert!(!d.snapshot.constrained);
    for (i, a) in d.snapshot.panels.iter().enumerate() {
        assert!(a.w >= 20 && a.h >= 15);
        assert!(a.x + a.w <= d.document.width && a.y + a.h <= d.document.height);
        for b in &d.snapshot.panels[i + 1..] {
            assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y);
        }
    }
}
fn rejected(d: &LayoutDocument, result: Result<LayoutResponse, LayoutError>, code: ErrorCode) {
    let before = d.clone();
    let error = result.unwrap_err();
    assert_eq!(error.code, code, "{}", error.message);
    assert_eq!(*d, before);
}

#[test]
fn vertical_and_horizontal_absolute_pixel_movement() {
    for (side, vertical) in [(Side::Right, true), (Side::Down, false)] {
        let mut d = split(initial(407, 307, 7), "b", "a", side);
        let focus = d.document.workspaces[0].focused.clone();
        let revision = d.document.revision;
        d = resize_divider(d.document, 1, 123).unwrap();
        let guide = &d.snapshot.splits[0];
        assert_eq!(if vertical { guide.x } else { guide.y }, 123);
        assert_eq!(axis(panel(&d, "a"), vertical).1, 123);
        assert_eq!(axis(panel(&d, "b"), vertical).0, 130);
        assert_eq!(
            axis(panel(&d, "a"), !vertical).1,
            axis(panel(&d, "b"), !vertical).1
        );
        assert_eq!(d.document.revision, revision + 1);
        assert_eq!(d.document.workspaces[0].focused, focus);
        valid_geometry(&d);
    }
}

#[test]
fn exact_minimum_clamps_extreme_positions_in_both_axes_with_and_without_gap() {
    for gap in [0, 7] {
        for (side, vertical, extent, minimum) in
            [(Side::Right, true, 407, 20), (Side::Down, false, 307, 15)]
        {
            let mut d = split(initial(407, 307, gap), "b", "a", side);
            d = resize_divider(d.document, 1, i32::MIN).unwrap();
            assert_eq!(axis(panel(&d, "a"), vertical).1, minimum);
            valid_geometry(&d);
            d = resize_divider(d.document, 1, i32::MAX).unwrap();
            assert_eq!(axis(panel(&d, "a"), vertical).1, extent - gap - minimum);
            assert_eq!(axis(panel(&d, "b"), vertical).1, minimum);
            valid_geometry(&d);
        }
    }
}

#[test]
fn nested_fixed_ratios_determine_exact_feasible_span_not_aggregate_minima() {
    for (side, cross, vertical, minimum) in [
        (Side::Right, Side::Down, true, 20),
        (Side::Down, Side::Right, false, 15),
    ] {
        for descendant_in_first in [true, false] {
            let mut d = split(initial(2000, 2000, 7), "b", "a", side);
            let target = if descendant_in_first { "a" } else { "b" };
            d = split(d, "c", target, cross);
            d = split(d, "d", target, side);
            d = set_split_ratio(d.document, 3, 0.1).unwrap();
            let descendants: Vec<_> = d
                .snapshot
                .splits
                .iter()
                .filter(|s| s.id != 1)
                .map(|s| (s.id, s.ratio))
                .collect();
            d = resize_divider(
                d.document,
                1,
                if descendant_in_first {
                    i32::MIN
                } else {
                    i32::MAX
                },
            )
            .unwrap();
            let required = 10 * minimum - 5 + 7;
            assert_eq!(axis(panel(&d, target), vertical).1, minimum);
            assert_eq!(axis(panel(&d, "c"), vertical).1, required);
            let after: Vec<_> = d
                .snapshot
                .splits
                .iter()
                .filter(|s| s.id != 1)
                .map(|s| (s.id, s.ratio))
                .collect();
            assert_eq!(after, descendants);
            valid_geometry(&d);
        }
    }
}

#[test]
fn nested_coordinates_subtract_nonzero_origin() {
    for (side, vertical) in [(Side::Right, true), (Side::Down, false)] {
        let mut d = split(initial(1007, 1007, 7), "b", "a", side);
        d = split(d, "c", "b", side);
        assert_eq!(axis(panel(&d, "b"), vertical).0, 507);
        d = resize_divider(d.document, 2, 700).unwrap();
        assert_eq!(axis(panel(&d, "b"), vertical).1, 193);
        assert_eq!(axis(panel(&d, "c"), vertical).0, 707);
        let guide = &d.snapshot.splits[0];
        assert_eq!(if vertical { guide.x } else { guide.y }, 500);
        d = resize_divider(d.document, 2, i32::MIN).unwrap();
        assert_eq!(
            axis(panel(&d, "b"), vertical).1,
            if vertical { 20 } else { 15 }
        );
        valid_geometry(&d);
    }
}

#[test]
fn zero_gap_viewport_changes_use_current_extent_and_stable_split_id() {
    let mut d = split(initial(407, 307, 0), "b", "a", Side::Right);
    d = set_bounds(d.document, 200, 100).unwrap();
    d = resize_divider(d.document, 1, 180).unwrap();
    assert_eq!(panel(&d, "a").w, 180);
    assert_eq!(panel(&d, "b").x, 180);
    assert_eq!(panel(&d, "b").w, 20);
    assert_eq!(d.snapshot.splits[0].id, 1);
    assert_eq!(d.snapshot.splits[0].w, 0);
    valid_geometry(&d);
    let saved = d.document.clone();
    assert_eq!(get_snapshot(d.document.clone()).unwrap(), d.snapshot);
    assert_eq!(d.document, saved);
}

#[test]
fn unknown_inactive_and_constrained_errors_are_typed_and_atomic() {
    let mut d = split(initial(407, 307, 7), "b", "a", Side::Right);
    rejected(
        &d.document,
        resize_divider(d.document.clone(), 99, 50),
        ErrorCode::UnknownSplit,
    );
    d = create_workspace(d.document, "other".into(), "Other".into()).unwrap();
    rejected(
        &d.document,
        resize_divider(d.document.clone(), 1, 50),
        ErrorCode::UnknownSplit,
    );
    d = switch_workspace(d.document, "main".into()).unwrap();
    rejected(
        &d.document,
        set_split_ratio(d.document.clone(), 1, 0.01),
        ErrorCode::InsufficientSpace,
    );
    d = set_bounds(d.document, 10, 10).unwrap();
    for id in [1, 99] {
        rejected(
            &d.document,
            resize_divider(d.document.clone(), id, 50),
            ErrorCode::InsufficientSpace,
        );
    }
}

#[test]
fn divider_validates_js_safe_ids() {
    let d = initial(407, 307, 0);
    for id in [0, 9_007_199_254_740_992, u64::MAX] {
        rejected(
            &d.document,
            resize_divider(d.document.clone(), id, 1),
            ErrorCode::InvalidArgument,
        );
    }
    rejected(
        &d.document,
        resize_divider(d.document.clone(), 9_007_199_254_740_991, 1),
        ErrorCode::UnknownSplit,
    );
}
