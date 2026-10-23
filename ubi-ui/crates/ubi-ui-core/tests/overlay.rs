use serde_json::json;
use ubi_ui_core::{OverlayKind, Placement, View};

fn view(nodes: serde_json::Value) -> Result<View, ubi_ui_core::Error> {
    View::decode(json!({"schemaVersion": 1, "root": "root", "nodes": nodes}))
}

#[test]
fn overlay_defaults_and_relationships_are_normalized() {
    let decoded = view(json!([
        {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["trigger","dialog","tip","drawer"]},
        {"key":"trigger","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Help"]}],"children":[]},
        {"key":"dialog","kind":{"tag":"Overlay","values":[]},"props":[
            {"tag":"OverlayKind","values":[{"tag":"Dialog","values":[]}]},
            {"tag":"Label","values":["Details"]}],"children":[]},
        {"key":"tip","kind":{"tag":"Overlay","values":[]},"props":[
            {"tag":"OverlayKind","values":[{"tag":"Tooltip","values":[]}]},
            {"tag":"Anchor","values":["trigger"]},
            {"tag":"Label","values":["Help text"]}],"children":[]},
        {"key":"drawer","kind":{"tag":"Overlay","values":[]},"props":[
            {"tag":"OverlayKind","values":[{"tag":"Drawer","values":[]}]},
            {"tag":"LabelledBy","values":["trigger"]}],"children":[]}
    ]))
    .expect("valid overlay relationships should decode");
    assert!(!decoded.nodes["dialog"].open);
    assert_eq!(decoded.nodes["dialog"].placement, Placement::Center);
    assert_eq!(decoded.nodes["tip"].placement, Placement::Bottom);
    assert_eq!(decoded.nodes["drawer"].placement, Placement::Right);
    assert_eq!(
        decoded.nodes["tip"].overlay_kind,
        Some(OverlayKind::Tooltip)
    );
}

#[test]
fn rejects_dangling_self_and_cyclic_overlay_references() {
    let base = |overlays: [serde_json::Value; 2]| {
        view(json!([
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["a","b"]},
            {"key":"a","kind":{"tag":"Overlay","values":[]},"props":overlays[0],"children":[]},
            {"key":"b","kind":{"tag":"Overlay","values":[]},"props":overlays[1],"children":[]}
        ]))
    };
    let overlay = |kind: &str, anchor: &str| {
        json!([
            {"tag":"OverlayKind","values":[{"tag":kind,"values":[]}]},
            {"tag":"Anchor","values":[anchor]},
            {"tag":"Label","values":["Surface"]}
        ])
    };
    assert!(base([overlay("Popover", "missing"), json!([])]).is_err());
    assert!(base([overlay("Popover", "a"), json!([])]).is_err());
    assert!(base([overlay("Popover", "b"), overlay("Popover", "a")]).is_err());
}

#[test]
fn enforces_overlay_property_pairs_and_bounds() {
    let raw = |props| {
        view(json!([
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["overlay"]},
            {"key":"overlay","kind":{"tag":"Overlay","values":[]},"props":props,"children":[]}
        ]))
    };
    assert!(raw(json!([{ "tag":"Open", "values":[true] }])).is_err());
    assert!(raw(json!([
        {"tag":"OverlayKind","values":[{"tag":"Dialog","values":[]}]},
        {"tag":"TimeoutMs","values":[1]},
        {"tag":"Label","values":["Dialog"]}
    ]))
    .is_err());
    assert!(raw(json!([
        {"tag":"OverlayKind","values":[{"tag":"Toast","values":[]}]},
        {"tag":"TimeoutMs","values":[60001]}
    ]))
    .is_err());
    assert!(raw(json!([
        {"tag":"OverlayKind","values":[{"tag":"Popover","values":[]}]},
        {"tag":"Label","values":["Menu"]}
    ]))
    .is_err());
}
