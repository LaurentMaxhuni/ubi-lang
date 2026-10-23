#![cfg(feature = "native-ui")]

use serde_json::json;
use ubi_lang::ui::NativeApplication;
use ubi_ui_platform::{Application, Event, Input, Layout, TextMeasure, UiKey};

#[path = "../ubi-ui/tools/sources.rs"]
mod sources;

fn example(slug: &str) -> NativeApplication {
    let source = format!("examples/components/{slug}.ubi");
    let compiler = sources::compiler(&source).unwrap();
    NativeApplication::new(compiler.runtime().unwrap(), source).unwrap()
}

fn activate(app: &mut NativeApplication, key: &str) {
    let message = app.view().nodes[key].activate.clone().unwrap();
    app.dispatch(&Event {
        key: key.into(),
        value: json!({"tag":"Activate","values":[message]}),
    })
    .unwrap();
}

struct Text;
impl TextMeasure for Text {
    fn measure(&mut self, text: &str, size: f32, max_width: f32) -> (f32, f32) {
        ((text.len() as f32 * size * 0.5).min(max_width), size * 1.5)
    }
}

#[test]
fn compiled_dialog_enters_modal_focus_and_restores_the_source_trigger() {
    let mut app = example("dialog");
    let mut input = Input::default();
    input.focused = Some("open".into());
    input.sync(app.view());
    activate(&mut app, "open");
    input.sync(app.view());
    assert_eq!(input.focused.as_deref(), Some("close"));
    let close = input.key(app.view(), UiKey::Escape, true, false).unwrap();
    app.dispatch(&close).unwrap();
    input.sync(app.view());
    assert!(!app.view().nodes["dialog"].open);
    assert_eq!(input.focused.as_deref(), Some("open"));
}

#[test]
fn compiled_select_rejects_disabled_options_and_commits_typed_selection() {
    let mut app = example("select");
    let mut input = Input::default();
    input.focused = Some("frequency-trigger".into());
    input.sync(app.view());
    activate(&mut app, "frequency-trigger");
    input.sync(app.view());
    assert_eq!(input.focused.as_deref(), Some("frequency-option-weekly"));
    let before = app.model().clone();
    for (key, value, valid) in [
        ("frequency-option-never", "never", false),
        ("frequency-option-weekly", "weekly", true),
    ] {
        let message = app.view().nodes[key].change.clone().unwrap();
        let result = app.dispatch(&Event {
            key: key.into(),
            value: json!({"tag":"Select","values":[message,value]}),
        });
        assert_eq!(result.is_ok(), valid);
        if !valid {
            assert_eq!(app.model(), &before);
        }
    }
    assert!(app.view().nodes["frequency-option-weekly"].checked);
    assert!(!app.view().nodes["frequency-list"].open);
    input.sync(app.view());
    assert_eq!(input.focused.as_deref(), Some("frequency-trigger"));
}

#[test]
fn compiled_host_requests_use_registered_messages_and_reject_forged_actions() {
    for (slug, key, binding) in [
        ("tooltip", "help-tip", "request"),
        ("rotating-text", "tagline-phrases", "advance"),
    ] {
        let mut app = example(slug);
        let node = &app.view().nodes[key];
        let message = if binding == "request" {
            node.open_request.clone().unwrap()
        } else {
            node.advance.clone().unwrap()
        };
        let before = app.model().clone();
        assert!(app
            .dispatch(&Event {
                key: key.into(),
                value: json!({"tag":"Activate","values":[{"tag":"Forged","values":[]}]}),
            })
            .is_err());
        assert_eq!(app.model(), &before);
        app.dispatch(&Event {
            key: key.into(),
            value: json!({"tag":"Activate","values":[message]}),
        })
        .unwrap();
        assert_ne!(app.model(), &before, "{slug}");
    }
}

#[test]
fn compiled_combobox_navigates_filtered_options_without_moving_query_focus() {
    let mut app = example("combobox");
    let mut input = Input::default();
    input.focused = Some("theme-input".into());
    input.sync(app.view());
    let query = input.text(app.view(), "paper").unwrap();
    app.dispatch(&query).unwrap();
    input.sync(app.view());
    let caret = input.caret;
    assert!(input.key(app.view(), UiKey::Down, true, false).is_none());
    assert_eq!(input.focused.as_deref(), Some("theme-input"));
    assert_eq!(input.caret, caret);
    assert!(input.key(app.view(), UiKey::Enter, true, false).is_none());
    let choose = input.key(app.view(), UiKey::Enter, false, false).unwrap();
    assert_eq!(choose.key, "theme-option-paper");
    app.dispatch(&choose).unwrap();
    input.sync(app.view());
    assert_eq!(app.model()["selected"], "paper");
    assert_eq!(input.focused.as_deref(), Some("theme-input"));
}

#[test]
fn compiled_carousel_excludes_inactive_slides_from_geometry_and_semantics() {
    let mut app = example("carousel");
    let mut layout = Layout::default();
    for selected in 0..3 {
        let scene = layout.compute(app.view(), 640.0, 480.0, &mut Text).unwrap();
        let accessible = scene.accessibility(app.view(), None);
        for (index, key) in ["item-a", "item-b", "item-c"].iter().enumerate() {
            assert_eq!(scene.boxes.contains_key(*key), index == selected, "{key}");
            assert_eq!(
                accessible.iter().any(|node| node.key == *key),
                index == selected,
                "{key}"
            );
        }
        if selected < 2 {
            activate(&mut app, "highlights-next");
        }
    }
    assert!(app.view().nodes["highlights-next"].disabled);
}
