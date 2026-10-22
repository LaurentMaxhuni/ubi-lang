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
