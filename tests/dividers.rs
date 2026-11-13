use gams_layout2::execute;
use serde_json::{json, Value};

fn step(doc: &mut Value, command: Value) {
    *doc = serde_json::from_str(&execute(&doc["state"].to_string(), &command.to_string()).unwrap())
        .unwrap();
}
fn initial(width: u32, height: u32, gap: u32) -> Value {
    let mut doc: Value = serde_json::from_str(
        &execute(
            "",
            &json!({"op":"init", "width":width, "height":height, "gap":gap,
                "min_width":20, "min_height":15})
            .to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    step(&mut doc, json!({"op":"create", "id":"a"}));
    doc
}
fn split(doc: &mut Value, id: &str, target: &str, side: &str) {
    step(
        doc,
        json!({"op":"split", "id":id, "target":target, "side":side}),
    );
}
fn drag(doc: &mut Value, split_id: u64, position: i32) {
    step(
        doc,
        json!({"op":"resize-divider", "split_id":split_id, "position":position}),
    );
}
fn panel<'a>(doc: &'a Value, id: &str) -> &'a Value {
    doc["snapshot"]["panels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap()
}
fn rejected(doc: &Value, command: Value, code: &str) {
    let before = doc.clone();
    let error: Value = serde_json::from_str(
        &execute(&doc["state"].to_string(), &command.to_string()).unwrap_err(),
    )
    .unwrap();
    assert_eq!(error["code"], code, "{error}");
    let mut after = doc.clone();
    step(&mut after, json!({"op":"snapshot"}));
    assert_eq!(after, before);
}
fn valid_geometry(doc: &Value) {
    assert_eq!(doc["snapshot"]["constrained"], false);
    let panels = doc["snapshot"]["panels"].as_array().unwrap();
    for (i, a) in panels.iter().enumerate() {
        let num = |p: &Value, key: &str| p[key].as_u64().unwrap();
        assert!(num(a, "w") >= 20 && num(a, "h") >= 15);
        assert!(num(a, "x") + num(a, "w") <= doc["state"]["width"].as_u64().unwrap());
        assert!(num(a, "y") + num(a, "h") <= doc["state"]["height"].as_u64().unwrap());
        for b in &panels[i + 1..] {
            assert!(
                num(a, "x") + num(a, "w") <= num(b, "x")
                    || num(b, "x") + num(b, "w") <= num(a, "x")
                    || num(a, "y") + num(a, "h") <= num(b, "y")
                    || num(b, "y") + num(b, "h") <= num(a, "y")
            );
        }
    }
}

#[test]
fn vertical_and_horizontal_absolute_pixel_movement() {
    for (side, coordinate, span, other) in [("right", "x", "w", "h"), ("down", "y", "h", "w")] {
        let mut doc = initial(407, 307, 7);
        split(&mut doc, "b", "a", side);
        let focus = doc["state"]["workspaces"][0]["focused"].clone();
        let revision = doc["state"]["revision"].as_u64().unwrap();
        drag(&mut doc, 1, 123);
        assert_eq!(doc["snapshot"]["splits"][0][coordinate], 123);
        assert_eq!(panel(&doc, "a")[span], 123);
        assert_eq!(panel(&doc, "b")[coordinate], 130);
        assert_eq!(panel(&doc, "a")[other], panel(&doc, "b")[other]);
        assert_eq!(doc["state"]["revision"], revision + 1);
        assert_eq!(doc["state"]["workspaces"][0]["focused"], focus);
        valid_geometry(&doc);
    }
}

#[test]
fn exact_minimum_clamps_extreme_positions_in_both_axes_with_and_without_gap() {
    for gap in [0, 7] {
        for (side, span, extent, minimum) in [("right", "w", 407, 20), ("down", "h", 307, 15)] {
            let mut doc = initial(407, 307, gap);
            split(&mut doc, "b", "a", side);
            drag(&mut doc, 1, i32::MIN);
            assert_eq!(panel(&doc, "a")[span], minimum);
            valid_geometry(&doc);
            drag(&mut doc, 1, i32::MAX);
            assert_eq!(panel(&doc, "a")[span], extent - gap - minimum);
            assert_eq!(panel(&doc, "b")[span], minimum);
            valid_geometry(&doc);
        }
    }
}

#[test]
fn nested_fixed_ratios_determine_exact_feasible_span_not_aggregate_minima() {
    for (side, cross, span, minimum) in [("right", "down", "w", 20), ("down", "right", "h", 15)] {
        for descendant_in_first in [true, false] {
            let mut doc = initial(2000, 2000, 7);
            split(&mut doc, "b", "a", side);
            let target = if descendant_in_first { "a" } else { "b" };
            split(&mut doc, "c", target, cross);
            split(&mut doc, "d", target, side);
            step(
                &mut doc,
                json!({"op":"set-split-ratio", "split_id":3, "ratio":0.1}),
            );
            let subtree_before = doc["state"]["workspaces"][0]["tree"][if descendant_in_first {
                "first"
            } else {
                "second"
            }]
            .clone();
            drag(
                &mut doc,
                1,
                if descendant_in_first {
                    i32::MIN
                } else {
                    i32::MAX
                },
            );
            // round((span - gap) * .1) >= minimum; ties round upward.
            let required = 10 * minimum - 5 + 7;
            assert_eq!(panel(&doc, target)[span], minimum);
            assert_eq!(panel(&doc, "c")[span], required);
            assert_eq!(
                doc["state"]["workspaces"][0]["tree"][if descendant_in_first {
                    "first"
                } else {
                    "second"
                }],
                subtree_before
            );
            valid_geometry(&doc);
        }
    }
}

#[test]
fn nested_coordinates_subtract_nonzero_origin() {
    for (side, coordinate, span) in [("right", "x", "w"), ("down", "y", "h")] {
        let mut doc = initial(1007, 1007, 7);
        split(&mut doc, "b", "a", side);
        split(&mut doc, "c", "b", side);
        assert_eq!(panel(&doc, "b")[coordinate], 507);
        drag(&mut doc, 2, 700);
        assert_eq!(panel(&doc, "b")[span], 193);
        assert_eq!(panel(&doc, "c")[coordinate], 707);
        assert_eq!(doc["snapshot"]["splits"][0][coordinate], 500);
        drag(&mut doc, 2, i32::MIN);
        assert_eq!(
            panel(&doc, "b")[span],
            if side == "right" { 20 } else { 15 }
        );
        valid_geometry(&doc);
    }
}

#[test]
fn zero_gap_viewport_changes_use_current_extent_and_stable_split_id() {
    let mut doc = initial(407, 307, 0);
    split(&mut doc, "b", "a", "right");
    step(
        &mut doc,
        json!({"op":"set-bounds", "width":200, "height":100}),
    );
    drag(&mut doc, 1, 180);
    assert_eq!(panel(&doc, "a")["w"], 180);
    assert_eq!(panel(&doc, "b")["x"], 180);
    assert_eq!(panel(&doc, "b")["w"], 20);
    assert_eq!(doc["snapshot"]["splits"][0]["id"], 1);
    assert_eq!(doc["snapshot"]["splits"][0]["w"], 0);
    valid_geometry(&doc);
    let saved = doc.clone();
    step(&mut doc, json!({"op":"snapshot"}));
    assert_eq!(doc, saved);
}

#[test]
fn unknown_inactive_and_constrained_errors_are_typed_and_atomic() {
    let mut doc = initial(407, 307, 7);
    split(&mut doc, "b", "a", "right");
    rejected(
        &doc,
        json!({"op":"resize-divider", "split_id":99, "position":50}),
        "unknown-split",
    );
    step(
        &mut doc,
        json!({"op":"workspace-create", "id":"other", "name":"Other"}),
    );
    rejected(
        &doc,
        json!({"op":"resize-divider", "split_id":1, "position":50}),
        "unknown-split",
    );
    step(&mut doc, json!({"op":"workspace-switch", "id":"main"}));
    rejected(
        &doc,
        json!({"op":"set-split-ratio", "split_id":1, "ratio":0.01}),
        "insufficient-space",
    );
    step(
        &mut doc,
        json!({"op":"set-bounds", "width":10, "height":10}),
    );
    rejected(
        &doc,
        json!({"op":"resize-divider", "split_id":1, "position":50}),
        "insufficient-space",
    );
    rejected(
        &doc,
        json!({"op":"resize-divider", "split_id":99, "position":50}),
        "insufficient-space",
    );
}

#[test]
fn divider_command_validates_js_safe_ids_and_integer_positions() {
    let doc = initial(407, 307, 0);
    for split_id in [0, 9_007_199_254_740_992_u64, u64::MAX] {
        rejected(
            &doc,
            json!({"op":"resize-divider", "split_id":split_id, "position":1}),
            "invalid-command",
        );
    }
    for position in [
        json!(1.5),
        json!(2_147_483_648_u64),
        json!(-2_147_483_649_i64),
    ] {
        rejected(
            &doc,
            json!({"op":"resize-divider", "split_id":1, "position":position}),
            "invalid-command",
        );
    }
    rejected(
        &doc,
        json!({"op":"resize-divider", "split_id":9_007_199_254_740_991_u64, "position":1}),
        "unknown-split",
    );
}
