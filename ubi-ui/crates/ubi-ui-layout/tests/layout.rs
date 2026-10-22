use serde_json::{json, Value};
use ubi_ui_core::View;
use ubi_ui_layout::{Layout, TextMeasure};
struct Text;
impl TextMeasure for Text {
    fn measure(&mut self, s: &str, size: f32, _: f32) -> (f32, f32) {
        (s.chars().count() as f32 * size * 0.5, size * 1.5)
    }
}
fn node(key: &str, kind: &str, props: Value, children: &[&str]) -> Value {
    json!({"key":key,"kind":{"tag":kind,"values":[]},"props":props,"children":children})
}
fn view(nodes: Vec<Value>) -> View {
    View::decode(json!({"schemaVersion":1,"root":"root","nodes":nodes})).unwrap()
}
#[test]
fn percentage_constraints_absolute_grid_and_cached_resize() {
    let v = view(vec![
        node(
            "root",
            "Grid",
            json!([{"tag":"Columns","values":[2]},{"tag":"Gap","values":[{"tag":"None","values":[]}]}]),
            &["a", "b", "floating"],
        ),
        node(
            "a",
            "Button",
            json!([{"tag":"WidthPercent","values":[100]},{"tag":"MinWidth","values":[100]},{"tag":"MaxWidth","values":[200]}]),
            &[],
        ),
        node("b", "Button", json!([]), &[]),
        node(
            "floating",
            "Button",
            json!([{"tag":"Absolute","values":[10,15]},{"tag":"Width","values":[50]}]),
            &[],
        ),
    ]);
    let mut l = Layout::default();
    let s = l.compute(&v, 400.0, 200.0, &mut Text).unwrap();
    assert_eq!(s.boxes["a"].bounds.width, 200.0);
    assert_eq!(s.boxes["b"].bounds.x, 200.0);
    assert_eq!(s.boxes["floating"].bounds.x, 10.0);
    assert_eq!(s.boxes["floating"].bounds.y, 15.0);
    l.compute(&v, 400.0, 200.0, &mut Text).unwrap();
    assert_eq!(l.computations, 1);
    let s = l.compute(&v, 300.0, 200.0, &mut Text).unwrap();
    assert_eq!(s.boxes["a"].bounds.width, 150.0);
    assert_eq!(l.computations, 2);
    assert!(l.compute(&v, f32::NAN, 200.0, &mut Text).is_err());
}
#[test]
fn scroll_clips_descendants_clamps_and_does_not_relayout() {
    let v = view(vec![
        node("root", "Column", json!([]), &["scroll"]),
        node(
            "scroll",
            "ScrollView",
            json!([{"tag":"Height","values":[60]}]),
            &["a", "b", "c"],
        ),
        node("a", "Button", json!([]), &[]),
        node("b", "Button", json!([]), &[]),
        node("c", "Button", json!([]), &[]),
    ]);
    let mut l = Layout::default();
    let s = l.compute(&v, 300.0, 200.0, &mut Text).unwrap();
    assert_eq!(s.boxes["c"].clip.height, 60.0);
    assert!(l.scroll_at(&v, &s, 10.0, 10.0, 1000.0));
    let s = l.compute(&v, 300.0, 200.0, &mut Text).unwrap();
    assert!(s.boxes["a"].bounds.y < 0.0);
    assert_eq!(l.computations, 1);
    assert!(!l.scroll_at(&v, &s, 10.0, 10.0, 1000.0));
    let next = view(vec![node("root", "Column", json!([]), &[])]);
    l.compute(&next, 300.0, 200.0, &mut Text).unwrap();
    assert_eq!(l.node_count(), 1);
    assert_eq!(l.scroll_offset("scroll"), 0.0);
}
