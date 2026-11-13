use gams_layout2::execute;
use serde_json::{json, Value};
fn run(state: &Value, cmd: Value) -> Value {
    serde_json::from_str(&execute(&state.to_string(), &cmd.to_string()).unwrap()).unwrap()
}
fn init() -> Value {
    serde_json::from_str(
        &execute(
            "",
            &json!({"op":"init","width":1000,"height":600,"gap":6,"min_width":80,"min_height":60})
                .to_string(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn step(doc: &mut Value, cmd: Value) {
    *doc = run(&doc["state"], cmd);
}
fn panel<'a>(doc: &'a Value, id: &str) -> &'a Value {
    doc["snapshot"]["panels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap()
}
fn error(state: &Value, cmd: Value) -> Value {
    serde_json::from_str(&execute(&state.to_string(), &cmd.to_string()).unwrap_err()).unwrap()
}
#[test]
fn split_move_close_and_stable_roundtrip() {
    let mut d = init();
    step(&mut d, json!({"op":"create","id":"a"}));
    step(
        &mut d,
        json!({"op":"split","id":"b","target":"a","side":"right"}),
    );
    assert_eq!(panel(&d, "a")["w"], 497);
    assert_eq!(panel(&d, "b")["x"], 503);
    let root = d["snapshot"]["splits"][0]["id"].clone();
    step(
        &mut d,
        json!({"op":"create","id":"c","side":"down","ratio":0.25}),
    );
    assert_eq!(panel(&d, "c")["h"], 148);
    let saved = d.clone();
    step(&mut d, json!({"op":"snapshot"}));
    assert_eq!(d, saved);
    step(
        &mut d,
        json!({"op":"move","id":"c","target":"a","side":"left"}),
    );
    assert_eq!(d["snapshot"]["splits"][0]["id"], root);
    assert!(panel(&d, "c")["x"].as_u64().unwrap() < panel(&d, "a")["x"].as_u64().unwrap());
    step(&mut d, json!({"op":"close","id":"a"}));
    step(&mut d, json!({"op":"close","id":"b"}));
    assert_eq!(panel(&d, "c")["w"], 1000);
    assert!(d["snapshot"]["splits"].as_array().unwrap().is_empty());
}
#[test]
fn workspace_transfer_focus_and_order() {
    let mut d = init();
    step(&mut d, json!({"op":"create","id":"a"}));
    step(&mut d, json!({"op":"create","id":"b"}));
    step(
        &mut d,
        json!({"op":"workspace-create","id":"other","name":"Other"}),
    );
    assert_eq!(d["snapshot"]["active_workspace"], "other");
    step(&mut d, json!({"op":"workspace-switch","id":"main"}));
    step(
        &mut d,
        json!({"op":"workspace-move","id":"b","workspace":"other"}),
    );
    assert_eq!(d["snapshot"]["active_workspace"], "main");
    assert_eq!(d["snapshot"]["focused"], "a");
    assert_eq!(d["snapshot"]["workspaces"][1]["panels"], json!(["b"]));
    step(&mut d, json!({"op":"workspace-switch","id":"other"}));
    assert_eq!(d["snapshot"]["focused"], "b");
    assert_eq!(panel(&d, "b")["w"], 1000);
}
#[test]
fn constrained_bounds_and_atomic_errors() {
    let mut d = init();
    step(&mut d, json!({"op":"create","id":"a"}));
    step(&mut d, json!({"op":"create","id":"b"}));
    step(&mut d, json!({"op":"set-bounds","width":10,"height":10}));
    assert_eq!(d["snapshot"]["constrained"], true);
    assert_eq!(d["snapshot"]["panels"], json!([]));
    let before = d.clone();
    assert_eq!(
        error(&d["state"], json!({"op":"create","id":"c"}))["code"],
        "insufficient-space"
    );
    assert_eq!(run(&d["state"], json!({"op":"snapshot"})), before);
    step(&mut d, json!({"op":"set-bounds","width":1000,"height":600}));
    assert_eq!(d["snapshot"]["constrained"], false);
    assert_eq!(d["snapshot"]["panels"].as_array().unwrap().len(), 2);
}
#[test]
fn arrange_equalize_focus_resize() {
    let mut d = init();
    for id in ["a", "b", "c", "d"] {
        step(&mut d, json!({"op":"create","id":id,"side":"down"}));
    }
    step(&mut d, json!({"op":"arrange","kind":"horizontal"}));
    let widths: Vec<_> = d["snapshot"]["panels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["w"].as_u64().unwrap())
        .collect();
    assert!(widths.iter().max().unwrap() - widths.iter().min().unwrap() <= 1);
    step(&mut d, json!({"op":"focus","id":"a"}));
    step(&mut d, json!({"op":"focus-direction","side":"right"}));
    assert_eq!(d["snapshot"]["focused"], "b");
    let before = panel(&d, "a")["w"].as_u64().unwrap();
    step(
        &mut d,
        json!({"op":"resize","id":"a","side":"right","delta":20}),
    );
    assert_eq!(panel(&d, "a")["w"].as_u64().unwrap(), before + 20);
    step(&mut d, json!({"op":"equalize"}));
    for split in d["snapshot"]["splits"].as_array().unwrap() {
        assert_eq!(split["ratio"], 0.5);
    }
    step(&mut d, json!({"op":"arrange","kind":"grid"}));
    assert_eq!(d["snapshot"]["panels"].as_array().unwrap().len(), 4);
}
#[test]
fn strict_inputs_and_import_validation() {
    let d = init();
    assert_eq!(
        error(
            &d["state"],
            json!({"op":"create","id":"x","target":"missing"})
        )["code"],
        "unknown-panel"
    );
    for c in [
        json!({"op":"unknown"}),
        json!({"op":"close"}),
        json!({"op":"create","id":""}),
        json!({"op":"create","id":"x","ratio":0}),
        json!({"op":"snapshot","ignored":true}),
    ] {
        assert_eq!(error(&d["state"], c)["code"], "invalid-command");
    }
    let mut state = d["state"].clone();
    state["version"] = json!(900);
    assert_eq!(
        error(&state, json!({"op":"snapshot"}))["code"],
        "invalid-state"
    );
    assert!(execute("", r#"{"op":"snapshot"}"#).is_err());
}

#[test]
fn tiled_focus_does_not_jump_diagonally_at_an_outer_edge() {
    let mut d = init();
    step(&mut d, json!({"op":"create","id":"a"}));
    step(
        &mut d,
        json!({"op":"create","id":"b","target":"a","side":"right"}),
    );
    step(
        &mut d,
        json!({"op":"create","id":"c","target":"b","side":"down"}),
    );
    step(&mut d, json!({"op":"focus","id":"a"}));
    for side in ["up", "down", "left"] {
        step(&mut d, json!({"op":"focus-direction","side":side}));
        assert_eq!(d["snapshot"]["focused"], "a");
    }
    step(&mut d, json!({"op":"focus-direction","side":"right"}));
    assert_eq!(d["snapshot"]["focused"], "b");
}
