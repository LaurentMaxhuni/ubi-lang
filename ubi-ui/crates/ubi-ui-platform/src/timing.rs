use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use ubi_ui_core::{Effect, Kind, OverlayKind, Rect, View};
use ubi_ui_input::{Event, Input};
use ubi_ui_layout::Scene;

const DEFAULT_ROTATION_PERIOD_MS: u64 = 3_000;
const TOOLTIP_HOVER_DELAY_MS: u64 = 350;

#[derive(Default)]
pub(crate) struct TimingState {
    now_ms: u64,
    toasts: BTreeMap<String, ToastTimer>,
    rotations: BTreeMap<String, RotationTimer>,
    tooltips: BTreeMap<String, TooltipTimer>,
}

impl TimingState {
    pub(crate) fn sync(
        &mut self,
        view: &View,
        scene: &Scene,
        input: &Input,
        now_ms: u64,
        host_active: bool,
        reduced: bool,
    ) {
        let now_ms = self.observe_now(now_ms);
        self.sync_toasts(view, scene, input, now_ms, host_active);
        self.sync_rotations(view, scene, input, now_ms, host_active, reduced);
        self.sync_tooltips(view, scene, input, now_ms, host_active);
    }

    pub(crate) fn poll(&mut self, view: &View, now_ms: u64) -> Vec<Event> {
        let now_ms = self.observe_now(now_ms);
        let mut events = Vec::new();

        self.toasts.retain(|key, timer| {
            view.nodes.get(key).is_some_and(|node| {
                node.kind == Kind::Overlay
                    && node.overlay_kind == Some(OverlayKind::Toast)
                    && node.open
                    && node.timeout_ms == timer.timeout_ms
                    && node.dismiss.as_ref() == Some(&timer.dismiss)
            })
        });
        self.rotations.retain(|key, timer| {
            view.nodes.get(key).is_some_and(|node| {
                node.effect == Some(Effect::RotatingText)
                    && node.auto_play
                    && node.advance.as_ref() == Some(&timer.advance)
                    && rotation_period(node.period_ms) == timer.period_ms
            })
        });
        self.tooltips.retain(|key, timer| {
            view.nodes.get(key).is_some_and(|node| {
                node.kind == Kind::Overlay
                    && node.overlay_kind == Some(OverlayKind::Tooltip)
                    && node.anchor.as_deref() == Some(timer.anchor.as_str())
                    && node.open_request == timer.open_request
                    && node.dismiss == timer.dismiss
                    && node.open == timer.open
            })
        });

        for key in &view.order {
            let Some(node) = view.nodes.get(key) else {
                continue;
            };

            let toast_current = self.toasts.get(key).is_some_and(|timer| {
                node.kind == Kind::Overlay
                    && node.overlay_kind == Some(OverlayKind::Toast)
                    && node.open
                    && node.timeout_ms == timer.timeout_ms
                    && node.dismiss.as_ref() == Some(&timer.dismiss)
            });
            if !toast_current {
                self.toasts.remove(key);
            } else if let Some(timer) = self.toasts.get_mut(key) {
                if timer
                    .countdown
                    .deadline_ms
                    .is_some_and(|deadline| deadline <= now_ms)
                {
                    timer.countdown.deadline_ms = None;
                    timer.countdown.remaining_ms = 0;
                    timer.fired = true;
                    events.push(activate_event(key, &timer.dismiss));
                }
            }

            let rotation_current = self.rotations.get(key).is_some_and(|timer| {
                node.effect == Some(Effect::RotatingText)
                    && node.auto_play
                    && node.advance.as_ref() == Some(&timer.advance)
                    && rotation_period(node.period_ms) == timer.period_ms
            });
            if !rotation_current {
                self.rotations.remove(key);
            } else if let Some(timer) = self.rotations.get_mut(key) {
                if node.paused
                    || node.disabled
                    || node.busy
                    || node.motion.is_some_and(|motion| motion.duration_ms == 0)
                {
                    timer.countdown.pause_preserving_remaining(now_ms);
                } else if timer
                    .countdown
                    .deadline_ms
                    .is_some_and(|deadline| deadline <= now_ms)
                {
                    events.push(activate_event(key, &timer.advance));
                    timer.countdown.start_period(now_ms, timer.period_ms);
                }
            }

            let tooltip_current = self.tooltips.get(key).is_some_and(|timer| {
                node.kind == Kind::Overlay
                    && node.overlay_kind == Some(OverlayKind::Tooltip)
                    && node.anchor.as_deref() == Some(timer.anchor.as_str())
                    && node.open_request == timer.open_request
                    && node.dismiss == timer.dismiss
                    && node.open == timer.open
            });
            if !tooltip_current {
                self.tooltips.remove(key);
            } else if let Some(timer) = self.tooltips.get_mut(key) {
                if !timer.open
                    && !timer.request_sent
                    && timer
                        .request_deadline_ms
                        .is_some_and(|deadline| deadline <= now_ms)
                {
                    if let Some(message) = timer.open_request.as_ref() {
                        events.push(activate_event(key, message));
                        timer.request_deadline_ms = None;
                        timer.request_sent = true;
                    }
                } else if timer.open
                    && !timer.dismiss_sent
                    && timer
                        .dismiss_deadline_ms
                        .is_some_and(|deadline| deadline <= now_ms)
                {
                    if let Some(message) = timer.dismiss.as_ref() {
                        events.push(activate_event(key, message));
                        timer.dismiss_deadline_ms = None;
                        timer.dismiss_sent = true;
                        timer.engaged = false;
                    }
                }
            }
        }

        events
    }

    pub(crate) fn next_wake_ms(&self) -> Option<u64> {
        self.toasts
            .values()
            .filter_map(|timer| timer.countdown.deadline_ms)
            .chain(
                self.rotations
                    .values()
                    .filter_map(|timer| timer.countdown.deadline_ms),
            )
            .chain(
                self.tooltips
                    .values()
                    .flat_map(|timer| [timer.request_deadline_ms, timer.dismiss_deadline_ms])
                    .flatten(),
            )
            .min()
    }

    fn observe_now(&mut self, now_ms: u64) -> u64 {
        self.now_ms = self.now_ms.max(now_ms);
        self.now_ms
    }

    fn sync_toasts(
        &mut self,
        view: &View,
        scene: &Scene,
        input: &Input,
        now_ms: u64,
        host_active: bool,
    ) {
        let mut active = BTreeSet::new();
        for key in &view.order {
            let Some(node) = view.nodes.get(key) else {
                continue;
            };
            if node.kind != Kind::Overlay
                || node.overlay_kind != Some(OverlayKind::Toast)
                || !node.open
                || node.timeout_ms == 0
            {
                continue;
            }
            let Some(dismiss) = node.dismiss.as_ref() else {
                continue;
            };

            active.insert(key.clone());
            let timer = self
                .toasts
                .entry(key.clone())
                .or_insert_with(|| ToastTimer {
                    dismiss: dismiss.clone(),
                    timeout_ms: node.timeout_ms,
                    countdown: Countdown::new(u64::from(node.timeout_ms)),
                    fired: false,
                });
            if timer.dismiss != *dismiss || timer.timeout_ms != node.timeout_ms {
                *timer = ToastTimer {
                    dismiss: dismiss.clone(),
                    timeout_ms: node.timeout_ms,
                    countdown: Countdown::new(u64::from(node.timeout_ms)),
                    fired: false,
                };
            }

            let paused = !host_active
                || visible_rect(scene, key).is_none()
                || pointer_over_subtree(view, scene, input, key)
                || focus_within(view, input, key);
            if !timer.fired {
                timer.countdown.set_paused(now_ms, paused);
            }
        }
        self.toasts.retain(|key, _| active.contains(key));
    }

    fn sync_rotations(
        &mut self,
        view: &View,
        scene: &Scene,
        input: &Input,
        now_ms: u64,
        host_active: bool,
        reduced: bool,
    ) {
        let mut active = BTreeSet::new();
        for key in &view.order {
            let Some(node) = view.nodes.get(key) else {
                continue;
            };
            if node.effect != Some(Effect::RotatingText) || !node.auto_play {
                continue;
            }
            let Some(advance) = node.advance.as_ref() else {
                continue;
            };

            active.insert(key.clone());
            let period_ms = rotation_period(node.period_ms);
            let timer = self
                .rotations
                .entry(key.clone())
                .or_insert_with(|| RotationTimer {
                    advance: advance.clone(),
                    period_ms,
                    countdown: Countdown::new(period_ms),
                });
            if timer.advance != *advance || timer.period_ms != period_ms {
                *timer = RotationTimer {
                    advance: advance.clone(),
                    period_ms,
                    countdown: Countdown::new(period_ms),
                };
            }

            let paused = node.paused
                || node.disabled
                || node.busy
                || node.motion.is_some_and(|motion| motion.duration_ms == 0)
                || reduced
                || !host_active
                || visible_rect(scene, key).is_none()
                || pointer_over_subtree(view, scene, input, key)
                || focus_within(view, input, key);
            if paused {
                timer.countdown.pause_preserving_remaining(now_ms);
            } else {
                timer.countdown.resume_preserving_remaining(now_ms);
            }
        }
        self.rotations.retain(|key, _| active.contains(key));
    }

    fn sync_tooltips(
        &mut self,
        view: &View,
        scene: &Scene,
        input: &Input,
        now_ms: u64,
        host_active: bool,
    ) {
        let mut active = BTreeSet::new();
        for key in &view.order {
            let Some(node) = view.nodes.get(key) else {
                continue;
            };
            if node.kind != Kind::Overlay || node.overlay_kind != Some(OverlayKind::Tooltip) {
                continue;
            }
            let Some(anchor) = node.anchor.as_ref() else {
                continue;
            };
            if !view.nodes.contains_key(anchor) {
                continue;
            }

            active.insert(key.clone());
            let timer = self.tooltips.entry(key.clone()).or_insert_with(|| {
                TooltipTimer::new(
                    anchor.clone(),
                    node.open_request.clone(),
                    node.dismiss.clone(),
                    node.open,
                )
            });
            if timer.anchor != *anchor
                || timer.open_request != node.open_request
                || timer.dismiss != node.dismiss
            {
                *timer = TooltipTimer::new(
                    anchor.clone(),
                    node.open_request.clone(),
                    node.dismiss.clone(),
                    node.open,
                );
            } else if timer.open != node.open {
                let engaged = !timer.open && node.open && timer.engaged;
                timer.reset_generation(node.open);
                timer.engaged = engaged;
            }

            let anchor_focused = focus_within(view, input, anchor);
            let anchor_hovered = host_active && pointer_over_subtree(view, scene, input, anchor);
            if node.open {
                let tooltip_hovered = pointer_over_subtree(view, scene, input, key);
                let inside = anchor_focused || anchor_hovered || tooltip_hovered;
                timer.request_deadline_ms = None;
                if inside {
                    timer.engaged = true;
                    timer.dismiss_deadline_ms = None;
                    timer.dismiss_sent = false;
                } else if timer.engaged && !timer.dismiss_sent && timer.dismiss.is_some() {
                    timer.engaged = false;
                    timer.dismiss_deadline_ms = Some(now_ms);
                }
            } else {
                timer.dismiss_deadline_ms = None;
                timer.dismiss_sent = false;
                let anchor_active = host_active && (anchor_focused || anchor_hovered);
                if !anchor_active {
                    timer.request_deadline_ms = None;
                    timer.request_sent = false;
                    timer.engaged = false;
                } else {
                    timer.engaged = true;
                    if !timer.request_sent && timer.open_request.is_some() {
                        if anchor_focused {
                            timer.request_deadline_ms = Some(now_ms);
                            timer.request_is_hover = false;
                        } else if timer.request_deadline_ms.is_none() || !timer.request_is_hover {
                            timer.request_deadline_ms =
                                future_deadline(now_ms, TOOLTIP_HOVER_DELAY_MS);
                            timer.request_is_hover = true;
                        }
                    }
                }
            }
        }
        self.tooltips.retain(|key, _| active.contains(key));
    }
}

struct Countdown {
    deadline_ms: Option<u64>,
    remaining_ms: u64,
    paused: bool,
}

impl Countdown {
    fn new(duration_ms: u64) -> Self {
        Self {
            deadline_ms: None,
            remaining_ms: duration_ms,
            paused: true,
        }
    }

    fn set_paused(&mut self, now_ms: u64, paused: bool) {
        if self
            .deadline_ms
            .is_some_and(|deadline_ms| deadline_ms <= now_ms)
        {
            self.paused = paused;
            return;
        }
        if self.paused == paused {
            return;
        }
        self.paused = paused;
        if paused {
            if let Some(deadline_ms) = self.deadline_ms.take() {
                self.remaining_ms = deadline_ms.saturating_sub(now_ms);
            }
        } else {
            self.deadline_ms = future_deadline(now_ms, self.remaining_ms);
        }
    }

    fn pause_preserving_remaining(&mut self, now_ms: u64) {
        if let Some(deadline_ms) = self.deadline_ms.take() {
            self.remaining_ms = deadline_ms.saturating_sub(now_ms);
        }
        self.paused = true;
    }

    fn resume_preserving_remaining(&mut self, now_ms: u64) {
        if !self.paused {
            return;
        }
        self.paused = false;
        self.deadline_ms = if self.remaining_ms == 0 {
            Some(now_ms)
        } else {
            future_deadline(now_ms, self.remaining_ms)
        };
    }

    fn start_period(&mut self, now_ms: u64, period_ms: u64) {
        self.remaining_ms = period_ms;
        if self.paused {
            self.deadline_ms = None;
        } else {
            self.deadline_ms = future_deadline(now_ms, period_ms);
        }
    }
}

struct ToastTimer {
    dismiss: Value,
    timeout_ms: u32,
    countdown: Countdown,
    fired: bool,
}

struct RotationTimer {
    advance: Value,
    period_ms: u64,
    countdown: Countdown,
}

struct TooltipTimer {
    anchor: String,
    open_request: Option<Value>,
    dismiss: Option<Value>,
    open: bool,
    request_deadline_ms: Option<u64>,
    request_sent: bool,
    request_is_hover: bool,
    dismiss_deadline_ms: Option<u64>,
    dismiss_sent: bool,
    engaged: bool,
}

impl TooltipTimer {
    fn new(
        anchor: String,
        open_request: Option<Value>,
        dismiss: Option<Value>,
        open: bool,
    ) -> Self {
        Self {
            anchor,
            open_request,
            dismiss,
            open,
            request_deadline_ms: None,
            request_sent: false,
            request_is_hover: false,
            dismiss_deadline_ms: None,
            dismiss_sent: false,
            engaged: false,
        }
    }

    fn reset_generation(&mut self, open: bool) {
        self.open = open;
        self.request_deadline_ms = None;
        self.request_sent = false;
        self.request_is_hover = false;
        self.dismiss_deadline_ms = None;
        self.dismiss_sent = false;
        self.engaged = false;
    }
}

fn rotation_period(period_ms: u32) -> u64 {
    if period_ms == 0 {
        DEFAULT_ROTATION_PERIOD_MS
    } else {
        u64::from(period_ms)
    }
}

fn future_deadline(now_ms: u64, duration_ms: u64) -> Option<u64> {
    let deadline_ms = now_ms.saturating_add(duration_ms);
    (deadline_ms > now_ms).then_some(deadline_ms)
}

fn activate_event(key: &str, message: &Value) -> Event {
    Event {
        key: key.to_owned(),
        value: json!({"tag":"Activate", "values":[message]}),
    }
}

fn visible_rect(scene: &Scene, key: &str) -> Option<Rect> {
    let layout = scene.boxes.get(key)?;
    let visible = layout
        .bounds
        .intersect(layout.clip)
        .intersect(scene.viewport);
    (visible.x.is_finite()
        && visible.y.is_finite()
        && visible.width.is_finite()
        && visible.height.is_finite()
        && visible.width > 0.0
        && visible.height > 0.0)
        .then_some(visible)
}

fn pointer_over_subtree(view: &View, scene: &Scene, input: &Input, root: &str) -> bool {
    if !input.pointer_inside {
        return false;
    }
    if input
        .hovered
        .as_deref()
        .is_some_and(|hovered| is_in_subtree(view, root, hovered))
        && input
            .hovered
            .as_deref()
            .is_some_and(|hovered| visible_rect(scene, hovered).is_some())
    {
        return true;
    }
    visible_rect(scene, root)
        .is_some_and(|bounds| bounds.contains(input.pointer.0, input.pointer.1))
}

fn focus_within(view: &View, input: &Input, root: &str) -> bool {
    input
        .focused
        .as_deref()
        .is_some_and(|focused| is_in_subtree(view, root, focused))
}

fn is_in_subtree(view: &View, root: &str, candidate: &str) -> bool {
    let mut pending = vec![root.to_owned()];
    let mut visited = BTreeSet::new();
    while let Some(key) = pending.pop() {
        if !visited.insert(key.clone()) {
            continue;
        }
        if key == candidate {
            return true;
        }
        if let Some(node) = view.nodes.get(&key) {
            pending.extend(node.children.iter().cloned());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use ubi_ui_layout::BoxLayout;

    fn message(tag: &str) -> Value {
        json!({"tag":tag, "values":[]})
    }

    fn decode(nodes: Value) -> View {
        View::decode(json!({"schemaVersion":1, "root":"root", "nodes":nodes}))
            .expect("test view should decode")
    }

    fn toast_view(open: bool, timeout_ms: u32) -> View {
        decode(json!([
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["toast"]},
            {"key":"toast","kind":{"tag":"Overlay","values":[]},"props":[
                {"tag":"OverlayKind","values":[{"tag":"Toast","values":[]}]},
                {"tag":"Open","values":[open]},
                {"tag":"Label","values":["Notice"]},
                {"tag":"TimeoutMs","values":[timeout_ms]},
                {"tag":"Dismiss","values":[message("Dismiss")]}
            ],"children":["body"]},
            {"key":"body","kind":{"tag":"Text","values":[]},"props":[{"tag":"Content","values":["Saved"]}],"children":[]}
        ]))
    }

    fn dialog_view() -> View {
        decode(json!([
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["toast"]},
            {"key":"toast","kind":{"tag":"Overlay","values":[]},"props":[
                {"tag":"OverlayKind","values":[{"tag":"Dialog","values":[]}]},
                {"tag":"Open","values":[true]},
                {"tag":"Label","values":["Details"]}
            ],"children":[]}
        ]))
    }

    fn empty_view() -> View {
        decode(json!([
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":[]}
        ]))
    }

    fn rotating_view(paused: bool, period_ms: Option<u32>) -> View {
        let mut props = vec![
            json!({"tag":"Effect","values":[{"tag":"RotatingText","values":[]}]}),
            json!({"tag":"AutoPlay","values":[true]}),
            json!({"tag":"Paused","values":[paused]}),
            json!({"tag":"Advance","values":[message("Advance")] }),
        ];
        if let Some(period_ms) = period_ms {
            props.push(json!({"tag":"PeriodMs","values":[period_ms]}));
        }
        decode(json!([
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["rotating"]},
            {"key":"rotating","kind":{"tag":"EffectSurface","values":[]},"props":props,"children":["phrase"]},
            {"key":"phrase","kind":{"tag":"Text","values":[]},"props":[{"tag":"Content","values":["One"]}],"children":[]}
        ]))
    }

    fn rotating_motion_view(duration_ms: u32, period_ms: u32) -> View {
        let mut view = rotating_view(false, Some(period_ms));
        view.nodes.get_mut("rotating").unwrap().motion = Some(ubi_ui_core::Motion {
            duration_ms,
            delay_ms: 0,
            easing: ubi_ui_core::Easing::Linear,
        });
        view
    }

    fn tooltip_view(open: bool) -> View {
        decode(json!([
            {"key":"root","kind":{"tag":"Window","values":[]},"props":[],"children":["anchor","tooltip"]},
            {"key":"anchor","kind":{"tag":"Button","values":[]},"props":[{"tag":"Label","values":["Help"]}],"children":[]},
            {"key":"tooltip","kind":{"tag":"Overlay","values":[]},"props":[
                {"tag":"OverlayKind","values":[{"tag":"Tooltip","values":[]}]},
                {"tag":"Anchor","values":["anchor"]},
                {"tag":"Open","values":[open]},
                {"tag":"Label","values":["More information"]},
                {"tag":"OpenRequest","values":[message("Open")]},
                {"tag":"Dismiss","values":[message("Dismiss")]}
            ],"children":[]}
        ]))
    }

    fn scene(keys: &[&str]) -> Scene {
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let bounds = Rect {
            x: 10.0,
            y: 10.0,
            width: 160.0,
            height: 40.0,
        };
        let mut scene = Scene {
            viewport,
            ..Scene::default()
        };
        for key in keys {
            scene.boxes.insert(
                (*key).to_owned(),
                BoxLayout {
                    bounds,
                    clip: viewport,
                    content_height: bounds.height,
                },
            );
            scene.order.push((*key).to_owned());
        }
        scene
    }

    fn tooltip_scene() -> Scene {
        let mut scene = scene(&["root", "anchor", "tooltip"]);
        scene.boxes.get_mut("anchor").unwrap().bounds = Rect {
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 24.0,
        };
        scene.boxes.get_mut("tooltip").unwrap().bounds = Rect {
            x: 110.0,
            y: 10.0,
            width: 120.0,
            height: 40.0,
        };
        scene
    }

    fn assert_activate(event: &Event, key: &str, tag: &str) {
        assert_eq!(event.key, key);
        assert_eq!(
            event.value,
            json!({"tag":"Activate", "values":[message(tag)]})
        );
    }

    #[test]
    fn toast_deadline_is_absolute_and_fires_once_even_if_source_stays_open() {
        let view = toast_view(true, 100);
        let scene = scene(&["root", "toast", "body"]);
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &Input::default(), 7, true, false);

        assert_eq!(timing.next_wake_ms(), Some(107));
        assert!(timing.poll(&view, 106).is_empty());
        let events = timing.poll(&view, 107);
        assert_eq!(events.len(), 1);
        assert_activate(&events[0], "toast", "Dismiss");
        assert!(timing.poll(&view, 1_000).is_empty());
        assert_eq!(timing.next_wake_ms(), None);
    }

    #[test]
    fn toast_timeout_zero_is_manual_and_pause_preserves_remaining_time() {
        let scene = scene(&["root", "toast", "body"]);
        let manual = toast_view(true, 0);
        let mut timing = TimingState::default();
        timing.sync(&manual, &scene, &Input::default(), 0, true, false);
        assert_eq!(timing.next_wake_ms(), None);
        assert!(timing.poll(&manual, 10_000).is_empty());

        let view = toast_view(true, 100);
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &Input::default(), 0, true, false);
        let mut hovered = Input::default();
        hovered.hovered = Some("body".into());
        hovered.pointer_inside = true;
        hovered.pointer = (20.0, 20.0);
        timing.sync(&view, &scene, &hovered, 40, true, false);
        assert_eq!(timing.next_wake_ms(), None);

        hovered.hovered = None;
        hovered.pointer_inside = false;
        timing.sync(&view, &scene, &hovered, 500, false, false);
        assert_eq!(timing.next_wake_ms(), None);
        timing.sync(&view, &scene, &hovered, 800, true, false);
        assert_eq!(timing.next_wake_ms(), Some(860));
        let mut focused = Input::default();
        focused.focused = Some("body".into());
        timing.sync(&view, &scene, &focused, 820, true, false);
        assert_eq!(timing.next_wake_ms(), None);
        timing.sync(&view, &scene, &Input::default(), 1_000, true, false);
        assert_eq!(timing.next_wake_ms(), Some(1_040));
        assert!(timing.poll(&view, 1_039).is_empty());
        let events = timing.poll(&view, 1_040);
        assert_eq!(events.len(), 1);
        assert_activate(&events[0], "toast", "Dismiss");
    }

    #[test]
    fn toast_close_removal_and_kind_replacement_cancel_old_generation() {
        let open = toast_view(true, 20);
        let closed = toast_view(false, 20);
        let scene = scene(&["root", "toast", "body"]);
        let mut timing = TimingState::default();
        timing.sync(&open, &scene, &Input::default(), 0, true, false);
        timing.sync(&closed, &scene, &Input::default(), 5, true, false);
        timing.sync(&open, &scene, &Input::default(), 8, true, false);
        assert!(timing.poll(&open, 20).is_empty());
        assert_eq!(timing.poll(&open, 28).len(), 1);

        let mut timing = TimingState::default();
        timing.sync(&open, &scene, &Input::default(), 0, true, false);
        timing.sync(&empty_view(), &scene, &Input::default(), 5, true, false);
        timing.sync(&open, &scene, &Input::default(), 8, true, false);
        assert!(timing.poll(&open, 20).is_empty());
        assert_eq!(timing.poll(&open, 28).len(), 1);

        let mut timing = TimingState::default();
        timing.sync(&open, &scene, &Input::default(), 0, true, false);
        timing.sync(&dialog_view(), &scene, &Input::default(), 5, true, false);
        timing.sync(&open, &scene, &Input::default(), 8, true, false);
        assert!(timing.poll(&open, 20).is_empty());
        assert_eq!(timing.poll(&open, 28).len(), 1);
    }

    #[test]
    fn toast_suppresses_a_due_event_when_current_source_closed() {
        let open = toast_view(true, 10);
        let closed = toast_view(false, 10);
        let scene = scene(&["root", "toast", "body"]);
        let mut timing = TimingState::default();
        timing.sync(&open, &scene, &Input::default(), 0, true, false);
        timing.sync(&closed, &scene, &Input::default(), 5, true, false);
        assert!(timing.poll(&closed, 50).is_empty());
    }

    #[test]
    fn toast_clock_does_not_move_backwards() {
        let view = toast_view(true, 100);
        let scene = scene(&["root", "toast", "body"]);
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &Input::default(), 10, true, false);
        assert!(timing.poll(&view, 90).is_empty());
        assert!(timing.poll(&view, 20).is_empty());
        assert_eq!(timing.next_wake_ms(), Some(110));
        assert_eq!(timing.poll(&view, 110).len(), 1);
    }

    #[test]
    fn rotating_text_uses_default_period_and_source_remains_authoritative() {
        let view = rotating_view(false, None);
        let scene = scene(&["root", "rotating", "phrase"]);
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &Input::default(), 0, true, false);

        assert_eq!(timing.next_wake_ms(), Some(3_000));
        assert!(timing.poll(&view, 2_999).is_empty());
        let events = timing.poll(&view, 3_000);
        assert_eq!(events.len(), 1);
        assert_activate(&events[0], "rotating", "Advance");
        assert_eq!(view.nodes["rotating"].index, 0);
        assert_eq!(timing.next_wake_ms(), Some(6_000));
        assert!(timing.poll(&view, 3_000).is_empty());
    }

    #[test]
    fn rotation_pauses_for_source_pause_visibility_host_and_reduced_motion() {
        let view = rotating_view(false, Some(1_000));
        let paused = rotating_view(true, Some(1_000));
        let mut scene = scene(&["root", "rotating", "phrase"]);
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &Input::default(), 0, true, false);

        scene.boxes.get_mut("rotating").unwrap().bounds.x = 900.0;
        timing.sync(&view, &scene, &Input::default(), 200, true, false);
        scene.boxes.get_mut("rotating").unwrap().bounds.x = 10.0;
        timing.sync(&view, &scene, &Input::default(), 400, true, true);
        timing.sync(&view, &scene, &Input::default(), 600, false, false);
        let mut focused = Input::default();
        focused.focused = Some("phrase".into());
        timing.sync(&view, &scene, &focused, 800, true, false);
        timing.sync(&paused, &scene, &Input::default(), 1_000, true, false);
        assert!(timing.poll(&paused, 5_000).is_empty());

        timing.sync(&view, &scene, &Input::default(), 5_000, true, false);
        assert_eq!(timing.next_wake_ms(), Some(5_800));
        assert!(timing.poll(&view, 5_799).is_empty());
        assert_eq!(timing.poll(&view, 5_800).len(), 1);
    }

    #[test]
    fn rotation_poll_revalidates_pause_disabled_and_busy_without_losing_time() {
        let view = rotating_view(false, Some(1_000));
        let scene = scene(&["root", "rotating", "phrase"]);
        for blocked_by in ["paused", "disabled", "busy"] {
            let mut blocked = rotating_view(false, Some(1_000));
            let node = blocked.nodes.get_mut("rotating").unwrap();
            match blocked_by {
                "paused" => node.paused = true,
                "disabled" => node.disabled = true,
                "busy" => node.busy = true,
                _ => unreachable!(),
            }
            let mut timing = TimingState::default();
            timing.sync(&view, &scene, &Input::default(), 0, true, false);

            assert!(timing.poll(&blocked, 400).is_empty(), "{blocked_by}");
            assert_eq!(timing.next_wake_ms(), None, "{blocked_by}");
            timing.sync(&view, &scene, &Input::default(), 500, true, false);
            assert_eq!(timing.next_wake_ms(), Some(1_100), "{blocked_by}");
            assert!(timing.poll(&view, 1_099).is_empty(), "{blocked_by}");
            assert_eq!(timing.poll(&view, 1_100).len(), 1, "{blocked_by}");
        }
    }

    #[test]
    fn explicit_zero_motion_keeps_new_rotation_timer_static() {
        let static_view = rotating_motion_view(0, 1_000);
        let active_view = rotating_view(false, Some(1_000));
        let scene = scene(&["root", "rotating", "phrase"]);
        let mut timing = TimingState::default();
        timing.sync(&static_view, &scene, &Input::default(), 0, true, false);

        assert_eq!(timing.next_wake_ms(), None);
        assert!(timing.poll(&static_view, 5_000).is_empty());
        timing.sync(&active_view, &scene, &Input::default(), 5_000, true, false);
        assert_eq!(timing.next_wake_ms(), Some(6_000));
        assert!(timing.poll(&active_view, 5_999).is_empty());
        assert_eq!(timing.poll(&active_view, 6_000).len(), 1);
    }

    #[test]
    fn zero_motion_pauses_active_rotation_without_losing_remaining_time() {
        let active_view = rotating_view(false, Some(1_000));
        let static_view = rotating_motion_view(0, 1_000);
        let scene = scene(&["root", "rotating", "phrase"]);
        let mut timing = TimingState::default();
        timing.sync(&active_view, &scene, &Input::default(), 0, true, false);

        assert!(timing.poll(&static_view, 400).is_empty());
        assert_eq!(timing.next_wake_ms(), None);
        timing.sync(&active_view, &scene, &Input::default(), 500, true, false);
        assert_eq!(timing.next_wake_ms(), Some(1_100));
        assert!(timing.poll(&active_view, 1_099).is_empty());
        assert_eq!(timing.poll(&active_view, 1_100).len(), 1);
    }

    #[test]
    fn rotation_poll_suppresses_due_event_when_current_source_is_paused() {
        let view = rotating_view(false, Some(1_000));
        let paused = rotating_view(true, Some(1_000));
        let scene = scene(&["root", "rotating", "phrase"]);
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &Input::default(), 0, true, false);

        assert!(timing.poll(&paused, 1_000).is_empty());
        assert_eq!(timing.next_wake_ms(), None);
        timing.sync(&view, &scene, &Input::default(), 2_000, true, false);
        assert_eq!(timing.next_wake_ms(), Some(2_000));
        let events = timing.poll(&view, 2_000);
        assert_eq!(events.len(), 1);
        assert_activate(&events[0], "rotating", "Advance");
    }

    #[test]
    fn tooltip_pointer_request_waits_350ms_and_focus_requests_immediately() {
        let view = tooltip_view(false);
        let scene = tooltip_scene();
        let mut hovered = Input::default();
        hovered.hovered = Some("anchor".into());
        hovered.pointer_inside = true;
        hovered.pointer = (20.0, 20.0);
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &hovered, 0, true, false);
        assert_eq!(timing.next_wake_ms(), Some(350));
        assert!(timing.poll(&view, 349).is_empty());
        let events = timing.poll(&view, 350);
        assert_eq!(events.len(), 1);
        assert_activate(&events[0], "tooltip", "Open");
        assert!(timing.poll(&view, 351).is_empty());

        let mut focused = Input::default();
        focused.focused = Some("anchor".into());
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &focused, 20, true, false);
        let events = timing.poll(&view, 20);
        assert_eq!(events.len(), 1);
        assert_activate(&events[0], "tooltip", "Open");
        timing.sync(&view, &scene, &focused, 21, true, false);
        assert!(timing.poll(&view, 21).is_empty());
    }

    #[test]
    fn tooltip_dismisses_once_only_after_pointer_leaves_anchor_and_surface_and_blurs() {
        let view = tooltip_view(true);
        let scene = tooltip_scene();
        let mut focused_anchor = Input::default();
        focused_anchor.focused = Some("anchor".into());
        focused_anchor.pointer_inside = true;
        focused_anchor.pointer = (20.0, 20.0);
        focused_anchor.hovered = Some("anchor".into());
        let mut timing = TimingState::default();
        timing.sync(&view, &scene, &focused_anchor, 0, true, false);

        let mut over_tooltip = Input::default();
        over_tooltip.focused = Some("anchor".into());
        over_tooltip.pointer_inside = true;
        over_tooltip.pointer = (120.0, 20.0);
        over_tooltip.hovered = Some("tooltip".into());
        timing.sync(&view, &scene, &over_tooltip, 1, true, false);
        let mut blurred_over_tooltip = Input::default();
        blurred_over_tooltip.pointer_inside = true;
        blurred_over_tooltip.pointer = (120.0, 20.0);
        blurred_over_tooltip.hovered = Some("tooltip".into());
        timing.sync(&view, &scene, &blurred_over_tooltip, 2, true, false);
        assert!(timing.poll(&view, 2).is_empty());

        timing.sync(&view, &scene, &Input::default(), 3, true, false);
        assert_eq!(timing.next_wake_ms(), Some(3));
        let events = timing.poll(&view, 3);
        assert_eq!(events.len(), 1);
        assert_activate(&events[0], "tooltip", "Dismiss");
        assert!(timing.poll(&view, 3).is_empty());
    }
}
