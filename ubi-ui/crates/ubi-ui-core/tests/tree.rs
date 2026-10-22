use serde_json::{json, Value};
use ubi_ui_core::{Rect, Tree, View};
fn node(key: &str, kind: &str, children: &[&str]) -> Value {
    json!({"key":key,"kind":{"tag":kind,"values":[]},"props":[],"children":children})
}
fn view(nodes: Vec<Value>) -> Result<View, ubi_ui_core::Error> {
    View::decode(json!({"schemaVersion":1,"root":"root","nodes":nodes}))
}
#[test]
fn rejects_cycles_aliases_unreachable_unknown_and_oversized_values() {
    for nodes in [
        vec![node("root", "Column", &["root"])],
        vec![
            node("root", "Column", &["a", "a"]),
            node("a", "Button", &[]),
        ],
        vec![node("root", "Column", &[]), node("a", "Text", &[])],
        vec![node("root", "Unknown", &[])],
        vec![node("root", "Text", &["a"]), node("a", "Text", &[])],
    ] {
        assert!(view(nodes).is_err());
    }
    let mut n = node("root", "Button", &[]);
    n["props"] = json!([{"tag":"Width","values":[-1]}]);
    assert!(view(vec![n.clone()]).is_err());
    n["props"] = json!([{"tag":"Absolute","values":[i64::MIN,0]}]);
    assert!(view(vec![n.clone()]).is_err());
    n["props"] = json!([{"tag":"Disabled","values":[true]},{"tag":"Disabled","values":[false]}]);
    assert!(view(vec![n]).is_err());
}
#[test]
fn keyed_reconciliation_reports_exact_lifecycle_and_skips_equal_views() {
    let first = view(vec![
        node("root", "Column", &["a", "b"]),
        node("a", "Button", &[]),
        node("b", "Text", &[]),
    ])
    .unwrap();
    let mut tree = Tree::new(first.clone());
    assert!(tree.reconcile(first).is_empty());
    assert_eq!(tree.revision, 1);
    let next = view(vec![
        node("root", "Column", &["b", "c"]),
        node("b", "Button", &[]),
        node("c", "Text", &[]),
    ])
    .unwrap();
    let delta = tree.reconcile(next);
    assert_eq!(delta.unmounted, ["a", "b"]);
    assert_eq!(delta.mounted, ["b", "c"]);
    assert_eq!(delta.updated, ["root"]);
    assert_eq!(tree.revision, 2);
}
#[test]
fn accessibility_redacts_passwords_and_clips_geometry() {
    let mut n = node("root", "PasswordInput", &[]);
    n["props"] = json!([{"tag":"Value","values":["secret"]},{"tag":"Label","values":["Password"]}]);
    let view = view(vec![n]).unwrap();
    let a = view.nodes["root"].accessible(Rect::default(), true);
    assert_eq!(a.name, "Password");
    assert!(a.value.is_empty());
    assert!(a.focused);
    let mut slider = view.nodes["root"].clone();
    slider.kind = ubi_ui_core::Kind::Slider;
    slider.progress = 73;
    assert_eq!(slider.accessible(Rect::default(), false).value, "73");
    let rect = Rect {
        x: 10.0,
        y: 10.0,
        width: 30.0,
        height: 20.0,
    }
    .intersect(Rect {
        x: 0.0,
        y: 0.0,
        width: 20.0,
        height: 20.0,
    });
    assert_eq!(rect.width, 10.0);
    assert!(!rect.contains(20.0, 15.0));
}

#[test]
fn new_control_properties_validate_ranges_roles_and_semantic_states() {
    let mut n = node("root", "NumberInput", &[]);
    n["props"] = json!([{"tag":"Min","values":[-10]},{"tag":"Max","values":[20]},{"tag":"Step","values":[2]},{"tag":"ReadOnly","values":[true]}]);
    let v = view(vec![n.clone()]).unwrap();
    assert_eq!(v.nodes["root"].min, -10);
    assert!(v.nodes["root"].accessible(Rect::default(), false).read_only);
    n["props"][2]["values"][0] = json!(0);
    assert!(view(vec![n.clone()]).is_err());
    n["props"] = json!([{"tag":"Min","values":[20]},{"tag":"Max","values":[-10]}]);
    assert!(view(vec![n.clone()]).is_err());
    n["props"] = json!([{"tag":"Role","values":["invented-role"]}]);
    assert!(view(vec![n.clone()]).is_err());
    n["kind"]["tag"] = json!("Radio");
    n["props"] = json!([{"tag":"Selected","values":[true]},{"tag":"Option","values":["weekly"]},{"tag":"Group","values":["schedule"]}]);
    let v = view(vec![n.clone()]).unwrap();
    let a = v.nodes["root"].accessible(Rect::default(), true);
    assert_eq!(a.role, "radio");
    assert_eq!(a.selected, Some(true));
    n["props"] = json!([{"tag":"Selected","values":[true]},{"tag":"Checked","values":[false]}]);
    assert!(view(vec![n]).is_err());
    let mut n = node("root", "Toggle", &[]);
    n["props"] = json!([{"tag":"Expanded","values":[false]}]);
    let v = view(vec![n]).unwrap();
    assert_eq!(
        v.nodes["root"].accessible(Rect::default(), false).expanded,
        Some(false)
    );
}
