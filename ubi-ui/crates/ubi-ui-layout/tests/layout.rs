use serde_json::{json, Value};
use ubi_ui_core::View;
use ubi_ui_layout::{Layout, TextMeasure};
struct Text;
impl TextMeasure for Text {
    fn measure(&mut self, s: &str, size: f32, _: f32) -> (f32, f32) {
        (s.chars().count() as f32 * size * 0.5, size * 1.5)
    }
}

#[test]
fn wrapped_labels_and_alerts_reserve_their_painted_text_height() {
    struct WrappedText;
    impl TextMeasure for WrappedText {
        fn measure(&mut self, s: &str, _: f32, max: f32) -> (f32, f32) {
            let width = s.chars().count() as f32 * 8.0;
            (width.min(max), (width / max).ceil().max(1.0) * 20.0)
        }
    }
    for (kind, inset, padding) in [("Label", 0.0, 0.0), ("Alert", 58.0, 24.0)] {
        let content = "a ".repeat(60);
        let v = view(vec![
            node("root", "Column", json!([]), &["wrapped", "next"]),
            node(
                "wrapped",
                kind,
                json!([{"tag":"Content","values":[content]}]),
                &[],
            ),
            node("next", "Button", json!([]), &[]),
        ]);
        let s = Layout::default()
            .compute(&v, 160.0, 1000.0, &mut WrappedText)
            .unwrap();
        let bounds = s.boxes["wrapped"].bounds;
        let (_, text_height) = WrappedText.measure(&content, 14.0, bounds.width - inset);
        assert!(
            bounds.height >= text_height + padding,
            "{kind} clipped wrapped text"
        );
        assert!(s.boxes["next"].bounds.y >= bounds.y + text_height + padding);
    }
}

#[test]
fn text_reveal_and_tooltip_reserve_their_painted_text_bounds() {
    struct WrappedText;
    impl TextMeasure for WrappedText {
        fn measure(&mut self, text: &str, _: f32, max: f32) -> (f32, f32) {
            let width = text.chars().count() as f32 * 8.0;
            (width.min(max), (width / max).ceil().max(1.0) * 20.0)
        }
    }

    let content = "word ".repeat(50);
    let reveal = view(vec![
        node("root", "Column", json!([]), &["reveal", "next"]),
        node(
            "reveal",
            "EffectSurface",
            json!([
                {"tag":"Effect","values":[{"tag":"TextReveal","values":[]}]},
                {"tag":"Content","values":[content]}
            ]),
            &[],
        ),
        node("next", "Button", json!([]), &[]),
    ]);
    let scene = Layout::default()
        .compute(&reveal, 160.0, 1000.0, &mut WrappedText)
        .unwrap();
    let reveal_bounds = scene.boxes["reveal"].bounds;
    assert!(
        reveal_bounds.height > 0.0,
        "TextReveal collapsed to zero height"
    );
    assert!(
        scene.boxes["next"].bounds.y >= reveal_bounds.y + reveal_bounds.height,
        "following content overlaps TextReveal"
    );

    let tooltip = view(vec![
        node("root", "Column", json!([]), &["trigger", "tip"]),
        node("trigger", "Button", json!([]), &[]),
        node(
            "tip",
            "Overlay",
            json!([
                {"tag":"OverlayKind","values":[{"tag":"Tooltip","values":[]}]},
                {"tag":"Open","values":[true]},
                {"tag":"Anchor","values":["trigger"]},
                {"tag":"Label","values":["A helpful tooltip message"]},
                {"tag":"Content","values":["A helpful tooltip message"]}
            ]),
            &[],
        ),
    ]);
    let scene = Layout::default()
        .compute(&tooltip, 240.0, 180.0, &mut WrappedText)
        .unwrap();
    let tip_bounds = scene.boxes["tip"].bounds;
    assert!(tip_bounds.width > 32.0, "Tooltip collapsed to zero width");
    assert!(tip_bounds.height > 16.0, "Tooltip collapsed to zero height");
}

#[test]
fn replacing_container_or_child_kind_relinks_retained_layout_ids() {
    let first = view(vec![
        node("root", "Row", json!([]), &["a", "b"]),
        node("a", "Button", json!([{"tag":"Width","values":[100]}]), &[]),
        node("b", "Button", json!([{"tag":"Width","values":[100]}]), &[]),
    ]);
    let mut l = Layout::default();
    let s = l.compute(&first, 300.0, 200.0, &mut Text).unwrap();
    assert_eq!(s.boxes["b"].bounds.x, 112.0);
    let mut next = first.clone();
    next.nodes.get_mut("root").unwrap().kind = ubi_ui_core::Kind::Column;
    let s = l.compute(&next, 300.0, 200.0, &mut Text).unwrap();
    assert_eq!(s.boxes["b"].bounds.y, 52.0);
    next.nodes.get_mut("a").unwrap().kind = ubi_ui_core::Kind::Checkbox;
    let s = l.compute(&next, 300.0, 200.0, &mut Text).unwrap();
    assert_eq!(s.boxes["a"].bounds.height, 40.0);
    assert_eq!(s.boxes["b"].bounds.y, 52.0);
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
    assert!(l.reveal(&v, &s, "c"));
    let focused = l.compute(&v, 300.0, 200.0, &mut Text).unwrap();
    assert_eq!(
        focused.boxes["c"]
            .bounds
            .intersect(focused.boxes["c"].clip)
            .height,
        40.0
    );
    assert!(l.scroll_at(&v, &focused, 10.0, 10.0, -1000.0));
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
