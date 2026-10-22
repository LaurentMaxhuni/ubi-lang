use serde_json::json;
use ubi_lang::Compiler;

fn compiler(source: &str) -> Compiler {
    let mut compiler = Compiler::new();
    compiler.add_source("main.ubi", source).unwrap();
    compiler
}

#[test]
fn native_calls_validate_and_round_trip_typed_aggregates() {
    let runtime = compiler(
        r#"
export enum Event { Rename(string), Toggle(bool) }
export record Model { name: string, enabled: bool }
export fn initial() -> Model { Model { name: "Ubi", enabled: false } }
export fn update(model: Model, event: Event) -> Model {
    match (event) {
        Event.Rename(name) => Model { ...model, name: name },
        Event.Toggle(value) => Model { ...model, enabled: value }
    }
}
fn private() -> int { 42 }
"#,
    )
    .runtime()
    .unwrap();
    let model = runtime.invoke("main.ubi", "initial", &[]).unwrap();
    assert_eq!(model, json!({"name":"Ubi", "enabled":false}));
    let changed = runtime
        .invoke(
            "main.ubi",
            "update",
            &[model.clone(), json!({"tag":"Rename","values":["árvíz 🦀"]})],
        )
        .unwrap();
    assert_eq!(changed["name"], "árvíz 🦀");
    assert_eq!(model["name"], "Ubi");
    for args in [
        vec![model.clone()],
        vec![
            json!({"name":"x","enabled":0}),
            json!({"tag":"Toggle","values":[true]}),
        ],
        vec![model, json!({"tag":"Toggle","values":[true],"extra":0})],
    ] {
        assert!(runtime.invoke("main.ubi", "update", &args).is_err());
    }
    assert!(runtime.invoke("main.ubi", "private", &[]).is_err());
    assert!(runtime.invoke("main.ubi", "missing", &[]).is_err());
}

#[test]
fn native_runtime_rejects_invalid_programs_and_unbounded_calls() {
    assert!(compiler("export fn main() -> int { true }")
        .runtime()
        .is_err());
    let runtime = compiler("export fn main() -> int { while (true) {} 0 }")
        .runtime()
        .unwrap();
    let error = runtime.invoke("main.ubi", "main", &[]).unwrap_err();
    assert!(error.to_string().contains("UBI-R"), "{error}");
}

#[test]
fn native_runtime_preserves_numeric_faults_and_float_host_encoding() {
    let runtime = compiler(
        "export fn divide(n: int) -> int { 10 / n } export fn echo(n: float) -> float { n }",
    )
    .runtime()
    .unwrap();
    assert!(runtime
        .invoke("main.ubi", "divide", &[json!(0)])
        .unwrap_err()
        .to_string()
        .contains("UBI-R"));
    for value in [
        json!("NaN"),
        json!("+Infinity"),
        json!("-Infinity"),
        json!("-0"),
        json!(3.5),
    ] {
        assert_eq!(
            runtime
                .invoke("main.ubi", "echo", std::slice::from_ref(&value))
                .unwrap(),
            value
        );
    }
}
