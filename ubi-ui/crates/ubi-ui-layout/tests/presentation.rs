use serde_json::json;
use std::collections::BTreeMap;
use ubi_ui_core::motion::MotionFrame;
use ubi_ui_core::View;
use ubi_ui_layout::{place_overlay, present, Layout, TextMeasure};

struct Text;
impl TextMeasure for Text {
    fn measure(&mut self, _: &str, _: f32, _: f32) -> (f32, f32) {
        (20.0, 20.0)
    }
}

#[test]
fn presented_scene_composes_nested_transforms_and_keeps_clips() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"View","values":[]},"props":[{"tag":"Width","values":[100]},{"tag":"Height","values":[100]},{"tag":"Clip","values":[true]}],"children":["button"]},
        {"key":"button","kind":{"tag":"Button","values":[]},"props":[{"tag":"Width","values":[40]},{"tag":"Height","values":[40]}],"children":[]}
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 200.0, 120.0, &mut Text)
        .unwrap();
    let channels = BTreeMap::from([
        (
            "root".into(),
            BTreeMap::from([("translate-x".into(), 10.0), ("scale".into(), 2.0)]),
        ),
        (
            "button".into(),
            BTreeMap::from([("translate-x".into(), 5.0), ("scale".into(), 0.5)]),
        ),
    ]);
    let frame = MotionFrame {
        channels,
        ..MotionFrame::default()
    };
    let presented = present(&view, &scene, &frame);
    let button = &presented.scene.boxes["button"];
    assert!((button.bounds.width - 40.0).abs() < 0.001);
    assert!((button.bounds.height - 40.0).abs() < 0.001);
    assert!(button.clip.x >= 0.0 && button.clip.y >= 0.0);
    assert!(
        button.clip.width <= scene.viewport.width && button.clip.height <= scene.viewport.height
    );
    assert!(!button.bounds.contains(
        scene.boxes["button"].bounds.x + 35.0,
        scene.boxes["button"].bounds.y + 20.0
    ));
}

#[test]
fn placement_flips_clamps_and_handles_tiny_viewports() {
    let viewport = ubi_ui_core::Rect {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 120.0,
    };
    let anchor = ubi_ui_core::Rect {
        x: 80.0,
        y: 2.0,
        width: 30.0,
        height: 20.0,
    };
    let placed = place_overlay(anchor, (120.0, 60.0), viewport, ubi_ui_core::Placement::Top);
    assert!(placed.y >= anchor.y + anchor.height);
    assert!(placed.x >= 8.0 && placed.x + placed.width <= 192.0);
    let tiny = place_overlay(
        anchor,
        (90.0, 70.0),
        ubi_ui_core::Rect {
            x: 3.0,
            y: 4.0,
            width: 8.0,
            height: 6.0,
        },
        ubi_ui_core::Placement::Bottom,
    );
    assert!(tiny.x.is_finite() && tiny.y.is_finite() && tiny.width >= 0.0 && tiny.height >= 0.0);
    assert!(tiny.width <= 8.0 && tiny.height <= 6.0);
}

#[test]
fn closed_overlays_leave_layout_and_accessibility_immediately() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["before","modal","after"]},
        {"key":"before","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["Before"]}],"children":[]},
        {"key":"modal","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"Dialog","values":[]}]},{"tag":"Label","values":["Dialog"]},{"tag":"Open","values":[false]}],"children":["inside"]},
        {"key":"inside","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["Inside"]}],"children":[]},
        {"key":"after","kind":{"tag":"Button","values":[]},"props":[{"tag":"Content","values":["After"]}],"children":[]}
    ]})).unwrap();
    let scene = Layout::default()
        .compute(&view, 400.0, 300.0, &mut Text)
        .unwrap();
    assert!(!scene.boxes.contains_key("modal"));
    assert!(!scene.boxes.contains_key("inside"));
    assert!(!scene
        .accessibility(&view, None)
        .iter()
        .any(|node| node.key == "modal" || node.key == "inside"));
}

#[test]
fn context_menu_uses_pointer_point_and_clamps_to_viewport() {
    let mut view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","menu"]},
        {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Project"]}],"children":[]},
        {"key":"menu","kind":{"tag":"Overlay","values":[]},"props":[{"tag":"OverlayKind","values":[{"tag":"ContextMenu","values":[]}]},{"tag":"Label","values":["Project actions"]},{"tag":"Anchor","values":["trigger"]},{"tag":"Open","values":[false]},{"tag":"OpenRequest","values":[{"tag":"Request","values":[]}]}],"children":[]}
    ]})).unwrap();
    let mut layout = Layout::default();
    layout.compute(&view, 240.0, 160.0, &mut Text).unwrap();
    layout.set_context_point("menu", Some((220.0, 150.0)));
    view.nodes.get_mut("menu").unwrap().open = true;
    let scene = layout.compute(&view, 240.0, 160.0, &mut Text).unwrap();
    let menu = scene.boxes["menu"].bounds;
    assert!(menu.x >= 8.0 && menu.y >= 8.0);
    assert!(menu.x + menu.width <= 232.0 && menu.y + menu.height <= 152.0);
    assert_ne!(menu.x, scene.boxes["trigger"].bounds.x);
    assert_eq!(scene.context_points["menu"], (220.0, 150.0));
}

#[test]
fn disclosure_motion_clips_content_and_moves_following_siblings_together() {
    let view = View::decode(json!({"schemaVersion":1,"root":"root","nodes":[
        {"key":"root","kind":{"tag":"Column","values":[]},"props":[],"children":["panel","footer"]},
        {"key":"panel","kind":{"tag":"Column","values":[]},"props":[{"tag":"Disclosure","values":[true]}],"children":["body"]},
        {"key":"body","kind":{"tag":"Text","values":[]},"props":[{"tag":"Content","values":["Panel body"]}],"children":[]},
        {"key":"footer","kind":{"tag":"Text","values":[]},"props":[{"tag":"Content","values":["After panel"]}],"children":[]}
    ]})).unwrap();
    let mut layout = Layout::default();
    let base = layout.compute(&view, 300.0, 200.0, &mut Text).unwrap();
    let full_height = base.boxes["panel"].bounds.height;
    let footer_y = base.boxes["footer"].bounds.y;
    let frame = MotionFrame {
        channels: BTreeMap::from([(
            "panel".into(),
            BTreeMap::from([("disclosure-progress".into(), 0.5)]),
        )]),
        ..MotionFrame::default()
    };
    let shown = present(&view, &base, &frame);
    assert!((shown.scene.boxes["panel"].bounds.height - full_height * 0.5).abs() < 0.001);
    assert!((shown.scene.boxes["body"].clip.height - full_height * 0.5).abs() < 0.001);
    assert!((shown.scene.boxes["footer"].bounds.y - (footer_y - full_height * 0.5)).abs() < 0.001);
}

#[test]
fn animated_list_layout_channels_move_reordered_item_subtrees() {
    let make_view = |children: [&str; 2]| {
        View::decode(json!({"schemaVersion":1,"root":"list","nodes":[
            {"key":"list","kind":{"tag":"EffectSurface","values":[]},"props":[{"tag":"Effect","values":[{"tag":"AnimatedList","values":[]}]}],"children":[children[0],children[1]]},
            {"key":"a","kind":{"tag":"Column","values":[]},"props":[{"tag":"Height","values":[40]}],"children":["a-label"]},
            {"key":"a-label","kind":{"tag":"Text","values":[]},"props":[{"tag":"Content","values":["A"]}],"children":[]},
            {"key":"b","kind":{"tag":"Column","values":[]},"props":[{"tag":"Height","values":[40]}],"children":["b-label"]},
            {"key":"b-label","kind":{"tag":"Text","values":[]},"props":[{"tag":"Content","values":["B"]}],"children":[]}
        ]})).unwrap()
    };
    let old = make_view(["a", "b"]);
    let current = make_view(["b", "a"]);
    let mut layout = Layout::default();
    let old_scene = layout.compute(&old, 300.0, 200.0, &mut Text).unwrap();
    let current_scene = layout.compute(&current, 300.0, 200.0, &mut Text).unwrap();
    let old_y = old_scene.boxes["a"].bounds.y;
    let current_y = current_scene.boxes["a"].bounds.y;
    assert!(current_y > old_y);
    let presented = present(
        &current,
        &current_scene,
        &MotionFrame {
            channels: BTreeMap::from([(
                "a".into(),
                BTreeMap::from([("layout-y".into(), old_y as f64)]),
            )]),
            ..MotionFrame::default()
        },
    );
    let shift = old_y - current_y;
    assert!((presented.scene.boxes["a"].bounds.y - old_y).abs() < 0.001);
    assert!(
        (presented.scene.boxes["a-label"].bounds.y
            - (current_scene.boxes["a-label"].bounds.y + shift))
            .abs()
            < 0.001
    );
}
