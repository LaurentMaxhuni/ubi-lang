use serde_json::json;
use ubi_ui_core::View;

#[test]
fn decodes_the_overlay_component_contract() {
    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "root",
        "nodes": [
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["dialog"]},
            {"key":"dialog","kind":{"tag":"Overlay","values":[]},"props":[
                {"tag":"OverlayKind","values":[{"tag":"Dialog","values":[]}]},
                {"tag":"Open","values":[true]},
                {"tag":"Label","values":["Account details"]},
                {"tag":"Dismiss","values":[{"tag":"Close","values":[]}]}
            ],"children":[]}
        ]
    }));
    assert!(
        view.is_ok(),
        "valid overlay contract should decode: {view:?}"
    );
}
