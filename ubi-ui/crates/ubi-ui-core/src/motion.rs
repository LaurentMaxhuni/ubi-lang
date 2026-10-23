//! Pure frame sampling and bounded presentation lifecycle.
use crate::{Easing, Error, Kind, Motion, Node, Rect, Transition};
use std::collections::{BTreeMap, BTreeSet};

const MAX_NODES: usize = 4096;
const MAX_CHANNELS: usize = 16;
const FRAME_INTERVAL_MS: u64 = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotNode {
    pub node: Node,
    pub bounds: Rect,
    pub clip: Rect,
}

/// Sanitized drawing data for a removed subtree. It contains no event messages
/// or editable controls, so it can never dispatch or preserve typed input.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualSnapshot {
    pub root: String,
    pub nodes: Vec<SnapshotNode>,
}

impl VisualSnapshot {
    fn sanitized(&self) -> Option<Self> {
        if self.nodes.len() > MAX_NODES || self.root.is_empty() {
            return None;
        }
        let mut by_key = BTreeMap::new();
        for entry in &self.nodes {
            if entry.node.key.is_empty() || by_key.insert(entry.node.key.as_str(), entry).is_some()
            {
                return None;
            }
        }
        let mut excluded = BTreeSet::new();
        let mut pending: Vec<_> = self
            .nodes
            .iter()
            .filter(|entry| entry.node.kind.editable())
            .map(|entry| entry.node.key.as_str())
            .collect();
        while let Some(key) = pending.pop() {
            if !excluded.insert(key) {
                continue;
            }
            let entry = by_key.get(key)?;
            for child in &entry.node.children {
                pending.push(child);
            }
        }
        let retained: BTreeSet<_> = by_key
            .keys()
            .copied()
            .filter(|key| !excluded.contains(key))
            .collect();
        if !retained.contains(self.root.as_str()) {
            return None;
        }
        let mut nodes = Vec::with_capacity(retained.len());
        for entry in &self.nodes {
            if !retained.contains(entry.node.key.as_str()) {
                continue;
            }
            let mut node = entry.node.clone();
            if node
                .children
                .iter()
                .any(|key| !by_key.contains_key(key.as_str()))
            {
                return None;
            }
            node.children.retain(|key| retained.contains(key.as_str()));
            node.value.clear();
            node.activate = None;
            node.input = None;
            node.toggle = None;
            node.change = None;
            node.dismiss = None;
            node.open_request = None;
            node.advance = None;
            node.disabled = true;
            node.busy = false;
            node.motion = None;
            node.enter = Transition::None;
            node.exit = Transition::None;
            nodes.push(SnapshotNode {
                node,
                bounds: entry.bounds,
                clip: entry.clip,
            });
        }
        Some(Self {
            root: self.root.clone(),
            nodes,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MotionTarget {
    pub key: String,
    pub kind: Kind,
    pub channels: BTreeMap<String, f64>,
    pub motion: Motion,
    pub enter: Transition,
    pub exit: Transition,
    pub snapshot: Option<VisualSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisualExit {
    pub snapshot: VisualSnapshot,
    pub channels: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MotionFrame {
    pub channels: BTreeMap<String, BTreeMap<String, f64>>,
    pub exits: Vec<VisualExit>,
    pub active: bool,
    pub next_wake_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Track {
    from: f64,
    to: f64,
    start_ms: u64,
    motion: Motion,
}

#[derive(Clone, Debug)]
struct Instance {
    kind: Kind,
    targets: BTreeMap<String, f64>,
    channels: BTreeMap<String, f64>,
    tracks: BTreeMap<String, Track>,
    motion: Motion,
    exit: Transition,
    snapshot: Option<VisualSnapshot>,
}

#[derive(Clone, Debug)]
struct ExitInstance {
    snapshot: VisualSnapshot,
    channels: BTreeMap<String, f64>,
    tracks: BTreeMap<String, Track>,
    created: u64,
}

#[derive(Clone, Debug, Default)]
pub struct MotionState {
    instances: BTreeMap<String, Instance>,
    exits: Vec<ExitInstance>,
    sequence: u64,
}

impl MotionState {
    /// Apply a new logical tree atomically. Rejected targets leave live tracks intact.
    pub fn commit(
        &mut self,
        targets: &[MotionTarget],
        now_ms: u64,
        reduced: bool,
    ) -> Result<(), Error> {
        validate_targets(targets)?;
        let mut next = BTreeMap::new();
        let mut remaining = std::mem::take(&mut self.instances);
        for target in targets {
            let same_key_different_kind = remaining
                .get(&target.key)
                .is_some_and(|old| old.kind != target.kind);
            self.exits.retain(|exit| {
                exit.snapshot.root != target.key
                    || exit
                        .snapshot
                        .nodes
                        .iter()
                        .find(|entry| entry.node.key == target.key)
                        .is_some_and(|entry| entry.node.kind == target.kind)
            });
            let old = remaining.remove(&target.key);
            if let Some(previous) = old.as_ref().filter(|old| old.kind == target.kind) {
                for channel in ["disclosure-progress", "overlay-progress"] {
                    let was_open = previous.targets.get(channel).copied();
                    let is_open = target.channels.get(channel).copied();
                    if was_open.is_some_and(|from| is_open.is_some_and(|to| from > to)) {
                        if let Some(exit) =
                            make_exit_with(previous, Transition::Fade, now_ms, reduced)
                        {
                            self.sequence = self.sequence.saturating_add(1);
                            self.exits.push(ExitInstance {
                                created: self.sequence,
                                ..exit
                            });
                        }
                    } else if was_open.is_some_and(|from| is_open.is_some_and(|to| from < to)) {
                        self.exits.retain(|exit| exit.snapshot.root != target.key);
                    }
                }
            }
            let resurrected = if old.is_none() && !same_key_different_kind {
                self.exits
                    .iter()
                    .rposition(|exit| {
                        exit.snapshot.root == target.key
                            && exit
                                .snapshot
                                .nodes
                                .iter()
                                .find(|entry| entry.node.key == target.key)
                                .is_some_and(|entry| entry.node.kind == target.kind)
                    })
                    .map(|index| {
                        let mut exit = self.exits.remove(index);
                        sample_tracks(&mut exit.channels, &mut exit.tracks, now_ms);
                        exit
                    })
                    .filter(|exit| !exit.tracks.is_empty())
            } else {
                None
            };
            let instance = if let Some(old) = old.filter(|old| old.kind == target.kind) {
                retarget(target, &old, now_ms, reduced)
            } else if let Some(exit) = resurrected {
                retarget_from_channels(target, &exit.channels, now_ms, reduced)
            } else {
                retarget_new(target, now_ms, reduced)
            };
            next.insert(target.key.clone(), instance);
        }
        for (key, old) in remaining {
            if targets
                .iter()
                .any(|target| target.key == key && target.kind != old.kind)
            {
                continue;
            }
            if let Some(exit) = make_exit(&key, &old, now_ms, reduced) {
                self.sequence = self.sequence.saturating_add(1);
                self.exits.push(ExitInstance {
                    created: self.sequence,
                    ..exit
                });
            }
        }
        self.instances = next;
        if reduced {
            self.exits.clear();
        } else {
            self.trim_exits();
        }
        Ok(())
    }

    pub fn sample(&mut self, now_ms: u64, reduced: bool) -> MotionFrame {
        if reduced {
            for instance in self.instances.values_mut() {
                instance.channels.clone_from(&instance.targets);
                instance.tracks.clear();
            }
            self.exits.clear();
        }
        let mut frame = MotionFrame::default();
        for (key, instance) in &mut self.instances {
            sample_tracks(&mut instance.channels, &mut instance.tracks, now_ms);
            frame
                .channels
                .insert(key.clone(), instance.channels.clone());
            update_wake(&mut frame, &instance.tracks, now_ms);
        }
        self.exits.retain_mut(|exit| {
            sample_tracks(&mut exit.channels, &mut exit.tracks, now_ms);
            if !exit.tracks.is_empty() {
                update_wake(&mut frame, &exit.tracks, now_ms);
                frame.exits.push(VisualExit {
                    snapshot: exit.snapshot.clone(),
                    channels: exit.channels.clone(),
                });
                true
            } else {
                false
            }
        });
        frame.active = frame
            .channels
            .keys()
            .any(|key| !self.instances[key].tracks.is_empty())
            || !self.exits.is_empty();
        if frame.active && frame.next_wake_ms.is_none() {
            frame.next_wake_ms = Some(now_ms.saturating_add(FRAME_INTERVAL_MS));
        }
        frame
    }

    pub fn dispose(&mut self) {
        self.instances.clear();
        self.exits.clear();
    }

    fn trim_exits(&mut self) {
        while self
            .exits
            .iter()
            .map(|exit| exit.snapshot.nodes.len())
            .sum::<usize>()
            > MAX_NODES
        {
            let Some((index, _)) = self
                .exits
                .iter()
                .enumerate()
                .min_by_key(|(_, exit)| exit.created)
            else {
                break;
            };
            self.exits.remove(index);
        }
    }
}

fn validate_targets(targets: &[MotionTarget]) -> Result<(), Error> {
    if targets.len() > MAX_NODES {
        return Err("motion target node limit exceeded".into());
    }
    let mut keys = BTreeSet::new();
    for target in targets {
        if target.key.is_empty() || target.key.len() > 256 || !keys.insert(&target.key) {
            return Err("invalid or duplicate motion target key".into());
        }
        if target.motion.duration_ms > 2000 || target.motion.delay_ms > 1000 {
            return Err("motion target timing is out of range".into());
        }
        if target.channels.len() > MAX_CHANNELS {
            return Err("motion channel limit exceeded".into());
        }
        for (name, value) in &target.channels {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || !valid_channel_value(name, *value)
            {
                return Err("invalid motion channel".into());
            }
        }
        if let Some(snapshot) = &target.snapshot {
            if snapshot.nodes.len() > MAX_NODES
                || snapshot.nodes.iter().any(|entry| {
                    !valid_rect(entry.bounds)
                        || !valid_rect(entry.clip)
                        || entry.node.key.is_empty()
                        || entry.node.key.len() > 256
                })
                || !snapshot
                    .nodes
                    .iter()
                    .any(|entry| entry.node.key == snapshot.root)
                || snapshot.root != target.key
                || snapshot
                    .nodes
                    .iter()
                    .map(|entry| entry.node.key.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    != snapshot.nodes.len()
            {
                return Err("invalid visual snapshot".into());
            }
            let keys: BTreeSet<_> = snapshot
                .nodes
                .iter()
                .map(|entry| entry.node.key.as_str())
                .collect();
            if snapshot.nodes.iter().any(|entry| {
                entry
                    .node
                    .children
                    .iter()
                    .any(|child| !keys.contains(child.as_str()))
            }) {
                return Err("invalid visual snapshot".into());
            }
        }
    }
    Ok(())
}

fn valid_rect(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width >= 0.0
        && rect.height >= 0.0
}

fn valid_channel_value(name: &str, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    match name {
        "opacity" => (0.0..=1.0).contains(&value),
        "scale" => (0.25..=2.0).contains(&value),
        "switch-thumb"
        | "progress-fill"
        | "tab-selection"
        | "text-reveal-progress"
        | "skeleton-phase"
        | "scroll-reveal-progress"
        | "disclosure-progress"
        | "overlay-progress" => (0.0..=1.0).contains(&value),
        "tab-indicator-width" => (0.0..=100_000.0).contains(&value),
        "ticker-value" => (i32::MIN as f64..=i32::MAX as f64).contains(&value),
        "translate-x" | "translate-y" => (-100_000.0..=100_000.0).contains(&value),
        "layout-x" | "layout-y" => (-1_000_000_000_000.0..=1_000_000_000_000.0).contains(&value),
        _ => (-100_000.0..=100_000.0).contains(&value),
    }
}

fn clamp_channel(name: &str, value: f64) -> f64 {
    match name {
        "opacity" => value.clamp(0.0, 1.0),
        "scale" => value.clamp(0.25, 2.0),
        "switch-thumb"
        | "progress-fill"
        | "tab-selection"
        | "text-reveal-progress"
        | "skeleton-phase"
        | "scroll-reveal-progress"
        | "disclosure-progress"
        | "overlay-progress" => value.clamp(0.0, 1.0),
        "tab-indicator-width" => value.clamp(0.0, 100_000.0),
        "ticker-value" => value.clamp(i32::MIN as f64, i32::MAX as f64),
        "translate-x" | "translate-y" => value.clamp(-100_000.0, 100_000.0),
        "layout-x" | "layout-y" => value.clamp(-1_000_000_000_000.0, 1_000_000_000_000.0),
        _ => value.clamp(-100_000.0, 100_000.0),
    }
}

fn current_channels(instance: &Instance, now_ms: u64) -> BTreeMap<String, f64> {
    let mut channels = instance.channels.clone();
    for (name, track) in &instance.tracks {
        channels.insert(
            name.clone(),
            clamp_channel(
                name,
                sample(track.from, track.to, track.start_ms, now_ms, track.motion),
            ),
        );
    }
    channels
}

fn retarget(target: &MotionTarget, old: &Instance, now_ms: u64, reduced: bool) -> Instance {
    let current = current_channels(old, now_ms);
    let mut channels = target.channels.clone();
    let mut tracks = BTreeMap::new();
    for (name, to) in &target.channels {
        let from = current
            .get(name)
            .copied()
            .unwrap_or_else(|| default_channel(name));
        if reduced || target.motion.duration_ms == 0 || from == *to {
            continue;
        }
        if old.targets.get(name) == Some(to) {
            if let Some(track) = old.tracks.get(name) {
                tracks.insert(name.clone(), *track);
                channels.insert(name.clone(), clamp_channel(name, from));
                continue;
            }
        }
        tracks.insert(
            name.clone(),
            Track {
                from,
                to: *to,
                start_ms: now_ms,
                motion: target.motion,
            },
        );
        channels.insert(name.clone(), clamp_channel(name, from));
    }
    Instance {
        kind: target.kind,
        targets: target.channels.clone(),
        channels,
        tracks,
        motion: target.motion,
        exit: target.exit,
        snapshot: target.snapshot.as_ref().and_then(VisualSnapshot::sanitized),
    }
}

fn retarget_from_channels(
    target: &MotionTarget,
    current: &BTreeMap<String, f64>,
    now_ms: u64,
    reduced: bool,
) -> Instance {
    let mut channels = target.channels.clone();
    let mut tracks = BTreeMap::new();
    for (name, to) in &target.channels {
        let from = current
            .get(name)
            .copied()
            .unwrap_or_else(|| default_channel(name));
        if reduced || target.motion.duration_ms == 0 || from == *to {
            continue;
        }
        channels.insert(name.clone(), clamp_channel(name, from));
        tracks.insert(
            name.clone(),
            Track {
                from,
                to: *to,
                start_ms: now_ms,
                motion: target.motion,
            },
        );
    }
    Instance {
        kind: target.kind,
        targets: target.channels.clone(),
        channels,
        tracks,
        motion: target.motion,
        exit: target.exit,
        snapshot: target.snapshot.as_ref().and_then(VisualSnapshot::sanitized),
    }
}

fn retarget_new(target: &MotionTarget, now_ms: u64, reduced: bool) -> Instance {
    let mut destinations = target.channels.clone();
    let mut starts = destinations.clone();
    transition_start(target.enter, &mut starts, &mut destinations);
    let mut channels = destinations.clone();
    let mut tracks = BTreeMap::new();
    if !reduced && target.motion.duration_ms > 0 {
        for (name, to) in &destinations {
            let from = if matches!(name.as_str(), "layout-x" | "layout-y") {
                *to
            } else {
                starts
                    .get(name)
                    .copied()
                    .unwrap_or_else(|| default_channel(name))
            };
            if from != *to {
                channels.insert(name.clone(), clamp_channel(name, from));
                tracks.insert(
                    name.clone(),
                    Track {
                        from,
                        to: *to,
                        start_ms: now_ms,
                        motion: target.motion,
                    },
                );
            }
        }
    }
    Instance {
        kind: target.kind,
        targets: destinations,
        channels,
        tracks,
        motion: target.motion,
        exit: target.exit,
        snapshot: target.snapshot.as_ref().and_then(VisualSnapshot::sanitized),
    }
}

fn transition_start(
    transition: Transition,
    starts: &mut BTreeMap<String, f64>,
    destinations: &mut BTreeMap<String, f64>,
) {
    match transition {
        Transition::None => {}
        Transition::Fade => {
            destinations.entry("opacity".into()).or_insert(1.0);
            starts.insert("opacity".into(), 0.0);
        }
        Transition::Rise => {
            destinations.entry("opacity".into()).or_insert(1.0);
            let y = *destinations.entry("translate-y".into()).or_insert(0.0);
            starts.insert("opacity".into(), 0.0);
            starts.insert("translate-y".into(), y + 12.0);
            if destinations.contains_key("text-reveal-progress") {
                starts.insert("text-reveal-progress".into(), 0.0);
            }
        }
        Transition::Scale => {
            destinations.entry("opacity".into()).or_insert(1.0);
            let scale = *destinations.entry("scale".into()).or_insert(1.0);
            starts.insert("opacity".into(), 0.0);
            starts.insert("scale".into(), scale * 0.96);
        }
    }
    for (name, value) in starts.iter_mut() {
        *value = clamp_channel(name, *value);
    }
}

fn transition_end(transition: Transition, channels: &mut BTreeMap<String, f64>) {
    match transition {
        Transition::None => {}
        Transition::Fade => {
            channels.insert("opacity".into(), 0.0);
        }
        Transition::Rise => {
            channels.insert("opacity".into(), 0.0);
            let y = channels.get("translate-y").copied().unwrap_or(0.0);
            channels.insert("translate-y".into(), y + 12.0);
        }
        Transition::Scale => {
            channels.insert("opacity".into(), 0.0);
            let scale = channels.get("scale").copied().unwrap_or(1.0);
            channels.insert("scale".into(), scale * 0.96);
        }
    }
    for (name, value) in channels.iter_mut() {
        *value = clamp_channel(name, *value);
    }
}

fn make_exit(_key: &str, old: &Instance, now_ms: u64, reduced: bool) -> Option<ExitInstance> {
    make_exit_with(old, old.exit, now_ms, reduced)
}

fn make_exit_with(
    old: &Instance,
    transition: Transition,
    now_ms: u64,
    reduced: bool,
) -> Option<ExitInstance> {
    if reduced || transition == Transition::None || old.motion.duration_ms == 0 {
        return None;
    }
    let snapshot = old.snapshot.as_ref()?.sanitized()?;
    if snapshot.nodes.is_empty() {
        return None;
    }
    let channels = current_channels(old, now_ms);
    let mut targets = channels.clone();
    transition_end(transition, &mut targets);
    let mut tracks = BTreeMap::new();
    for (name, to) in targets {
        let from = channels
            .get(&name)
            .copied()
            .unwrap_or_else(|| default_channel(&name));
        if from != to {
            tracks.insert(
                name,
                Track {
                    from,
                    to,
                    start_ms: now_ms,
                    motion: old.motion,
                },
            );
        }
    }
    if tracks.is_empty() {
        return None;
    }
    Some(ExitInstance {
        snapshot,
        channels,
        tracks,
        created: 0,
    })
}

fn default_channel(name: &str) -> f64 {
    match name {
        "opacity" | "scale" => 1.0,
        _ => 0.0,
    }
}

fn sample_tracks(
    channels: &mut BTreeMap<String, f64>,
    tracks: &mut BTreeMap<String, Track>,
    now_ms: u64,
) {
    let mut finished = Vec::new();
    for (name, track) in tracks.iter() {
        let value = sample(track.from, track.to, track.start_ms, now_ms, track.motion);
        channels.insert(name.clone(), clamp_channel(name, value));
        let deadline = track
            .start_ms
            .saturating_add(u64::from(track.motion.delay_ms))
            .saturating_add(u64::from(track.motion.duration_ms));
        if now_ms >= deadline {
            channels.insert(name.clone(), track.to);
            finished.push(name.clone());
        }
    }
    for name in finished {
        tracks.remove(&name);
    }
}

fn update_wake(frame: &mut MotionFrame, tracks: &BTreeMap<String, Track>, now_ms: u64) {
    for track in tracks.values() {
        let begins = track
            .start_ms
            .saturating_add(u64::from(track.motion.delay_ms));
        let deadline = begins.saturating_add(u64::from(track.motion.duration_ms));
        if now_ms >= deadline {
            continue;
        }
        let requested = if now_ms < begins {
            begins
        } else {
            now_ms.saturating_add(FRAME_INTERVAL_MS)
        }
        .min(deadline);
        frame.next_wake_ms = Some(
            frame
                .next_wake_ms
                .map_or(requested, |old| old.min(requested)),
        );
    }
}

/// Sample one finite channel transition at a monotonic millisecond timestamp.
pub fn sample(from: f64, to: f64, start_ms: u64, now_ms: u64, motion: Motion) -> f64 {
    if motion.duration_ms == 0 {
        return to;
    }
    let elapsed = now_ms.saturating_sub(start_ms);
    if elapsed < u64::from(motion.delay_ms) {
        return from;
    }
    let t = (elapsed - u64::from(motion.delay_ms)) as f64 / f64::from(motion.duration_ms);
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 {
        return from;
    }
    if t == 1.0 {
        return to;
    }
    let eased = match motion.easing {
        Easing::Linear => t,
        Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
        Easing::EaseInOut if t < 0.5 => 4.0 * t.powi(3),
        Easing::EaseInOut => 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0,
        Easing::Spring => spring(t) / spring(1.0),
    };
    from + (to - from) * eased
}

fn spring(t: f64) -> f64 {
    1.0 - (-6.0 * t).exp() * ((10.0 * t).cos() + 0.6 * (10.0 * t).sin())
}
