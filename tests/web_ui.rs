use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

use web_export::sources;
#[allow(dead_code)]
#[path = "../ubi-ui/tools/web_export.rs"]
mod web_export;

#[test]
fn all_web_examples_match_native_initial_views_and_controlled_event_transitions() {
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("ui-web-test-{}", std::process::id()));
    let catalog: Vec<Value> = serde_json::from_str(include_str!("../ubi-ui/catalog.json")).unwrap();
    assert_eq!(web_export::export(&output).unwrap(), catalog.len() + 1);
    let manifest: Vec<Value> =
        serde_json::from_slice(&std::fs::read(output.join("manifest.json")).unwrap()).unwrap();
    let mut fixtures = Vec::new();
    for app in manifest {
        let source = app["source"].as_str().unwrap();
        let compiler = sources::compiler(source).unwrap();
        let runtime = compiler.runtime().unwrap();
        let initial = runtime.invoke(source, "init", &[]).unwrap();
        let view = runtime
            .invoke(source, "view", std::slice::from_ref(&initial))
            .unwrap();
        let mut cases = Vec::new();
        for node in view["nodes"].as_array().unwrap() {
            let props = node["props"].as_array().unwrap();
            let property = |tag: &str| {
                props
                    .iter()
                    .find(|p| p["tag"] == tag)
                    .map(|p| p["values"][0].clone())
            };
            if property("Disabled") == Some(json!(true)) || property("Busy") == Some(json!(true)) {
                continue;
            }
            for prop in props {
                let (tag, payload) = match prop["tag"].as_str().unwrap() {
                    "Activate" => ("Activate", None),
                    "Toggle" => (
                        "Toggle",
                        Some(json!(
                            property("Checked").or_else(|| property("Selected"))
                                != Some(json!(true))
                        )),
                    ),
                    "Input" if property("ReadOnly") != Some(json!(true)) => (
                        "Input",
                        Some(json!(if node["kind"]["tag"] == "NumberInput" {
                            "9"
                        } else {
                            "Hello 🦀"
                        })),
                    ),
                    "Change" => (
                        "Select",
                        Some(property("Option").unwrap_or_else(|| json!("73"))),
                    ),
                    _ => continue,
                };
                let mut values = vec![prop["values"][0].clone()];
                values.extend(payload);
                let event = json!({"tag":tag,"values":values});
                let model = runtime
                    .invoke(source, "update", &[initial.clone(), event.clone()])
                    .unwrap();
                let next = runtime
                    .invoke(source, "view", std::slice::from_ref(&model))
                    .unwrap();
                cases.push(json!({"event":event,"model":model,"view":next}));
            }
        }
        fixtures.push(json!({"slug":app["slug"],"module":app["module"],"initial":initial,"view":view,"cases":cases}));
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("ubi-ui/tools/verify_web.mjs");
    let fixture_path = output.join("fixtures.json");
    std::fs::write(&fixture_path, serde_json::to_vec(&fixtures).unwrap()).unwrap();
    let result = Command::new("node")
        .arg(script)
        .arg(&output)
        .arg(&fixture_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Node is required to verify the browser backend");
    println!("{}", String::from_utf8_lossy(&result.stdout));
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
