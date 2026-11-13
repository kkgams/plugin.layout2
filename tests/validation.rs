use gams_layout2::execute;
use serde_json::{json, Value};
fn initial() -> Value {
    serde_json::from_str(
        &execute(
            "",
            r#"{"op":"init","width":1000,"height":1000,"gap":6,"min_width":10,"min_height":10}"#,
        )
        .unwrap(),
    )
    .unwrap()
}
fn call(state: &Value, c: Value) -> Value {
    serde_json::from_str(&execute(&state.to_string(), &c.to_string()).unwrap()).unwrap()
}
fn rejected(state: &Value, c: Value, code: &str) {
    let e: Value =
        serde_json::from_str(&execute(&state.to_string(), &c.to_string()).unwrap_err()).unwrap();
    assert_eq!(e["code"], code, "{e}");
}
fn populated() -> Value {
    let d = initial();
    let d = call(&d["state"], json!({"op":"create","id":"a"}));
    call(&d["state"], json!({"op":"create","id":"b"}))
}
#[test]
fn duplicate_ids_bad_focus_and_counter_rejected() {
    let d = populated();
    let base = d["state"].clone();
    let mut s = base.clone();
    s["workspaces"][0]["tree"]["second"]["id"] = json!("a");
    rejected(&s, json!({"op":"snapshot"}), "invalid-state");
    let mut s = base.clone();
    s["workspaces"][0]["focused"] = json!("missing");
    rejected(&s, json!({"op":"snapshot"}), "invalid-state");
    let mut s = base.clone();
    s["next_split_id"] = json!(1);
    rejected(&s, json!({"op":"snapshot"}), "invalid-state");
    let mut s = base.clone();
    let w = s["workspaces"][0].clone();
    s["workspaces"].as_array_mut().unwrap().push(w);
    s["workspaces"][1]["id"] = json!("other");
    rejected(&s, json!({"op":"snapshot"}), "invalid-state");
    for key in ["focused", "tree"] {
        let mut s = base.clone();
        s["workspaces"][0].as_object_mut().unwrap().remove(key);
        rejected(&s, json!({"op":"snapshot"}), "invalid-state");
    }
    for key in ["revision", "next_split_id"] {
        let mut s = base.clone();
        s[key] = json!(9_007_199_254_740_992_u64);
        rejected(&s, json!({"op":"snapshot"}), "invalid-state");
    }
    let mut s = base.clone();
    s["revision"] = json!(9_007_199_254_740_991_u64);
    rejected(&s, json!({"op":"focus","id":"a"}), "limit-exceeded");
    assert_eq!(call(&s, json!({"op":"snapshot"}))["state"], s);
    let mut s = base;
    s["next_split_id"] = json!(9_007_199_254_740_991_u64);
    rejected(&s, json!({"op":"create","id":"c"}), "limit-exceeded");
}
#[test]
fn malformed_commands_and_extreme_resize_are_safe() {
    let d = populated();
    let s = &d["state"];
    for c in [
        json!({"op":"set-bounds","width":1_000_001,"height":1}),
        json!({"op":"create","id":"c","side":"diagonal"}),
        json!({"op":"create","id":"c","target":1}),
        json!({"op":"resize","id":"a","side":"right","delta":1.5}),
        json!({"op":"set-split-ratio","split_id":9_007_199_254_740_992_u64,"ratio":0.5}),
    ] {
        rejected(s, c, "invalid-command");
    }
    for side in ["right", "left"] {
        let id = if side == "right" { "a" } else { "b" };
        for delta in [i32::MIN, i32::MAX] {
            rejected(
                s,
                json!({"op":"resize","id":id,"side":side,"delta":delta}),
                "insufficient-space",
            );
        }
    }
    rejected(
        s,
        json!({"op":"create","id":"c","ratio":0.000001}),
        "insufficient-space",
    );
    rejected(
        s,
        json!({"op":"move","id":"a","target":"missing","side":"left"}),
        "unknown-panel",
    );
    assert_eq!(call(s, json!({"op":"snapshot"})), d);
}
#[test]
fn resize_relevant_ancestor_swap_root_and_noop_revision() {
    let mut d = populated();
    d = call(&d["state"], json!({"op":"create","id":"c","side":"down"}));
    let root = d["snapshot"]["splits"][0]["id"].clone();
    d = call(
        &d["state"],
        json!({"op":"resize","id":"c","side":"left","delta":25}),
    );
    assert_eq!(d["snapshot"]["panels"][0]["w"], 472);
    assert_eq!(d["snapshot"]["splits"][0]["id"], root);
    d = call(&d["state"], json!({"op":"swap","id":"a","target":"c"}));
    assert_eq!(d["snapshot"]["panels"][0]["id"], "c");
    d = call(
        &d["state"],
        json!({"op":"move-root","id":"b","side":"up","ratio":0.25}),
    );
    assert_eq!(d["snapshot"]["panels"][0]["id"], "b");
    let rev = d["snapshot"]["revision"].as_u64().unwrap();
    d = call(
        &d["state"],
        json!({"op":"move","id":"b","target":"b","side":"left"}),
    );
    assert_eq!(d["snapshot"]["revision"], rev + 1);
    let id = d["snapshot"]["splits"][0]["id"].clone();
    d = call(
        &d["state"],
        json!({"op":"set-split-ratio","split_id":id,"ratio":0.4}),
    );
    assert_eq!(d["snapshot"]["splits"][0]["ratio"], 0.4);
}
#[test]
fn independent_documents_allocate_identically() {
    assert_eq!(populated(), populated());
}
