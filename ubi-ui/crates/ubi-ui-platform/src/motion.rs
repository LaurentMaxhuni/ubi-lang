use std::collections::{BTreeMap, BTreeSet};

use ubi_ui_core::motion::{MotionTarget, SnapshotNode, VisualSnapshot};
use ubi_ui_core::{Easing, Effect, Kind, Motion, Transition, View};
use ubi_ui_input::Input;
use ubi_ui_layout::Scene;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionPreference {
    #[default]
    System,
    Full,
    Reduced,
}

impl MotionPreference {
    pub fn is_reduced(self) -> bool {
        match self {
            Self::Reduced => true,
            Self::Full => false,
            Self::System => system_reduced_motion(),
        }
    }
}

#[cfg(windows)]
fn system_reduced_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
    };
    let mut enabled: i32 = 0;
    // The API writes a Win32 BOOL to the caller-provided output pointer.
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        )
    };
    result == 0 || enabled == 0
}

#[cfg(not(windows))]
fn system_reduced_motion() -> bool {
    true
}

pub(crate) fn targets(
    view: &View,
    scene: &Scene,
    input: &Input,
    previous: Option<(&View, &Scene)>,
) -> Vec<MotionTarget> {
    if scene.order.is_empty() {
        let mut synthetic = scene.clone();
        synthetic.order.clone_from(&view.order);
        return targets_at(
            view,
            &synthetic,
            input,
            previous,
            &mut BTreeSet::new(),
            0,
            false,
        );
    }
    targets_at(view, scene, input, previous, &mut BTreeSet::new(), 0, false)
}

pub(crate) fn targets_at(
    view: &View,
    scene: &Scene,
    input: &Input,
    previous: Option<(&View, &Scene)>,
    revealed_scroll: &mut BTreeSet<String>,
    now_ms: u64,
    reduced: bool,
) -> Vec<MotionTarget> {
    revealed_scroll.retain(|key| {
        view.nodes
            .get(key)
            .is_some_and(|node| node.effect == Some(Effect::ScrollReveal))
    });
    let parents = parent_map(view);
    let mut covered = BTreeSet::new();
    let mut targets = Vec::with_capacity(scene.order.len());
    for key in &scene.order {
        let Some(node) = view.nodes.get(key) else {
            continue;
        };
        let animated_item = animated_list_item(view, key, &parents);
        let selected_effect_item = selected_effect_item(view, key, &parents);
        let (snapshot_source, full_snapshot) =
            if node.kind == Kind::Overlay && node.open || node.disclosure == Some(true) {
                (Some((view, scene)), true)
            } else if let Some((old_view, old_scene)) = previous {
                let was_open = old_view.nodes.get(key).is_some_and(|old| {
                    old.kind == Kind::Overlay && old.open || old.disclosure == Some(true)
                });
                if !node.open && node.kind == Kind::Overlay && was_open
                    || node.disclosure == Some(false) && was_open
                {
                    (Some((old_view, old_scene)), true)
                } else if let Some((_, item_root, _)) = animated_item {
                    (Some((view, scene)), item_root == key)
                } else if let Some((_, item_root, _)) = selected_effect_item {
                    (Some((view, scene)), item_root == key)
                } else {
                    (Some((view, scene)), false)
                }
            } else if let Some((_, item_root, _)) = animated_item {
                (Some((view, scene)), item_root == key)
            } else if let Some((_, item_root, _)) = selected_effect_item {
                (Some((view, scene)), item_root == key)
            } else {
                (Some((view, scene)), false)
            };

        let mut channels = BTreeMap::from([
            ("opacity".into(), node.opacity),
            ("translate-x".into(), node.translate.0 as f64),
            ("translate-y".into(), node.translate.1 as f64),
            ("scale".into(), node.scale),
        ]);
        let active = input.pressed(key) || node.pressed;
        if matches!(node.kind, Kind::Button | Kind::IconButton | Kind::Toggle) {
            channels.insert("scale".into(), node.scale * if active { 0.97 } else { 1.0 });
        }
        match node.kind {
            Kind::Switch => {
                channels.insert("switch-thumb".into(), if node.checked { 1.0 } else { 0.0 });
            }
            Kind::ProgressBar => {
                channels.insert("progress-fill".into(), node.progress as f64 / 100.0);
            }
            Kind::Radio | Kind::Tab | Kind::ToggleItem => {
                channels.insert("tab-selection".into(), if node.checked { 1.0 } else { 0.0 });
            }
            Kind::EffectSurface if node.effect == Some(Effect::NumberTicker) => {
                channels.insert(
                    "ticker-value".into(),
                    node.number.unwrap_or_default() as f64,
                );
            }
            _ => {}
        }
        if let Some(open) = node.disclosure {
            channels.insert("disclosure-progress".into(), if open { 1.0 } else { 0.0 });
        }
        if node.kind == Kind::Overlay {
            channels.insert("overlay-progress".into(), if node.open { 1.0 } else { 0.0 });
        }
        if node.effect == Some(Effect::TextReveal) {
            channels.insert("text-reveal-progress".into(), 1.0);
        }
        if node.kind == Kind::Skeleton {
            channels.insert(
                "skeleton-phase".into(),
                if reduced {
                    0.0
                } else {
                    (now_ms % 1200) as f64 / 1200.0
                },
            );
        }
        let newly_revealed = if node.effect == Some(Effect::ScrollReveal) {
            let visible = scene.boxes.get(key).is_some_and(|layout| {
                let rect = layout
                    .bounds
                    .intersect(layout.clip)
                    .intersect(scene.viewport);
                rect.width > 0.0 && rect.height > 0.0
            });
            let focused = input
                .focused
                .as_deref()
                .is_some_and(|focused| within(view, key, focused));
            let was_revealed = revealed_scroll.contains(key);
            if visible || focused || reduced {
                revealed_scroll.insert(key.clone());
            }
            channels.insert(
                "scroll-reveal-progress".into(),
                if revealed_scroll.contains(key) {
                    1.0
                } else {
                    0.0
                },
            );
            !was_revealed && revealed_scroll.contains(key)
        } else {
            false
        };
        if animated_item.is_some_and(|(_, item_root, _)| item_root == key) {
            if let Some(bounds) = scene.boxes.get(key).map(|layout| layout.bounds) {
                channels.insert("layout-x".into(), bounds.x as f64);
                channels.insert("layout-y".into(), bounds.y as f64);
            }
        }
        if node.kind == Kind::Row && node.role == "tablist" {
            if let Some(selected) = node
                .children
                .iter()
                .filter_map(|tab| view.nodes.get(tab))
                .find(|tab| tab.kind == Kind::Tab && tab.checked)
            {
                if let (Some(parent), Some(tab)) =
                    (scene.boxes.get(key), scene.boxes.get(&selected.key))
                {
                    channels.insert(
                        "tab-indicator-x".into(),
                        (tab.bounds.x - parent.bounds.x) as f64,
                    );
                    channels.insert("tab-indicator-width".into(), tab.bounds.width as f64);
                }
            }
        }
        if segmented_selection(view, node) {
            if let Some(selected) = node
                .children
                .iter()
                .filter_map(|child| view.nodes.get(child))
                .find(|item| item.kind == Kind::ToggleItem && item.checked)
            {
                if let (Some(parent), Some(item)) =
                    (scene.boxes.get(key), scene.boxes.get(&selected.key))
                {
                    channels.insert(
                        "tab-indicator-x".into(),
                        (item.bounds.x - parent.bounds.x) as f64,
                    );
                    channels.insert("tab-indicator-width".into(), item.bounds.width as f64);
                }
            }
        }

        let list_item_index = animated_item
            .filter(|(_, item_root, _)| *item_root == key)
            .map(|(_, _, index)| index);
        let mut enter = node.enter;
        let mut exit = node.exit;
        if list_item_index.is_some() {
            if enter == Transition::None {
                enter = Transition::Rise;
            }
            if exit == Transition::None {
                exit = Transition::Rise;
            }
        }
        let effect_item_root =
            selected_effect_item.is_some_and(|(_, item_root, _)| item_root == key);
        if effect_item_root {
            if enter == Transition::None {
                enter = Transition::Rise;
            }
            if exit == Transition::None {
                exit = Transition::Rise;
            }
        }
        if node.disclosure.is_some() {
            if enter == Transition::None {
                enter = Transition::Fade;
            }
            if exit == Transition::None {
                exit = Transition::Fade;
            }
        }
        if node.kind == Kind::Overlay {
            if enter == Transition::None {
                enter = Transition::Fade;
            }
            if exit == Transition::None {
                exit = Transition::Fade;
            }
        }
        if node.effect == Some(Effect::ScrollReveal) {
            if newly_revealed {
                if enter == Transition::None {
                    enter = Transition::Rise;
                }
            } else if channels.get("scroll-reveal-progress") == Some(&0.0) {
                enter = Transition::None;
                exit = Transition::None;
            } else if input
                .focused
                .as_deref()
                .is_some_and(|focused| within(view, key, focused))
                && reduced
            {
                enter = Transition::None;
            }
        }
        let mut motion = node
            .motion
            .unwrap_or_else(|| default_motion(node, enter, exit));
        if effect_item_root && node.motion.is_none() {
            motion = Motion {
                duration_ms: 220,
                delay_ms: 0,
                easing: Easing::Spring,
            };
        }

        let snapshot = snapshot_source.and_then(|(source_view, source_scene)| {
            if covered.contains(key) {
                return None;
            }
            if full_snapshot {
                let snapshot = subtree_snapshot(source_view, source_scene, key)?;
                for entry in &snapshot.nodes {
                    covered.insert(entry.node.key.clone());
                }
                Some(snapshot)
            } else {
                single_snapshot(source_view, source_scene, key)
            }
        });
        targets.push(MotionTarget {
            key: key.clone(),
            kind: node.kind,
            channels,
            motion: if node.kind == Kind::Skeleton {
                Motion {
                    duration_ms: 0,
                    delay_ms: 0,
                    easing: Easing::Linear,
                }
            } else if let Some(index) = list_item_index {
                Motion {
                    delay_ms: (index as u32 * 40).min(1000),
                    ..motion
                }
            } else {
                motion
            },
            enter,
            exit,
            snapshot,
        });
    }
    targets
}

fn within(view: &View, root: &str, key: &str) -> bool {
    root == key
        || view
            .nodes
            .get(root)
            .is_some_and(|node| node.children.iter().any(|child| within(view, child, key)))
}

fn segmented_selection(view: &View, node: &ubi_ui_core::Node) -> bool {
    node.kind == Kind::Row
        && node.role == "group"
        && !node.children.is_empty()
        && node.children.iter().all(|key| {
            view.nodes
                .get(key)
                .is_some_and(|item| item.kind == Kind::ToggleItem && item.group == node.key)
        })
        && node
            .children
            .iter()
            .filter_map(|key| view.nodes.get(key))
            .filter(|item| item.checked)
            .count()
            == 1
}

fn default_motion(node: &ubi_ui_core::Node, enter: Transition, exit: Transition) -> Motion {
    let (duration_ms, easing) = if node.disclosure.is_some() {
        (220, Easing::EaseInOut)
    } else if matches!(
        node.kind,
        Kind::Switch | Kind::Radio | Kind::Tab | Kind::ToggleItem
    ) || node.kind == Kind::Row && (node.role == "tablist" || node.role == "group")
    {
        (220, Easing::Spring)
    } else if node.effect == Some(Effect::NumberTicker) {
        (500, Easing::EaseOut)
    } else if matches!(
        node.kind,
        Kind::Button | Kind::IconButton | Kind::Toggle | Kind::ProgressBar
    ) || node.kind == Kind::Overlay
    {
        (140, Easing::EaseOut)
    } else if enter != Transition::None || exit != Transition::None {
        (220, Easing::EaseOut)
    } else {
        (0, Easing::Linear)
    };
    Motion {
        duration_ms,
        delay_ms: 0,
        easing,
    }
}

fn parent_map(view: &View) -> BTreeMap<String, String> {
    view.nodes
        .values()
        .flat_map(|node| {
            node.children
                .iter()
                .map(move |child| (child.clone(), node.key.clone()))
        })
        .collect()
}

fn animated_list_item<'a>(
    view: &'a View,
    key: &'a str,
    parents: &'a BTreeMap<String, String>,
) -> Option<(&'a str, &'a str, usize)> {
    let mut current = key;
    for _ in 0..32 {
        let parent_key = parents.get(current)?;
        let parent = view.nodes.get(parent_key)?;
        let index = parent.children.iter().position(|child| child == current)?;
        if parent.kind == Kind::EffectSurface && parent.effect == Some(Effect::AnimatedList) {
            return Some((parent_key, current, index));
        }
        current = parent_key;
    }
    None
}

fn selected_effect_item<'a>(
    view: &'a View,
    key: &'a str,
    parents: &'a BTreeMap<String, String>,
) -> Option<(&'a str, &'a str, usize)> {
    let mut current = key;
    for _ in 0..64 {
        let parent_key = parents.get(current)?;
        let parent = view.nodes.get(parent_key)?;
        if matches!(
            parent.effect,
            Some(Effect::Carousel | Effect::CardStack | Effect::RotatingText)
        ) && parent.selected_child_index().is_some_and(|index| {
            parent
                .children
                .get(index)
                .is_some_and(|child| child == current)
        }) {
            let index = parent.selected_child_index()?;
            return Some((parent_key.as_str(), current, index));
        }
        current = parent_key;
    }
    None
}

fn subtree_snapshot(view: &View, scene: &Scene, root: &str) -> Option<VisualSnapshot> {
    fn collect(view: &View, scene: &Scene, key: &str, output: &mut Vec<SnapshotNode>) {
        let Some(node) = view.nodes.get(key) else {
            return;
        };
        if let Some(layout) = scene.boxes.get(key) {
            output.push(SnapshotNode {
                node: node.clone(),
                bounds: layout.bounds,
                clip: layout.clip,
            });
        }
        for child in &node.children {
            collect(view, scene, child, output);
        }
    }
    let mut nodes = Vec::new();
    collect(view, scene, root, &mut nodes);
    nodes
        .iter()
        .any(|entry| entry.node.key == root)
        .then(|| VisualSnapshot {
            root: root.into(),
            nodes,
        })
}

fn single_snapshot(view: &View, scene: &Scene, key: &str) -> Option<VisualSnapshot> {
    let node = view.nodes.get(key)?;
    let layout = scene.boxes.get(key)?;
    let mut node = node.clone();
    node.children.clear();
    Some(VisualSnapshot {
        root: key.into(),
        nodes: vec![SnapshotNode {
            node,
            bounds: layout.bounds,
            clip: layout.clip,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use ubi_ui_core::motion::MotionState;

    fn control(kind: &str, props: Value) -> View {
        View::decode(json!({
            "schemaVersion": 1,
            "root": "control",
            "nodes": [{
                "key": "control",
                "kind": {"tag": kind, "values": []},
                "props": props,
                "children": []
            }]
        }))
        .expect("test control ABI should decode")
    }

    fn channel(view: &View, name: &str) -> f64 {
        targets(view, &Scene::default(), &Input::default(), None)[0].channels[name]
    }

    fn midpoint(from: &View, to: &View, channel_name: &str, at_ms: u64) -> f64 {
        let scene = Scene::default();
        let input = Input::default();
        let mut state = MotionState::default();
        state
            .commit(&targets(from, &scene, &input, None), 0, false)
            .unwrap();
        state
            .commit(&targets(to, &scene, &input, Some((from, &scene))), 0, false)
            .unwrap();
        state.sample(at_ms, false).channels["control"][channel_name]
    }

    #[test]
    fn switch_progress_and_number_ticker_drive_animated_paint_channels() {
        let off = control("Switch", json!([]));
        let on = control("Switch", json!([{"tag":"Checked","values":[true]}]));
        assert_eq!(channel(&off, "switch-thumb"), 0.0);
        assert_eq!(channel(&on, "switch-thumb"), 1.0);
        let thumb = midpoint(&off, &on, "switch-thumb", 20);
        assert!(
            thumb > 0.0 && thumb < 1.0,
            "switch thumb did not interpolate: {thumb}"
        );

        let empty = control("ProgressBar", json!([{"tag":"Progress","values":[0]}]));
        let full = control("ProgressBar", json!([{"tag":"Progress","values":[100]}]));
        let fill = midpoint(&empty, &full, "progress-fill", 70);
        assert!(
            fill > 0.0 && fill < 1.0,
            "progress fill did not interpolate: {fill}"
        );

        let zero = control(
            "EffectSurface",
            json!([
                {"tag":"Effect","values":[{"tag":"NumberTicker","values":[]}]},
                {"tag":"Number","values":[0]}
            ]),
        );
        let hundred = control(
            "EffectSurface",
            json!([
                {"tag":"Effect","values":[{"tag":"NumberTicker","values":[]}]},
                {"tag":"Number","values":[100]}
            ]),
        );
        let ticker = midpoint(&zero, &hundred, "ticker-value", 250);
        assert!(
            ticker > 0.0 && ticker < 100.0,
            "ticker value did not interpolate: {ticker}"
        );
    }

    #[test]
    fn animated_list_assigns_rise_and_stagger_per_item() {
        let view = View::decode(json!({
            "schemaVersion": 1,
            "root": "list",
            "nodes": [
                {"key":"list","kind":{"tag":"EffectSurface","values":[]},"props":[{"tag":"Effect","values":[{"tag":"AnimatedList","values":[]}]}],"children":["a","b","c"]},
                {"key":"a","kind":{"tag":"Column","values":[]},"props":[],"children":["a-label"]},
                {"key":"a-label","kind":{"tag":"Text","values":[]},"props":[],"children":[]},
                {"key":"b","kind":{"tag":"Column","values":[]},"props":[],"children":["b-label"]},
                {"key":"b-label","kind":{"tag":"Text","values":[]},"props":[],"children":[]},
                {"key":"c","kind":{"tag":"Column","values":[]},"props":[],"children":["c-label"]},
                {"key":"c-label","kind":{"tag":"Text","values":[]},"props":[],"children":[]}
            ]
        })).unwrap();
        let generated = targets(&view, &Scene::default(), &Input::default(), None);
        for (key, delay) in [("a", 0), ("b", 40), ("c", 80)] {
            let item = generated.iter().find(|target| target.key == key).unwrap();
            assert_eq!(item.enter, ubi_ui_core::Transition::Rise);
            assert_eq!(item.exit, ubi_ui_core::Transition::Rise);
            assert_eq!(item.motion.delay_ms, delay);
            let label = generated
                .iter()
                .find(|target| target.key == format!("{key}-label"))
                .unwrap();
            assert_eq!(label.enter, ubi_ui_core::Transition::None);
            assert_eq!(label.motion.delay_ms, 0);
            assert!(!label.channels.contains_key("layout-y"));
        }
    }

    #[test]
    fn animated_list_accepts_accumulated_layout_coordinates() {
        let view = View::decode(json!({
            "schemaVersion": 1,
            "root": "list",
            "nodes": [
                {"key":"list","kind":{"tag":"EffectSurface","values":[]},"props":[{"tag":"Effect","values":[{"tag":"AnimatedList","values":[]}]}],"children":["item"]},
                {"key":"item","kind":{"tag":"Text","values":[]},"props":[],"children":[]}
            ]
        })).unwrap();
        let layout = |x, y| ubi_ui_layout::BoxLayout {
            bounds: ubi_ui_core::Rect {
                x,
                y,
                width: 80.0,
                height: 24.0,
            },
            clip: ubi_ui_core::Rect {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 300.0,
            },
            content_height: 24.0,
        };
        let scene = Scene {
            boxes: BTreeMap::from([
                ("list".into(), layout(0.0, 0.0)),
                ("item".into(), layout(250_000.0, 400_000.0)),
            ]),
            order: vec!["list".into(), "item".into()],
            viewport: ubi_ui_core::Rect {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 300.0,
            },
            context_points: BTreeMap::new(),
        };
        let generated = targets(&view, &scene, &Input::default(), None);
        let item = generated
            .iter()
            .find(|target| target.key == "item")
            .unwrap();
        assert_eq!(item.channels["layout-x"], 250_000.0);
        assert_eq!(item.channels["layout-y"], 400_000.0);
        MotionState::default()
            .commit(&generated, 0, false)
            .expect("layout channels should fit the validated scene envelope");
    }
}
