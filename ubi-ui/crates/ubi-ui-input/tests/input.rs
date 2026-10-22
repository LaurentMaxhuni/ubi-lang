use serde_json::{json, Value};
use ubi_ui_core::View;
use ubi_ui_input::{Input, Key};
use ubi_ui_layout::{Layout, TextMeasure};
struct Text;
impl TextMeasure for Text {
    fn measure(&mut self, _: &str, _: f32, _: f32) -> (f32, f32) {
        (20.0, 20.0)
    }
}

#[test]
fn queued_pointer_events_tolerate_a_scene_from_before_unmount() {
    let view = fixture();
    let scene = Layout::default()
        .compute(&view, 300.0, 300.0, &mut Text)
        .unwrap();
    let mut next = view.clone();
    next.nodes.remove("button");
    next.order.retain(|k| k != "button");
    next.nodes
        .get_mut("root")
        .unwrap()
        .children
        .retain(|k| k != "button");
    assert_eq!(Input::hit(&next, &scene, 10.0, 10.0), None);
}
fn fixture() -> View {
    let n = |key: &str, kind: &str, props: Value, children: Vec<&str>| json!({"key":key,"kind":{"tag":kind,"values":[]},"props":props,"children":children});
    View::decode(json!({"schemaVersion":1,"root":"root","nodes":[n("root","Column",json!([]),vec!["button","disabled","toggle","input","slider"]),n("button","Button",json!([{"tag":"Activate","values":[{"tag":"Save","values":[]}]}]),vec![]),n("disabled","Button",json!([{"tag":"Disabled","values":[true]}]),vec![]),n("toggle","Checkbox",json!([{"tag":"Toggle","values":[{"tag":"Enable","values":[]}]}]),vec![]),n("input","TextInput",json!([{"tag":"Value","values":["ár🦀"]},{"tag":"Input","values":[{"tag":"Name","values":[]}]}]),vec![]),n("slider","Slider",json!([{"tag":"Change","values":[{"tag":"Level","values":[]}]}]),vec![])]})).unwrap()
}
#[test]
fn keyboard_order_activation_disabled_and_capture_cleanup() {
    let v = fixture();
    let mut input = Input::default();
    input.key(&v, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("button"));
    assert!(input.key(&v, Key::Space, true, false).is_none());
    assert!(input.pressed("button"));
    assert_eq!(
        input.key(&v, Key::Space, false, false).unwrap().value["values"][0]["tag"],
        "Save"
    );
    input.key(&v, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("toggle"));
    input.key(&v, Key::Enter, true, false);
    assert_eq!(
        input.key(&v, Key::Enter, false, false).unwrap().value["values"][1],
        true
    );
    let scene = Layout::default()
        .compute(&v, 300.0, 300.0, &mut Text)
        .unwrap();
    input.pointer_move(&v, &scene, 10.0, 10.0);
    input.pointer_down(&v, &scene);
    assert!(input.captured.is_some());
    input.pointer_move(&v, &scene, 500.0, 500.0);
    assert!(input.pointer_up(&v, &scene).is_none());
    input.captured = Some("gone".into());
    input.focused = Some("disabled".into());
    input.sync(&v);
    assert!(input.captured.is_none());
    assert!(input.focused.is_none());
}
#[test]
fn unicode_editing_and_slider_keyboard_emit_typed_controlled_events() {
    let v = fixture();
    let mut input = Input::default();
    for _ in 0..3 {
        input.key(&v, Key::Tab, true, false);
    }
    assert_eq!(input.focused.as_deref(), Some("input"));
    assert_eq!(input.caret, 3);
    let event = input.key(&v, Key::Backspace, true, false).unwrap();
    assert_eq!(event.value["values"][1], "ár");
    input.select_all = true;
    assert_eq!(input.text(&v, "你好").unwrap().value["values"][1], "你好");
    assert!(input.text(&v, "\n").is_none());
    input.key(&v, Key::Tab, true, false);
    assert_eq!(
        input.key(&v, Key::End, true, false).unwrap().value["values"][1],
        "100"
    );
}

fn controls(items: &[(&str, &str, Value)]) -> View {
    let children: Vec<_> = items.iter().map(|(key, _, _)| key).collect();
    let mut nodes = vec![
        json!({"key":"root","kind":{"tag":"Column","values":[]},"props":[],"children":children}),
    ];
    nodes.extend(items.iter().map(|(key, kind, props)| {
        json!({"key":key,"kind":{"tag":kind,"values":[]},"props":props,"children":[]})
    }));
    View::decode(json!({"schemaVersion":1,"root":"root","nodes":nodes})).unwrap()
}

fn binding(tag: &str) -> Value {
    json!({"tag":tag,"values":[{"tag":"Changed","values":[7]}]})
}

#[test]
fn new_controls_dispatch_typed_messages_with_pointer_and_keyboard() {
    for (kind, property, expected) in [
        (
            "Radio",
            "Change",
            json!({"tag":"Select","values":[{"tag":"Changed","values":[7]},"choice"]}),
        ),
        (
            "Tab",
            "Change",
            json!({"tag":"Select","values":[{"tag":"Changed","values":[7]},"choice"]}),
        ),
        (
            "ToggleItem",
            "Change",
            json!({"tag":"Select","values":[{"tag":"Changed","values":[7]},"choice"]}),
        ),
        (
            "Toggle",
            "Toggle",
            json!({"tag":"Toggle","values":[{"tag":"Changed","values":[7]},false]}),
        ),
        (
            "Link",
            "Activate",
            json!({"tag":"Activate","values":[{"tag":"Changed","values":[7]}]}),
        ),
    ] {
        let view = controls(&[(
            "control",
            kind,
            json!([binding(property),{"tag":"Option","values":["choice"]},{"tag":"Selected","values":[true]}]),
        )]);
        let scene = Layout::default()
            .compute(&view, 300.0, 300.0, &mut Text)
            .unwrap();
        let bounds = scene.boxes["control"].bounds;
        let mut input = Input::default();
        input.pointer_move(&view, &scene, bounds.x + 1.0, bounds.y + 1.0);
        assert!(input.pointer_down(&view, &scene).is_none(), "{kind}");
        assert_eq!(
            input.pointer_up(&view, &scene).unwrap().value,
            expected,
            "{kind}"
        );
        assert!(input.key(&view, Key::Space, true, false).is_none());
        assert!(input.key(&view, Key::Space, true, false).is_none());
        assert_eq!(
            input.key(&view, Key::Space, false, false).unwrap().value,
            expected
        );
        assert!(input.key(&view, Key::Space, false, false).is_none());
    }
}

#[test]
fn roving_groups_skip_disabled_items_and_remain_separate_by_kind_and_group() {
    let item = |group: &str, option: &str, selected: bool| {
        json!([
            binding("Change"),{"tag":"Group","values":[group]},
            {"tag":"Option","values":[option]},{"tag":"Selected","values":[selected]}
        ])
    };
    let mut disabled = item("shared", "disabled", true);
    disabled
        .as_array_mut()
        .unwrap()
        .push(json!({"tag":"Disabled","values":[true]}));
    let view = controls(&[
        ("before", "Button", json!([])),
        ("r1", "Radio", item("shared", "one", false)),
        ("disabled", "Radio", disabled),
        ("r2", "Radio", item("shared", "two", true)),
        ("r3", "Radio", item("shared", "three", false)),
        ("other", "Radio", item("other", "other", false)),
        ("tab1", "Tab", item("shared", "tab-one", false)),
        ("tab2", "Tab", item("shared", "tab-two", true)),
        ("toggle1", "ToggleItem", item("multi", "first", true)),
        ("toggle2", "ToggleItem", item("multi", "second", true)),
        ("after", "Button", json!([])),
    ]);
    let mut input = Input::default();
    input.key(&view, Key::Tab, true, false);
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("r2"));
    assert_eq!(
        input.key(&view, Key::Right, true, false).unwrap().value["values"][1],
        "three"
    );
    assert_eq!(input.focused.as_deref(), Some("r3"));
    assert_eq!(
        input.key(&view, Key::Down, true, false).unwrap().value["values"][1],
        "one"
    );
    assert_eq!(
        input.key(&view, Key::Up, true, false).unwrap().value["values"][1],
        "three"
    );
    assert_eq!(input.key(&view, Key::Home, true, false).unwrap().key, "r1");
    assert_eq!(input.key(&view, Key::End, true, false).unwrap().key, "r3");
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("other"));
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("tab2"));
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("toggle1"));
    for (key, focused) in [
        (Key::Right, "toggle2"),
        (Key::Left, "toggle1"),
        (Key::Down, "toggle2"),
        (Key::Up, "toggle1"),
        (Key::End, "toggle2"),
        (Key::Home, "toggle1"),
        (Key::Right, "toggle2"),
    ] {
        assert!(input.key(&view, key, true, false).is_none());
        assert_eq!(input.focused.as_deref(), Some(focused));
        assert!(view.nodes["toggle1"].checked);
        assert!(view.nodes["toggle2"].checked);
    }
    assert!(input.key(&view, Key::Space, true, false).is_none());
    assert_eq!(
        input.key(&view, Key::Space, false, false).unwrap().value,
        json!({"tag":"Select","values":[{"tag":"Changed","values":[7]},"second"]})
    );
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("after"));
    input.key(&view, Key::Tab, true, true);
    assert_eq!(input.focused.as_deref(), Some("toggle1"));
}

#[test]
fn textarea_preserves_unicode_columns_and_edits_lines_without_trapping_tab() {
    let view = controls(&[
        (
            "area",
            "TextArea",
            json!([binding("Input"),{"tag":"Value","values":["a🦀b\n中\nhello🙂"]}]),
        ),
        ("next", "Button", json!([])),
    ]);
    let mut input = Input::default();
    input.key(&view, Key::Tab, true, false);
    input.caret = 3;
    input.key(&view, Key::Down, true, false);
    assert_eq!(input.caret, 5);
    input.key(&view, Key::Down, true, false);
    assert_eq!(input.caret, 9);
    input.key(&view, Key::Up, true, false);
    assert_eq!(input.caret, 5);
    input.key(&view, Key::Home, true, false);
    assert_eq!(input.caret, 4);
    input.key(&view, Key::End, true, false);
    assert_eq!(input.caret, 5);
    assert_eq!(
        input.key(&view, Key::Enter, true, false).unwrap().value["values"][1],
        "a🦀b\n中\n\nhello🙂"
    );
    input.select_all = true;
    assert_eq!(
        input.text(&view, "你\n🦀").unwrap().value["values"][1],
        "你\n🦀"
    );
    assert_eq!(input.caret, 3);
    for text in ["\t", "\r\n", "\0"] {
        assert!(input.text(&view, text).is_none());
        assert_eq!(input.caret, 3);
    }
    input.key(&view, Key::Tab, true, false);
    assert_eq!(input.focused.as_deref(), Some("next"));
}

#[test]
fn number_input_allows_integer_prefixes_and_clamps_stepping_without_overflow() {
    let mut view = controls(&[(
        "number",
        "NumberInput",
        json!([
            binding("Input"),{"tag":"Value","values":["1"]},
            {"tag":"Min","values":[-2]},{"tag":"Max","values":[2]},{"tag":"Step","values":[2]}
        ]),
    )]);
    let mut input = Input::default();
    input.key(&view, Key::Tab, true, false);
    assert_eq!(
        input.key(&view, Key::Up, true, false).unwrap().value["values"][1],
        "2"
    );
    view.nodes.get_mut("number").unwrap().value = "2".into();
    assert_eq!(
        input.key(&view, Key::Down, true, false).unwrap().value["values"][1],
        "0"
    );
    input.select_all = true;
    assert_eq!(input.text(&view, "-").unwrap().value["values"][1], "-");
    view.nodes.get_mut("number").unwrap().value = "-".into();
    assert_eq!(input.text(&view, "2").unwrap().value["values"][1], "-2");
    for text in ["a", "+1", "1.2", "١", "2147483648", "\n"] {
        input.select_all = true;
        let previous = input.caret;
        assert!(input.text(&view, text).is_none(), "{text}");
        assert_eq!(input.caret, previous);
        assert!(input.select_all);
    }
    input.select_all = true;
    assert_eq!(
        input.key(&view, Key::Backspace, true, false).unwrap().value["values"][1],
        ""
    );
    let node = view.nodes.get_mut("number").unwrap();
    node.value = i32::MAX.to_string();
    node.min = i32::MIN;
    node.max = i32::MAX;
    node.step = i32::MAX;
    assert_eq!(
        input.key(&view, Key::Up, true, false).unwrap().value["values"][1],
        i32::MAX.to_string()
    );
    view.nodes.get_mut("number").unwrap().value = i32::MIN.to_string();
    assert_eq!(
        input.key(&view, Key::Down, true, false).unwrap().value["values"][1],
        i32::MIN.to_string()
    );
    input.caret = 1;
    assert!(input.key(&view, Key::Backspace, true, false).is_none());
    assert_eq!(input.caret, 1);
    view.nodes.get_mut("number").unwrap().step = 0;
    assert!(input.key(&view, Key::Up, true, false).is_none());
    view.nodes.get_mut("number").unwrap().step = 1;
    view.nodes.get_mut("number").unwrap().min = 1;
    view.nodes.get_mut("number").unwrap().max = -1;
    assert!(input.key(&view, Key::Down, true, false).is_none());
}

#[test]
fn readonly_editors_stay_focusable_and_reject_all_mutations() {
    for kind in ["TextInput", "TextArea", "NumberInput"] {
        let view = controls(&[(
            "editor",
            kind,
            json!([
                binding("Input"),{"tag":"Value","values":["12"]},{"tag":"ReadOnly","values":[true]}
            ]),
        )]);
        let mut input = Input::default();
        input.key(&view, Key::Tab, true, false);
        assert_eq!(input.focused.as_deref(), Some("editor"));
        assert!(input.text(&view, "3").is_none());
        for key in [Key::Backspace, Key::Delete, Key::Enter, Key::Up, Key::Down] {
            assert!(input.key(&view, key, true, false).is_none());
        }
        input.key(&view, Key::Home, true, false);
        assert_eq!(input.caret, 0);
    }
}

#[test]
fn stale_capture_and_focus_and_text_limits_do_not_panic_or_emit() {
    let view = fixture();
    let scene = Layout::default()
        .compute(&view, 300.0, 300.0, &mut Text)
        .unwrap();
    let mut input = Input::default();
    input.captured = Some("unmounted".into());
    input.focused = Some("unmounted".into());
    assert!(input.pointer_move(&view, &scene, 10.0, 10.0).is_none());
    assert!(input.pointer_up(&view, &scene).is_none());
    assert!(input.key(&view, Key::Enter, true, false).is_none());
    let view = controls(&[("area", "TextArea", json!([binding("Input")]))]);
    input.focused = None;
    input.key(&view, Key::Tab, true, false);
    assert!(input.text(&view, &"🦀".repeat(4096)).is_some());
    input.select_all = true;
    assert!(input.text(&view, &"🦀".repeat(4097)).is_none());
    assert!(input.select_all);
}
