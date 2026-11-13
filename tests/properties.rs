use gams_layout2::execute;
use serde_json::{json, Value};
fn init() -> Value {
    serde_json::from_str(
        &execute(
            "",
            r#"{"op":"init","width":1000,"height":800,"gap":6,"min_width":10,"min_height":10}"#,
        )
        .unwrap(),
    )
    .unwrap()
}
fn apply(state: &Value, c: Value) -> Result<Value, String> {
    execute(&state.to_string(), &c.to_string()).map(|v| serde_json::from_str(&v).unwrap())
}
fn balanced(start: usize, n: usize, counter: &mut u64) -> Value {
    if n == 1 {
        return json!({"kind":"leaf","id":format!("p{start}")});
    }
    let id = *counter;
    *counter += 1;
    let a = n / 2;
    json!({"kind":"split","id":id,"vertical":true,"ratio":0.5,"first":balanced(start,a,counter),"second":balanced(start+a,n-a,counter)})
}
fn error_code(result: Result<Value, String>) -> Value {
    serde_json::from_str::<Value>(&result.unwrap_err()).unwrap()["code"].clone()
}
#[test]
fn depth_and_count_are_bounded_but_maximum_documents_import() {
    let mut d = init();
    let mut tree = json!({"kind":"leaf","id":"p0"});
    for i in 1..64 {
        tree = json!({"kind":"split","id":i,"vertical":true,"ratio":0.5,"first":tree,"second":{"kind":"leaf","id":format!("p{i}")}});
    }
    d["state"]["workspaces"][0]["tree"] = tree.clone();
    d["state"]["workspaces"][0]["focused"] = json!("p0");
    d["state"]["next_split_id"] = json!(100);
    let imported = apply(&d["state"], json!({"op":"snapshot"})).unwrap();
    assert_eq!(imported["snapshot"]["constrained"], true);
    assert_eq!(
        imported["snapshot"]["workspaces"][0]["panels"]
            .as_array()
            .unwrap()
            .len(),
        64
    );
    d["state"]["workspaces"][0]["tree"] = json!({"kind":"split","id":64,"vertical":false,"ratio":0.5,"first":tree,"second":{"kind":"leaf","id":"last"}});
    assert_eq!(
        error_code(apply(&d["state"], json!({"op":"snapshot"}))),
        "invalid-state"
    );
    let mut s = init()["state"].clone();
    let mut counter = 1;
    s["workspaces"][0]["tree"] = balanced(0, 1024, &mut counter);
    s["workspaces"][0]["focused"] = json!("p0");
    s["next_split_id"] = json!(counter);
    assert!(apply(&s, json!({"op":"snapshot"})).is_ok());
    let mut counter = 1;
    s["workspaces"][0]["tree"] = balanced(0, 1025, &mut counter);
    s["next_split_id"] = json!(counter);
    assert_eq!(
        error_code(apply(&s, json!({"op":"snapshot"}))),
        "invalid-state"
    );
}
#[test]
fn workspace_move_failure_is_atomic_and_inactive_constraints_are_retained() {
    let mut d = init();
    for id in ["a", "b"] {
        d = apply(&d["state"], json!({"op":"create","id":id})).unwrap();
    }
    d = apply(
        &d["state"],
        json!({"op":"workspace-create","id":"other","name":"Other"}),
    )
    .unwrap();
    d = apply(&d["state"], json!({"op":"create","id":"c"})).unwrap();
    d = apply(
        &d["state"],
        json!({"op":"set-bounds","width":10,"height":10}),
    )
    .unwrap();
    let before = d.clone();
    assert_eq!(
        error_code(apply(
            &d["state"],
            json!({"op":"workspace-move","id":"c","workspace":"main"})
        )),
        "insufficient-space"
    );
    assert_eq!(
        apply(&d["state"], json!({"op":"snapshot"})).unwrap(),
        before
    );
    d = apply(&d["state"], json!({"op":"set-bounds","width":0,"height":0})).unwrap();
    assert_eq!(d["snapshot"]["constrained"], true);
    d = apply(&d["state"], json!({"op":"workspace-switch","id":"main"})).unwrap();
    assert_eq!(d["snapshot"]["workspaces"][0]["panels"], json!(["a", "b"]));
    assert_eq!(d["snapshot"]["panels"], json!([]));
}
#[test]
fn deterministic_replay_preserves_membership_and_nonoverlap() {
    let mut d = init();
    let mut seed = 123456789_u64;
    for turn in 0..300 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let ids: Vec<String> = d["snapshot"]["workspaces"][0]["panels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect();
        let id = ids
            .get(seed as usize % ids.len().max(1))
            .cloned()
            .unwrap_or_default();
        let side = ["left", "right", "up", "down"][(seed >> 8) as usize % 4];
        let c = match seed % 9 {
            0 | 1 => json!({"op":"create","id":format!("p{turn}"),"side":side}),
            2 if !ids.is_empty() => json!({"op":"close","id":id}),
            3 if !ids.is_empty() => json!({"op":"move-root","id":id,"side":side,"ratio":0.3}),
            4 if !ids.is_empty() => json!({"op":"resize","id":id,"side":side,"delta":13}),
            5 => {
                json!({"op":"arrange","kind":(["horizontal","vertical","grid"][(seed>>16)as usize%3])})
            }
            6 => json!({"op":"set-bounds","width":50+(seed>>16)%1000,"height":50+(seed>>32)%800}),
            7 => json!({"op":"equalize"}),
            _ => json!({"op":"focus-direction","side":side}),
        };
        match apply(&d["state"], c) {
            Ok(next) => d = next,
            Err(e) => {
                let e: Value = serde_json::from_str(&e).unwrap();
                assert_eq!(e["code"], "insufficient-space");
            }
        }
        assert_eq!(apply(&d["state"], json!({"op":"snapshot"})).unwrap(), d);
        let panels = d["snapshot"]["panels"].as_array().unwrap();
        for (a, p) in panels.iter().enumerate() {
            let x = p["x"].as_u64().unwrap();
            let y = p["y"].as_u64().unwrap();
            let w = p["w"].as_u64().unwrap();
            let h = p["h"].as_u64().unwrap();
            assert!(w >= 10 && h >= 10);
            assert!(
                x + w <= d["snapshot"]["width"].as_u64().unwrap()
                    && y + h <= d["snapshot"]["height"].as_u64().unwrap()
            );
            for q in &panels[a + 1..] {
                let qx = q["x"].as_u64().unwrap();
                let qy = q["y"].as_u64().unwrap();
                assert!(
                    x + w <= qx
                        || qx + q["w"].as_u64().unwrap() <= x
                        || y + h <= qy
                        || qy + q["h"].as_u64().unwrap() <= y
                );
            }
        }
    }
}
