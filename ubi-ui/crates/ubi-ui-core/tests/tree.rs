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
