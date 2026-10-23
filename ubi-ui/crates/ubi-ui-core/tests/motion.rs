use serde_json::json;
use std::collections::BTreeMap;
use ubi_ui_core::{Easing, Kind, Motion, Node, Rect, Transition, View};

#[test]
fn samples_match_shared_motion_fixtures() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/motion.json")).unwrap();
    for case in fixtures["samples"].as_array().unwrap() {
        let easing = match case["motion"]["easing"].as_str().unwrap() {
            "Linear" => Easing::Linear,
            "EaseOut" => Easing::EaseOut,
            "EaseInOut" => Easing::EaseInOut,
            "Spring" => Easing::Spring,
            other => panic!("unknown fixture easing {other}"),
        };
        let motion = Motion {
            duration_ms: case["motion"]["durationMs"].as_u64().unwrap() as u32,
            delay_ms: case["motion"]["delayMs"].as_u64().unwrap() as u32,
            easing,
        };
        let actual = ubi_ui_core::motion::sample(
            case["from"].as_f64().unwrap(),
            case["to"].as_f64().unwrap(),
            case["startMs"].as_u64().unwrap(),
            case["nowMs"].as_u64().unwrap(),
            motion,
        );
        let expected = case["expected"].as_f64().unwrap();
        assert!(
            (actual - expected).abs() <= 0.00001,
            "sample mismatch: {case}, got {actual}"
        );
    }
}

#[test]
fn accepts_motion_properties_at_schema_boundaries() {
    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "root",
        "nodes": [{
            "key": "root",
            "kind": {"tag": "Button", "values": []},
            "props": [
                {"tag": "Motion", "values": [{
                    "durationMs": 2000,
                    "delayMs": 1000,
                    "easing": {"tag": "Spring", "values": []}
                }]},
                {"tag": "Enter", "values": [{"tag": "Rise", "values": []}]},
                {"tag": "Exit", "values": [{"tag": "Scale", "values": []}]},
                {"tag": "Opacity", "values": [100]},
                {"tag": "Translate", "values": [-100000, 100000]},
                {"tag": "Scale", "values": [200]}
            ],
            "children": []
        }]
    }));
    let view = view.expect("valid motion ABI should decode");
    let node = &view.nodes["root"];
    assert_eq!(
        node.motion,
        Some(Motion {
            duration_ms: 2000,
            delay_ms: 1000,
            easing: Easing::Spring
        })
    );
    assert_eq!(node.enter, Transition::Rise);
    assert_eq!(node.exit, Transition::Scale);
    assert_eq!(node.opacity, 1.0);
    assert_eq!(node.translate, (-100000, 100000));
    assert_eq!(node.scale, 2.0);
}

#[test]
fn rejects_malformed_motion_shapes_and_out_of_range_targets() {
    let invalid = [
        json!([{"tag":"Motion","values":[{"durationMs":2001,"delayMs":0,"easing":{"tag":"Linear","values":[]}}]}]),
        json!([{"tag":"Motion","values":[{"durationMs":0,"delayMs":1001,"easing":{"tag":"Linear","values":[]}}]}]),
        json!([{"tag":"Motion","values":[{"durationMs":0,"delayMs":0,"easing":{"tag":"Linear","values":[true]}}]}]),
        json!([{"tag":"Motion","values":[{"durationMs":0,"delayMs":0,"easing":{"tag":"Unknown","values":[]}}]}]),
        json!([{"tag":"Motion","values":[{"durationMs":0,"delayMs":0,"easing":{"tag":"Linear","values":[]},"extra":1}]}]),
        json!([{"tag":"Opacity","values":[101]}]),
        json!([{"tag":"Opacity","values":[12.5]}]),
        json!([{"tag":"Translate","values":[100001,0]}]),
        json!([{"tag":"Translate","values":[1]}]),
        json!([{"tag":"Scale","values":[24]}]),
        json!([{"tag":"Scale","values":[201]}]),
        json!([
            {"tag":"Opacity","values":[50]},
            {"tag":"Opacity","values":[60]}
        ]),
    ];
    for props in invalid {
        let view = View::decode(json!({
            "schemaVersion": 1,
            "root": "root",
            "nodes": [{
                "key": "root",
                "kind": {"tag": "Button", "values": []},
                "props": props,
                "children": []
            }]
        }));
        assert!(view.is_err(), "invalid motion ABI was accepted: {props}");
    }
}

#[test]
fn interrupted_tracks_retarget_from_the_presented_value() {
    use ubi_ui_core::motion::{MotionState, MotionTarget};

    let target = |opacity, duration_ms| MotionTarget {
        key: "button".into(),
        kind: Kind::Button,
        channels: BTreeMap::from([("opacity".into(), opacity)]),
        motion: Motion {
            duration_ms,
            delay_ms: 0,
            easing: Easing::Linear,
        },
        enter: Transition::None,
        exit: Transition::None,
        snapshot: None,
    };
    let mut state = MotionState::default();
    state.commit(&[target(0.0, 0)], 0, false).unwrap();
    state.commit(&[target(1.0, 100)], 0, false).unwrap();
    assert_eq!(state.sample(50, false).channels["button"]["opacity"], 0.5);
    state.commit(&[target(0.0, 100)], 50, false).unwrap();
    assert_eq!(state.sample(50, false).channels["button"]["opacity"], 0.5);
    assert_eq!(state.sample(100, false).channels["button"]["opacity"], 0.25);
}

#[test]
fn resurrection_samples_an_exit_at_the_recommit_timestamp() {
    use ubi_ui_core::motion::{MotionState, MotionTarget, SnapshotNode, VisualSnapshot};

    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "panel",
        "nodes": [{"key":"panel","kind":{"tag":"View","values":[]},"props":[],"children":[]}]
    }))
    .unwrap();
    let target = || MotionTarget {
        key: "panel".into(),
        kind: Kind::View,
        channels: BTreeMap::from([("opacity".into(), 1.0)]),
        motion: Motion {
            duration_ms: 100,
            delay_ms: 0,
            easing: Easing::Linear,
        },
        enter: Transition::None,
        exit: Transition::Fade,
        snapshot: Some(VisualSnapshot {
            root: "panel".into(),
            nodes: vec![SnapshotNode {
                node: view.nodes["panel"].clone(),
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                clip: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
            }],
        }),
    };
    let mut state = MotionState::default();
    state.commit(&[target()], 0, false).unwrap();
    state.commit(&[], 10, false).unwrap();
    state.commit(&[target()], 50, false).unwrap();
    assert_eq!(state.sample(50, false).channels["panel"]["opacity"], 0.6);
}

#[test]
fn exit_snapshot_removes_editable_subtrees_and_keeps_visual_siblings() {
    use ubi_ui_core::motion::{MotionState, MotionTarget, SnapshotNode, VisualSnapshot};

    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "panel",
        "nodes": [{"key":"panel","kind":{"tag":"View","values":[]},"props":[],"children":[]}]
    }))
    .unwrap();
    let make_node = |key: &str, kind: Kind, children: &[&str]| {
        let mut node: Node = view.nodes["panel"].clone();
        node.key = key.into();
        node.kind = kind;
        node.children = children.iter().map(|child| (*child).into()).collect();
        node
    };
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let snapshot = VisualSnapshot {
        root: "panel".into(),
        nodes: vec![
            (
                make_node("panel", Kind::View, &["field", "caption"]),
                bounds,
            ),
            (
                make_node("field", Kind::TextInput, &["field-decoration"]),
                bounds,
            ),
            (make_node("field-decoration", Kind::Text, &[]), bounds),
            (make_node("caption", Kind::Text, &[]), bounds),
        ]
        .into_iter()
        .map(|(node, bounds)| SnapshotNode {
            node,
            bounds,
            clip: bounds,
        })
        .collect(),
    };
    let target = MotionTarget {
        key: "panel".into(),
        kind: Kind::View,
        channels: BTreeMap::from([("opacity".into(), 1.0)]),
        motion: Motion {
            duration_ms: 100,
            delay_ms: 0,
            easing: Easing::Linear,
        },
        enter: Transition::None,
        exit: Transition::Fade,
        snapshot: Some(snapshot),
    };
    let mut state = MotionState::default();
    state.commit(&[target], 0, false).unwrap();
    state.commit(&[], 10, false).unwrap();
    let exit = &state.sample(10, false).exits[0];
    let keys: Vec<_> = exit
        .snapshot
        .nodes
        .iter()
        .map(|entry| entry.node.key.as_str())
        .collect();
    assert_eq!(keys, ["panel", "caption"]);
}

#[test]
fn transition_synthetic_endpoints_stay_inside_declared_channel_bounds() {
    use ubi_ui_core::motion::{MotionState, MotionTarget, SnapshotNode, VisualSnapshot};

    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "panel",
        "nodes": [{"key":"panel","kind":{"tag":"View","values":[]},"props":[],"children":[]}]
    }))
    .unwrap();
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let mut state = MotionState::default();
    let mut target = MotionTarget {
        key: "panel".into(),
        kind: Kind::View,
        channels: BTreeMap::from([
            ("opacity".into(), 1.0),
            ("translate-y".into(), 100_000.0),
            ("scale".into(), 0.25),
        ]),
        motion: Motion {
            duration_ms: 100,
            delay_ms: 0,
            easing: Easing::Linear,
        },
        enter: Transition::Rise,
        exit: Transition::Scale,
        snapshot: Some(VisualSnapshot {
            root: "panel".into(),
            nodes: vec![SnapshotNode {
                node: view.nodes["panel"].clone(),
                bounds,
                clip: bounds,
            }],
        }),
    };
    state.commit(&[target.clone()], 0, false).unwrap();
    let values = &state.sample(0, false).channels["panel"];
    assert_eq!(values["translate-y"], 100_000.0);
    assert_eq!(values["scale"], 0.25);
    assert_eq!(values["opacity"], 0.0);

    target.enter = Transition::None;
    state.commit(&[target], 100, false).unwrap();
    state.commit(&[], 101, false).unwrap();
    let exit = &state.sample(101, false).exits[0];
    assert_eq!(exit.channels["scale"], 0.25);
    assert_eq!(exit.channels["translate-y"], 100_000.0);
}

#[test]
fn exit_snapshots_share_the_flat_visual_node_budget() {
    use ubi_ui_core::motion::{MotionState, MotionTarget, SnapshotNode, VisualSnapshot};

    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "base",
        "nodes": [{"key":"base","kind":{"tag":"View","values":[]},"props":[],"children":[]}]
    }))
    .unwrap();
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
    let target = |root: &str| {
        let child_keys: Vec<_> = (0..2999).map(|index| format!("{root}-{index}")).collect();
        let mut root_node: Node = view.nodes["base"].clone();
        root_node.key = root.into();
        root_node.children = child_keys.clone();
        let mut nodes = Vec::with_capacity(3000);
        nodes.push(SnapshotNode {
            node: root_node,
            bounds,
            clip: bounds,
        });
        nodes.extend(child_keys.into_iter().map(|key| {
            let mut node = view.nodes["base"].clone();
            node.key = key;
            SnapshotNode {
                node,
                bounds,
                clip: bounds,
            }
        }));
        MotionTarget {
            key: root.into(),
            kind: Kind::View,
            channels: BTreeMap::from([("opacity".into(), 1.0)]),
            motion: Motion {
                duration_ms: 100,
                delay_ms: 0,
                easing: Easing::Linear,
            },
            enter: Transition::None,
            exit: Transition::Fade,
            snapshot: Some(VisualSnapshot {
                root: root.into(),
                nodes,
            }),
        }
    };
    let mut state = MotionState::default();
    state
        .commit(&[target("first"), target("second")], 0, false)
        .unwrap();
    state.commit(&[], 1, false).unwrap();
    let frame = state.sample(1, false);
    assert_eq!(frame.exits.len(), 1);
    assert_eq!(frame.exits[0].snapshot.root, "second");
    assert_eq!(frame.exits[0].snapshot.nodes.len(), 3000);
}

#[test]
fn exit_snapshots_remove_overlay_and_rotation_event_messages() {
    use ubi_ui_core::motion::{MotionState, MotionTarget, SnapshotNode, VisualSnapshot};

    let view = View::decode(json!({
        "schemaVersion": 1,
        "root": "action",
        "nodes": [{"key":"action","kind":{"tag":"Button","values":[]},"props":[],"children":[]}]
    }))
    .unwrap();
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 24.0,
        height: 24.0,
    };
    let mut node = view.nodes["action"].clone();
    node.dismiss = Some(json!({"secret":"dismiss"}));
    node.open_request = Some(json!({"secret":"open"}));
    node.advance = Some(json!({"secret":"advance"}));
    let target = MotionTarget {
        key: "action".into(),
        kind: Kind::Button,
        channels: BTreeMap::from([("opacity".into(), 1.0)]),
        motion: Motion {
            duration_ms: 100,
            delay_ms: 0,
            easing: Easing::Linear,
        },
        enter: Transition::None,
        exit: Transition::Fade,
        snapshot: Some(VisualSnapshot {
            root: "action".into(),
            nodes: vec![SnapshotNode {
                node,
                bounds,
                clip: bounds,
            }],
        }),
    };
    let mut state = MotionState::default();
    state.commit(&[target], 0, false).unwrap();
    state.commit(&[], 1, false).unwrap();
    let frame = state.sample(1, false);
    let exit_node = &frame.exits[0].snapshot.nodes[0].node;
    assert!(exit_node.dismiss.is_none());
    assert!(exit_node.open_request.is_none());
    assert!(exit_node.advance.is_none());
}
