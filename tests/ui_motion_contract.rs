#![cfg(feature = "native-ui")]

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn view(kind: &str, props: Vec<Value>) -> Value {
    json!({"schemaVersion":1,"root":"root","nodes":[{
        "key":"root","kind":{"tag":kind,"values":[]},"props":props,"children":[]
    }]})
}

fn prop(name: &str, value: Value) -> Value {
    json!({"tag":name,"values":[value]})
}

#[test]
fn native_and_browser_accept_the_same_ui_contract() {
    let motion = json!({"durationMs":220,"delayMs":0,"easing":{"tag":"Spring","values":[]}});
    let mut cases = vec![
        json!({"name":"old static view", "view":view("Text", vec![]), "valid":true}),
        json!({"name":"bounded spring", "view":view("Text", vec![prop("Motion", motion.clone())]), "valid":true}),
        json!({"name":"opacity upper bound", "view":view("Text",vec![prop("Opacity",json!(100))]), "valid":true}),
        json!({"name":"scale lower bound", "view":view("Text",vec![prop("Scale",json!(25))]), "valid":true}),
    ];
    for (name, value) in [
        ("Opacity", json!(101)),
        ("Opacity", json!(0.5)),
        ("Scale", json!(24)),
        ("Scale", json!(201)),
    ] {
        cases.push(json!({"name":format!("invalid {name}: {value}"),"view":view("Text",vec![prop(name,value)]),"valid":false}));
    }
    for (field, value) in [
        ("durationMs", json!(2001)),
        ("durationMs", json!(-1)),
        ("delayMs", json!(1001)),
        ("delayMs", json!(0.5)),
        ("extra", json!(true)),
        ("easing", json!({"tag":"Spring","values":[1]})),
    ] {
        let mut invalid = motion.clone();
        invalid[field] = value;
        cases.push(json!({"name":format!("invalid motion {field}"),"view":view("Text",vec![prop("Motion",invalid)]),"valid":false}));
    }
    for (coords, valid) in [
        (vec![json!(-100000), json!(100000)], true),
        (vec![json!(100001), json!(0)], false),
        (vec![json!(1.5), json!(0)], false),
        (vec![json!(0)], false),
    ] {
        cases.push(json!({"name":format!("Translate {coords:?}"),"view":view("Text",vec![json!({"tag":"Translate","values":coords})]),"valid":valid}));
    }
    cases.push(json!({"name":"duplicate Motion", "view":view("Text",vec![prop("Motion",motion.clone()),prop("Motion",motion)]),"valid":false}));
    for fixtures in [
        include_str!("../ubi-ui/fixtures/overlays.json"),
        include_str!("../ubi-ui/fixtures/effects.json"),
    ] {
        cases.extend(serde_json::from_str::<Vec<Value>>(fixtures).unwrap());
    }
    for case in &cases {
        assert_eq!(
            ubi_ui_platform::View::decode(case["view"].clone()).is_ok(),
            case["valid"].as_bool().unwrap(),
            "native ABI: {}",
            case["name"]
        );
    }
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("ubi-ui/tools/verify_view_contract.mjs");
    let mut child = Command::new("node")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&cases).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
