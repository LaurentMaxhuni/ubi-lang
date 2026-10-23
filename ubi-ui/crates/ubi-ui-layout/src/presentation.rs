//! Presented transforms shared by painting, hit testing and accessibility.
use crate::Scene;
use std::collections::BTreeMap;
use ubi_ui_core::motion::{MotionFrame, VisualExit};
use ubi_ui_core::{Kind, OverlayKind, Placement, Rect, View};

#[derive(Clone, Debug, Default)]
pub struct PresentedScene {
    pub scene: Scene,
    pub opacity: BTreeMap<String, f32>,
    pub channels: BTreeMap<String, BTreeMap<String, f32>>,
    pub exits: Vec<VisualExit>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Transform {
    scale: f32,
    x: f32,
    y: f32,
    opacity: f32,
}

pub fn present(view: &View, scene: &Scene, frame: &MotionFrame) -> PresentedScene {
    let mut result = PresentedScene {
        scene: scene.clone(),
        channels: frame
            .channels
            .iter()
            .map(|(key, values)| {
                (
                    key.clone(),
                    values
                        .iter()
                        .map(|(name, value)| (name.clone(), *value as f32))
                        .collect(),
                )
            })
            .collect(),
        ..Default::default()
    };
    let mut transforms = BTreeMap::new();
    let mut opacity = BTreeMap::new();
    #[allow(clippy::too_many_arguments)]
    fn visit(
        view: &View,
        base: &Scene,
        out: &mut Scene,
        key: &str,
        parent: Transform,
        frame: &MotionFrame,
        transforms: &mut BTreeMap<String, Transform>,
        opacity: &mut BTreeMap<String, f32>,
    ) {
        let Some(node) = view.nodes.get(key) else {
            return;
        };
        let Some(original) = base.boxes.get(key) else {
            return;
        };
        let channels = frame.channels.get(key);
        let local_scale = channels
            .and_then(|c| c.get("scale"))
            .copied()
            .unwrap_or(node.scale) as f32;
        let local_x = channels
            .and_then(|c| c.get("translate-x"))
            .copied()
            .unwrap_or(node.translate.0 as f64) as f32;
        let local_y = channels
            .and_then(|c| c.get("translate-y"))
            .copied()
            .unwrap_or(node.translate.1 as f64) as f32;
        let layout_x = channels
            .and_then(|c| c.get("layout-x"))
            .copied()
            .unwrap_or(original.bounds.x as f64) as f32;
        let layout_y = channels
            .and_then(|c| c.get("layout-y"))
            .copied()
            .unwrap_or(original.bounds.y as f64) as f32;
        let mut local_opacity = channels
            .and_then(|c| c.get("opacity"))
            .copied()
            .unwrap_or(node.opacity) as f32;
        if node.kind == Kind::Overlay {
            local_opacity *= channels
                .and_then(|c| c.get("overlay-progress"))
                .copied()
                .unwrap_or(if node.open { 1.0 } else { 0.0 }) as f32;
        }
        if node.effect == Some(ubi_ui_core::Effect::ScrollReveal) {
            local_opacity *= channels
                .and_then(|c| c.get("scroll-reveal-progress"))
                .copied()
                .unwrap_or(1.0) as f32;
        }
        let center_x = original.bounds.x + original.bounds.width * 0.5;
        let center_y = original.bounds.y + original.bounds.height * 0.5;
        let mapped_center_x = parent.scale * center_x + parent.x;
        let mapped_center_y = parent.scale * center_y + parent.y;
        let scale = parent.scale * local_scale;
        let effective = Transform {
            scale,
            x: mapped_center_x + parent.scale * (local_x + layout_x - original.bounds.x)
                - scale * center_x,
            y: mapped_center_y + parent.scale * (local_y + layout_y - original.bounds.y)
                - scale * center_y,
            opacity: (parent.opacity * local_opacity).clamp(0.0, 1.0),
        };
        let mut box_layout = original.clone();
        box_layout.bounds = transform_rect(original.bounds, effective);
        box_layout.clip = transform_rect(original.clip, parent).intersect(base.viewport);
        out.boxes.insert(key.into(), box_layout);
        transforms.insert(key.into(), effective);
        opacity.insert(key.into(), effective.opacity);
        for child in &node.children {
            visit(
                view, base, out, child, effective, frame, transforms, opacity,
            );
        }
    }
    let identity = Transform {
        scale: 1.0,
        opacity: 1.0,
        ..Default::default()
    };
    visit(
        view,
        scene,
        &mut result.scene,
        &view.root,
        identity,
        frame,
        &mut transforms,
        &mut opacity,
    );
    result.opacity = opacity;

    // Disclosure content is laid out at its open size, then clipped and its
    // following siblings are shifted by the same sampled height fraction.
    for key in &view.order {
        let Some(node) = view.nodes.get(key).filter(|node| node.disclosure.is_some()) else {
            continue;
        };
        let Some(layout) = result.scene.boxes.get(key).cloned() else {
            continue;
        };
        let progress = frame
            .channels
            .get(key)
            .and_then(|channels| channels.get("disclosure-progress"))
            .copied()
            .unwrap_or(if node.disclosure == Some(false) {
                0.0
            } else {
                1.0
            })
            .clamp(0.0, 1.0) as f32;
        let visible_height = layout.bounds.height * progress;
        if let Some(bounds) = result
            .scene
            .boxes
            .get_mut(key)
            .map(|layout| &mut layout.bounds)
        {
            bounds.height = visible_height;
        }
        let visible = Rect {
            height: visible_height,
            ..layout.bounds
        };
        for child in &node.children {
            clip_subtree(view, &mut result.scene, child, visible);
        }
        shift_after(
            view,
            &mut result.scene,
            key,
            -layout.bounds.height * (1.0 - progress),
        );
    }

    // A just-closed disclosure no longer has live content geometry. Its
    // sanitized exit layer supplies the old measured height for sibling flow.
    for exit in &frame.exits {
        let Some(root) = exit
            .snapshot
            .nodes
            .iter()
            .find(|entry| entry.node.key == exit.snapshot.root)
        else {
            continue;
        };
        if root.node.disclosure.is_none() || !view.nodes.contains_key(&exit.snapshot.root) {
            continue;
        }
        let amount = exit
            .channels
            .get("opacity")
            .copied()
            .unwrap_or(1.0)
            .clamp(0.0, 1.0) as f32;
        shift_after(
            view,
            &mut result.scene,
            &exit.snapshot.root,
            root.bounds.height * amount,
        );
    }

    // Anchored content follows the trigger's presented bounds during animation.
    for key in scene.order.iter() {
        let Some(node) = view.nodes.get(key) else {
            continue;
        };
        if node.kind != Kind::Overlay || !node.open {
            continue;
        }
        let anchor_bounds = if node.overlay_kind == Some(OverlayKind::ContextMenu) {
            scene.context_points.get(key).map(|(x, y)| Rect {
                x: *x,
                y: *y,
                width: 0.0,
                height: 0.0,
            })
        } else {
            None
        }
        .or_else(|| {
            node.anchor
                .as_deref()
                .and_then(|anchor| result.scene.boxes.get(anchor))
                .map(|layout| layout.bounds)
        });
        let Some(anchor_bounds) = anchor_bounds else {
            continue;
        };
        let Some(overlay_bounds) = result.scene.boxes.get(key).map(|b| b.bounds) else {
            continue;
        };
        let target = place_overlay(
            anchor_bounds,
            (overlay_bounds.width, overlay_bounds.height),
            scene.viewport,
            node.placement,
        );
        shift_subtree(
            view,
            &mut result.scene,
            key,
            target.x - overlay_bounds.x,
            target.y - overlay_bounds.y,
            false,
        );
    }
    result.exits = frame.exits.iter().map(present_exit).collect();
    result
}

fn present_exit(exit: &VisualExit) -> VisualExit {
    let Some(root) = exit
        .snapshot
        .nodes
        .iter()
        .find(|entry| entry.node.key == exit.snapshot.root)
    else {
        return exit.clone();
    };
    let center_x = root.bounds.x + root.bounds.width * 0.5;
    let center_y = root.bounds.y + root.bounds.height * 0.5;
    let scale = exit.channels.get("scale").copied().unwrap_or(1.0) as f32;
    let dx = exit.channels.get("translate-x").copied().unwrap_or(0.0) as f32;
    let dy = exit.channels.get("translate-y").copied().unwrap_or(0.0) as f32;
    let transform = Transform {
        scale,
        x: center_x + dx - scale * center_x,
        y: center_y + dy - scale * center_y,
        opacity: 1.0,
    };
    let mut result = exit.clone();
    for entry in &mut result.snapshot.nodes {
        entry.bounds = transform_rect(entry.bounds, transform);
        entry.clip = transform_rect(entry.clip, transform);
    }
    result
}

fn transform_rect(rect: Rect, transform: Transform) -> Rect {
    Rect {
        x: transform.scale * rect.x + transform.x,
        y: transform.scale * rect.y + transform.y,
        width: (transform.scale * rect.width).max(0.0),
        height: (transform.scale * rect.height).max(0.0),
    }
}

fn shift_subtree(view: &View, scene: &mut Scene, key: &str, dx: f32, dy: f32, shift_clip: bool) {
    if let Some(box_layout) = scene.boxes.get_mut(key) {
        box_layout.bounds.x += dx;
        box_layout.bounds.y += dy;
        if shift_clip {
            box_layout.clip.x += dx;
            box_layout.clip.y += dy;
        }
    }
    if let Some(node) = view.nodes.get(key) {
        let child_clip_moves =
            node.style.clip || node.kind == Kind::ScrollView || node.disclosure.is_some();
        for child in &node.children {
            shift_subtree(view, scene, child, dx, dy, child_clip_moves);
        }
    }
}

fn clip_subtree(view: &View, scene: &mut Scene, key: &str, clip: Rect) {
    if let Some(layout) = scene.boxes.get_mut(key) {
        layout.clip = layout.clip.intersect(clip);
    }
    if let Some(node) = view.nodes.get(key) {
        for child in &node.children {
            clip_subtree(view, scene, child, clip);
        }
    }
}

fn shift_after(view: &View, scene: &mut Scene, key: &str, dy: f32) {
    if dy == 0.0 {
        return;
    }
    let Some((parent, node)) = view
        .nodes
        .iter()
        .find(|(_, node)| node.children.iter().any(|child| child == key))
    else {
        return;
    };
    let Some(index) = node.children.iter().position(|child| child == key) else {
        return;
    };
    for sibling in node.children.iter().skip(index + 1) {
        shift_subtree(view, scene, sibling, 0.0, dy, false);
    }
    let _ = parent;
}

/// Place anchored surfaces with an 8px gap, side flipping and viewport clamping.
pub fn place_overlay(anchor: Rect, size: (f32, f32), viewport: Rect, placement: Placement) -> Rect {
    let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
    let vx = finite(viewport.x);
    let vy = finite(viewport.y);
    let vw = finite(viewport.width).max(0.0);
    let vh = finite(viewport.height).max(0.0);
    let margin = if vw >= 16.0 && vh >= 16.0 { 8.0 } else { 0.0 };
    let max_w = (vw - margin * 2.0).max(0.0);
    let max_h = (vh - margin * 2.0).max(0.0);
    let width = finite(size.0).max(0.0).min(max_w);
    let height = finite(size.1).max(0.0).min(max_h);
    let mut side = placement;
    let a = Rect {
        x: finite(anchor.x),
        y: finite(anchor.y),
        width: finite(anchor.width).max(0.0),
        height: finite(anchor.height).max(0.0),
    };
    let mut x = a.x + (a.width - width) * 0.5;
    let mut y = a.y + a.height + 8.0;
    match side {
        Placement::Center => {
            x = vx + (vw - width) * 0.5;
            y = vy + (vh - height) * 0.5;
        }
        Placement::Top if a.y - height - 8.0 < vy + margin => side = Placement::Bottom,
        Placement::Bottom if a.y + a.height + height + 8.0 > vy + vh - margin => {
            side = Placement::Top
        }
        Placement::Left if a.x - width - 8.0 < vx + margin => side = Placement::Right,
        Placement::Right if a.x + a.width + width + 8.0 > vx + vw - margin => {
            side = Placement::Left
        }
        _ => {}
    }
    match side {
        Placement::Top => y = a.y - height - 8.0,
        Placement::Right => {
            x = a.x + a.width + 8.0;
            y = a.y + (a.height - height) * 0.5;
        }
        Placement::Bottom => y = a.y + a.height + 8.0,
        Placement::Left => {
            x = a.x - width - 8.0;
            y = a.y + (a.height - height) * 0.5;
        }
        Placement::Center => {}
    }
    Rect {
        x: x.clamp(vx + margin, (vx + vw - margin - width).max(vx + margin)),
        y: y.clamp(vy + margin, (vy + vh - margin - height).max(vy + margin)),
        width,
        height,
    }
}

pub(crate) fn place_initial_overlays(view: &View, scene: &mut Scene) {
    let mut toast_index = 0;
    for key in scene.order.clone() {
        let Some(node) = view.nodes.get(&key) else {
            continue;
        };
        if node.kind != Kind::Overlay || !node.open {
            continue;
        }
        let Some(old) = scene.boxes.get(&key).cloned() else {
            continue;
        };
        let target = match node.overlay_kind {
            Some(OverlayKind::Toast) => {
                let width = old.bounds.width.max(0.0).min(scene.viewport.width.max(0.0));
                let height = old
                    .bounds
                    .height
                    .max(0.0)
                    .min(scene.viewport.height.max(0.0));
                let rect = Rect {
                    x: (scene.viewport.x + scene.viewport.width - 8.0 - width)
                        .max(scene.viewport.x),
                    y: (scene.viewport.y + 8.0 + toast_index as f32 * (height + 8.0))
                        .min(scene.viewport.y + scene.viewport.height - height),
                    width,
                    height,
                };
                toast_index += 1;
                rect
            }
            _ => {
                let pointer_anchor = (node.overlay_kind == Some(OverlayKind::ContextMenu))
                    .then(|| scene.context_points.get(&key))
                    .flatten()
                    .map(|(x, y)| Rect {
                        x: *x,
                        y: *y,
                        width: 0.0,
                        height: 0.0,
                    });
                let anchor = pointer_anchor
                    .or_else(|| {
                        node.anchor
                            .as_deref()
                            .and_then(|key| scene.boxes.get(key))
                            .map(|b| b.bounds)
                    })
                    .unwrap_or(scene.viewport);
                place_overlay(
                    anchor,
                    (old.bounds.width, old.bounds.height),
                    scene.viewport,
                    node.placement,
                )
            }
        };
        let dx = target.x - old.bounds.x;
        let dy = target.y - old.bounds.y;
        if node
            .anchor
            .as_deref()
            .is_some_and(|anchor| !scene.boxes.contains_key(anchor))
        {
            remove_subtree(view, scene, &key);
        } else {
            shift_subtree(view, scene, &key, dx, dy, false);
        }
    }
}

fn remove_subtree(view: &View, scene: &mut Scene, key: &str) {
    scene.boxes.remove(key);
    scene.order.retain(|item| item != key);
    if let Some(node) = view.nodes.get(key) {
        for child in &node.children {
            remove_subtree(view, scene, child);
        }
    }
}
