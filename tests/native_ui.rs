#![cfg(feature = "native-ui")]
use serde_json::json;
use ubi_lang::{ui::NativeApplication, Compiler};
use ubi_ui_platform::{Application, Event};

fn gallery() -> NativeApplication {
    let mut compiler = Compiler::new();
    for path in [
        "examples/gallery.ubi",
        "packages/ubi-ui/core.ubi",
        "packages/ubi-ui/components.ubi",
    ] {
        compiler
            .add_source(
                path,
                std::fs::read(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("ubi-ui")
                        .join(path),
                )
                .unwrap(),
            )
            .unwrap();
    }
    NativeApplication::new(compiler.runtime().unwrap(), "examples/gallery.ubi").unwrap()
}

#[test]
fn native_gallery_resizes_without_horizontal_control_overflow() {
    struct Measure;
    impl ubi_ui_platform::TextMeasure for Measure {
        fn measure(&mut self, text: &str, size: f32, max: f32) -> (f32, f32) {
            let width = text.chars().count() as f32 * size * 0.5;
            let lines = if max.is_finite() {
                (width / max.max(1.0)).ceil().max(1.0)
            } else {
                1.0
            };
            (width.min(max), size * 1.4 * lines)
        }
    }
    let app = gallery();
    let mut layout = ubi_ui_platform::Layout::default();
    for width in [420.0, 1100.0, 640.0] {
        let scene = layout
            .compute(app.view(), width, 1000.0, &mut Measure)
            .unwrap();
        for (key, node) in &app.view().nodes {
            if node.kind.focusable() {
                let b = scene.boxes[key].bounds;
                assert!(
                    b.x >= 0.0 && b.x + b.width <= width + 0.5,
                    "{key} overflow at {width}: {b:?}"
                );
            }
        }
    }
}
fn event(key: &str, tag: &str, message: &str, extra: Option<serde_json::Value>) -> Event {
    let mut values = vec![json!({"tag":message,"values":[]})];
    if let Some(extra) = extra {
        values.push(extra);
    }
    Event {
        key: key.into(),
        value: json!({"tag":tag,"values":values}),
    }
}
#[test]
fn real_ubi_gallery_handles_controlled_events_and_derived_views() {
    let mut app = gallery();
    assert!(app.view().nodes.len() > 45);
    let changes = app
        .dispatch(&event("increment", "Activate", "Increment", None))
        .unwrap();
    assert_eq!(app.model()["count"], 1);
    assert_eq!(changes.updated, ["count"]);
    app.dispatch(&event("name", "Input", "Name", Some(json!("árvíz 🦀"))))
        .unwrap();
    assert_eq!(app.view().nodes["name"].value, "árvíz 🦀");
    app.dispatch(&event("level", "Select", "Level", Some(json!("98"))))
        .unwrap();
    assert_eq!(app.view().nodes["progress"].progress, 98);
    app.dispatch(&event("theme", "Toggle", "Theme", Some(json!(true))))
        .unwrap();
    assert_eq!(app.view().nodes["app"].theme.as_deref(), Some("Dark"));
    app.dispatch(&event("enabled", "Toggle", "Enable", Some(json!(false))))
        .unwrap();
    assert!(app.view().nodes["increment"].disabled);
    assert!(app
        .dispatch(&event("increment", "Activate", "Increment", None))
        .is_err());
    assert!(app
        .dispatch(&event("reset", "Activate", "Increment", None))
        .is_err());
    app.dispatch(&event("reset", "Activate", "Reset", None))
        .unwrap();
    let count = app.view_evaluations;
    assert!(app
        .dispatch(&event("reset", "Activate", "Reset", None))
        .unwrap()
        .is_empty());
    assert_eq!(app.view_evaluations, count);
}

#[test]
fn keyboard_activation_runs_the_real_ubi_reducer_and_updates_only_counter() {
    let mut app = gallery();
    let mut input = ubi_ui_platform::Input::default();
    for _ in 0..20 {
        input.key(app.view(), ubi_ui_platform::UiKey::Tab, true, false);
        if input.focused.as_deref() == Some("increment") {
            break;
        }
    }
    assert_eq!(input.focused.as_deref(), Some("increment"));
    assert!(input
        .key(app.view(), ubi_ui_platform::UiKey::Enter, true, false)
        .is_none());
    let event = input
        .key(app.view(), ubi_ui_platform::UiKey::Enter, false, false)
        .unwrap();
    assert_eq!(app.dispatch(&event).unwrap().updated, ["count"]);
    assert_eq!(app.model()["count"], 1);
    assert_eq!(app.view().nodes["count"].content, "Button activations: 1");
}
#[test]
fn invalid_view_rolls_back_model_tree_and_revision() {
    let mut compiler = Compiler::new();
    compiler
        .add_source(
            "core.ubi",
            include_str!("../ubi-ui/packages/ubi-ui/core.ubi"),
        )
        .unwrap();
    compiler.add_source("main.ubi",r#"
import { View, Event, Kind, Prop, node } from "./core.ubi";
export enum Message { Tap }
export fn init() -> int { 0 }
export fn update(model: int, event: Event<Message>) -> int { model + 1 }
export fn view(model: int) -> View<Message> {
    node(if (model == 2) { "" } else { "root" }, Kind.Button, [Prop.Content(toString(model)), Prop.Activate(Message.Tap)])
}
"#).unwrap();
    let mut app = NativeApplication::new(compiler.runtime().unwrap(), "main.ubi").unwrap();
    app.dispatch(&event("root", "Activate", "Tap", None))
        .unwrap();
    let revision = app.revision();
    assert!(app
        .dispatch(&event("root", "Activate", "Tap", None))
        .is_err());
    assert_eq!(app.model(), &json!(1));
    assert_eq!(app.revision(), revision);
    assert_eq!(app.view().nodes["root"].content, "1");
    assert!(app
        .dispatch(&event("root", "Activate", "Tap", Some(json!(1))))
        .is_err());
}
