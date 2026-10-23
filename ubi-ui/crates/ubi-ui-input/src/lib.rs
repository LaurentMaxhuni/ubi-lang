//! Centralized controlled input, focus and pointer capture.
use serde_json::{json, Value};
use std::collections::BTreeSet;
use ubi_ui_core::{Kind, OverlayKind, View};
use ubi_ui_layout::Scene;

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub key: String,
    pub value: Value,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    Enter,
    Space,
    Escape,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// Renderer-provided visual rows; caret offsets count Unicode scalars.
#[derive(Clone, Debug, Default)]
pub struct TextLayout {
    pub lines: Vec<VisualLine>,
}
#[derive(Clone, Debug)]
pub struct VisualLine {
    pub carets: Vec<(usize, f32)>,
    pub y: f32,
    pub height: f32,
}
impl TextLayout {
    /// A soft-wrap boundary has two positions. Retain navigation affinity when
    /// supplied; ordinary text edits use the start of the following visual row.
    pub fn caret(&self, at: usize, affinity: Option<usize>) -> Option<(usize, f32)> {
        let position = |row: usize| {
            self.lines
                .get(row)?
                .carets
                .iter()
                .find(|(i, _)| *i == at)
                .map(|(_, x)| (row, *x))
        };
        affinity
            .and_then(position)
            .or_else(|| (0..self.lines.len()).rev().find_map(position))
    }
}

/// Focus/capture is keyed and cleaned when nodes unmount, change kind or disable.
#[derive(Default)]
pub struct Input {
    pub focused: Option<String>,
    pub hovered: Option<String>,
    pub captured: Option<String>,
    pub caret: usize,
    pub caret_line: Option<usize>,
    pub select_all: bool,
    pub pointer: (f32, f32),
    pub pointer_inside: bool,
    pub active_option: Option<String>,
    keyboard_pressed: Option<String>,
    vertical_column: Option<usize>,
    vertical_x: Option<f32>,
    active_modal: Option<String>,
    focus_returns: std::collections::BTreeMap<String, FocusReturn>,
    outside_down: Option<(String, u64)>,
    overlay_generations: std::collections::BTreeMap<String, u64>,
    last_open: std::collections::BTreeMap<String, bool>,
    overlay_focus_returns: std::collections::BTreeMap<String, FocusReturn>,
    effect_focus_fallback: Vec<String>,
    typeahead_buffer: String,
    typeahead_deadline_ms: Option<u64>,
}
#[derive(Clone, Debug, Default)]
struct FocusReturn {
    preferred: Option<String>,
    fallback: Vec<String>,
}
impl Input {
    pub fn sync(&mut self, view: &View) {
        let mut next_open = std::collections::BTreeMap::new();
        for node in view
            .nodes
            .values()
            .filter(|node| node.kind == Kind::Overlay)
        {
            next_open.insert(node.key.clone(), node.open);
            if node.open && self.last_open.get(&node.key) != Some(&true) {
                let generation = self
                    .overlay_generations
                    .entry(node.key.clone())
                    .or_default();
                *generation = generation.saturating_add(1);
            }
        }
        let previous_open = std::mem::replace(&mut self.last_open, next_open.clone());
        let visible = visible_keys(view);
        for key in &view.order {
            let Some(node) = view.nodes.get(key) else {
                continue;
            };
            if node.kind == Kind::Overlay
                && matches!(
                    node.overlay_kind,
                    Some(
                        OverlayKind::Combobox
                            | OverlayKind::Select
                            | OverlayKind::DropdownMenu
                            | OverlayKind::ContextMenu
                    )
                )
                && node.open
                && previous_open.get(key) != Some(&true)
            {
                self.overlay_focus_returns
                    .entry(key.clone())
                    .or_insert_with(|| focus_return(view, key, self.focused.clone()));
                let target = match node.overlay_kind {
                    Some(OverlayKind::Select) => {
                        let options = overlay_items(view, key, Kind::OptionItem, &visible);
                        options
                            .iter()
                            .find(|item| view.nodes[*item].checked)
                            .or_else(|| options.first())
                            .cloned()
                    }
                    Some(OverlayKind::DropdownMenu | OverlayKind::ContextMenu) => {
                        overlay_items(view, key, Kind::MenuItem, &visible)
                            .into_iter()
                            .next()
                    }
                    _ => None,
                };
                if let Some(target) = target {
                    self.focus(view, Some(target));
                }
            }
        }
        let previous_modal = self.active_modal.clone();
        let next_modal = top_modal(view).map(str::to_owned);
        if previous_modal != next_modal {
            if let Some(next) = next_modal.as_deref() {
                let prior_focus = self.focused.clone();
                self.focus_returns
                    .entry(next.to_owned())
                    .or_insert_with(|| focus_return(view, next, prior_focus));
            }
            if let Some(previous) = previous_modal.as_deref() {
                let restore = self.focus_returns.remove(previous).unwrap_or_default();
                if let Some(next) = next_modal.as_deref() {
                    if let Some(key) = focus_candidates(view, &visible, Some(next), &restore) {
                        self.focus(view, Some(key));
                    }
                } else {
                    let candidate = focus_candidates(view, &visible, None, &restore)
                        .or_else(|| {
                            view.order
                                .iter()
                                .find(|key| focusable_enabled(view, &visible, key))
                                .cloned()
                        })
                        .or_else(|| Some(view.root.clone()));
                    self.focus(view, candidate);
                }
            }
        }
        self.active_modal = next_modal.clone();
        let valid = |key: &String| {
            visible.contains(key.as_str())
                && view.nodes.get(key).is_some_and(|n| {
                    (n.focusable() || next_modal.as_deref() == Some(key.as_str()))
                        && !n.disabled
                        && !n.busy
                })
        };
        let closed_overlays: Vec<_> = self
            .overlay_focus_returns
            .keys()
            .filter(|key| !next_open.get(*key).copied().unwrap_or(false))
            .cloned()
            .collect();
        for key in closed_overlays {
            let restore = self.overlay_focus_returns.remove(&key).unwrap_or_default();
            let focus_is_invalid = self.focused.as_ref().is_none_or(|focused| !valid(focused));
            let focus_was_inside = self
                .focused
                .as_deref()
                .is_some_and(|focused| within(view, &key, focused));
            if focus_is_invalid || focus_was_inside {
                if let Some(candidate) = focus_candidates(view, &visible, None, &restore) {
                    self.focus(view, Some(candidate));
                }
            }
        }
        if self.focused.as_ref().is_some_and(|key| !valid(key)) {
            let fallback = self
                .effect_focus_fallback
                .iter()
                .find(|key| focusable_enabled(view, &visible, key))
                .cloned();
            if let Some(fallback) = fallback {
                self.focus(view, Some(fallback));
            } else {
                self.focus(view, None);
            }
        }
        if self.captured.as_ref().is_some_and(|key| !valid(key)) {
            self.captured = None;
        }
        if self.hovered.as_ref().is_some_and(|key| !valid(key)) {
            self.hovered = None;
        }
        if self
            .keyboard_pressed
            .as_ref()
            .is_some_and(|key| !valid(key))
        {
            self.keyboard_pressed = None;
        }
        if let Some(node) = self.focused.as_ref().and_then(|k| view.nodes.get(k)) {
            self.caret = self.caret.min(node.value.chars().count());
        }
        if let Some(modal) = next_modal.as_deref() {
            if self
                .focused
                .as_deref()
                .is_none_or(|key| !within(view, modal, key))
            {
                let first = view
                    .order
                    .iter()
                    .find(|key| within(view, modal, key) && focusable_enabled(view, &visible, key))
                    .cloned()
                    .or_else(|| Some(modal.to_owned()));
                self.focus(view, first);
            }
        }
        if self.active_option.as_ref().is_some_and(|active| {
            let Some(focused) = self.focused.as_deref() else {
                return true;
            };
            let Some(options) = open_combobox_options(view, focused, &visible) else {
                return true;
            };
            !options.contains(active)
        }) {
            self.active_option = None;
        }
    }
    pub fn cancel(&mut self) {
        self.captured = None;
        self.keyboard_pressed = None;
        self.hovered = None;
        self.pointer_inside = false;
        self.outside_down = None;
        self.typeahead_buffer.clear();
        self.typeahead_deadline_ms = None;
    }
    pub fn pressed(&self, key: &str) -> bool {
        self.captured.as_deref() == Some(key) || self.keyboard_pressed.as_deref() == Some(key)
    }
    /// Active descendant for an open Combobox while focus remains in its input.
    pub fn active_option(&self) -> Option<&str> {
        self.active_option.as_deref()
    }
    pub fn hit(view: &View, scene: &Scene, x: f32, y: f32) -> Option<String> {
        let visible = visible_keys(view);
        let modal = top_modal(view);
        scene
            .order
            .iter()
            .rev()
            .find(|k| {
                visible.contains(k.as_str())
                    && modal.is_none_or(|modal| within(view, modal, k))
                    && view.nodes.get(*k).is_some_and(|n| n.focusable())
                    && scene
                        .boxes
                        .get(*k)
                        .is_some_and(|b| b.bounds.intersect(b.clip).contains(x, y))
            })
            .cloned()
    }
    pub fn pointer_move(&mut self, view: &View, scene: &Scene, x: f32, y: f32) -> Option<Event> {
        self.pointer = (x, y);
        self.pointer_inside = true;
        self.hovered = Self::hit(view, scene, x, y);
        self.captured.as_ref().and_then(|key| {
            if view.nodes.get(key).is_some_and(|n| n.kind == Kind::Slider) {
                slider(view, scene, key, x)
            } else {
                None
            }
        })
    }
    pub fn pointer_leave(&mut self) {
        self.pointer_inside = false;
        self.hovered = None;
    }

    /// Return the source-controlled ContextMenu request under a pointer point.
    pub fn context_request_at(
        &self,
        view: &View,
        scene: &Scene,
        x: f32,
        y: f32,
    ) -> Option<(String, Event)> {
        let target = scene.order.iter().rev().find(|key| {
            scene
                .boxes
                .get(*key)
                .is_some_and(|layout| layout.bounds.intersect(layout.clip).contains(x, y))
        })?;
        context_request_from(view, target)
    }

    /// Return the ContextMenu request associated with the focused trigger.
    pub fn context_request_focused(&self, view: &View) -> Option<(String, Event)> {
        context_request_from(view, self.focused.as_deref()?)
    }

    /// Focus the next enabled menu/option with a case-insensitive prefix.
    /// The search buffer expires after 500ms and never handles editable queries.
    pub fn typeahead(&mut self, view: &View, text: &str, now_ms: u64) -> bool {
        let Some(target) = self.focused.as_deref() else {
            return false;
        };
        let Some(node) = view.nodes.get(target) else {
            return false;
        };
        if !matches!(node.kind, Kind::MenuItem | Kind::OptionItem) {
            return false;
        }
        let incoming: String = text
            .chars()
            .filter(|character| !character.is_control())
            .take(64)
            .flat_map(char::to_lowercase)
            .collect();
        if incoming.is_empty() {
            return false;
        }
        let expired = self
            .typeahead_deadline_ms
            .is_none_or(|deadline| now_ms >= deadline);
        if expired {
            self.typeahead_buffer.clear();
        }
        let incoming_char = incoming.chars().next();
        let repeated = incoming.chars().count() == 1
            && incoming_char.is_some_and(|character| {
                !self.typeahead_buffer.is_empty()
                    && self.typeahead_buffer.chars().all(|buffered| {
                        buffered.to_lowercase().to_string() == character.to_string()
                    })
            });
        let mut query = if repeated {
            incoming.clone()
        } else {
            format!("{}{}", self.typeahead_buffer, incoming)
        };
        query = query.chars().take(64).collect();
        let visible = visible_keys(view);
        let items = typeahead_items(view, target, &visible);
        let start = items
            .iter()
            .position(|item| item.key == target)
            .map_or(0, |index| index + 1);
        let matches = |query: &str| {
            (0..items.len()).find_map(|offset| {
                let item = items[(start + offset) % items.len()];
                let label = if item.label.trim().is_empty() {
                    &item.content
                } else {
                    &item.label
                };
                label
                    .to_lowercase()
                    .starts_with(query)
                    .then(|| item.key.clone())
            })
        };
        let selected = matches(&query).or_else(|| {
            if expired || repeated {
                None
            } else {
                query = incoming.clone();
                matches(&query)
            }
        });
        let Some(selected) = selected else {
            self.typeahead_buffer.clear();
            self.typeahead_deadline_ms = None;
            return false;
        };
        self.typeahead_buffer = query;
        self.typeahead_deadline_ms = Some(now_ms.saturating_add(500));
        if self.focused.as_deref() == Some(selected.as_str()) {
            false
        } else {
            self.focus(view, Some(selected));
            true
        }
    }
    pub fn pointer_down(&mut self, view: &View, scene: &Scene) -> Option<Event> {
        self.outside_down = outside_dismiss_target(view, scene, self.pointer.0, self.pointer.1)
            .map(|key| {
                let generation = self
                    .overlay_generations
                    .get(&key)
                    .copied()
                    .unwrap_or_default();
                (key, generation)
            });
        let hit = Self::hit(view, scene, self.pointer.0, self.pointer.1)
            .filter(|k| !view.nodes[k].disabled && !view.nodes[k].busy);
        let combobox_anchor = hit
            .as_deref()
            .and_then(|key| combobox_anchor_for_option(view, key));
        if let Some(anchor) = combobox_anchor {
            self.focus(view, Some(anchor));
            self.active_option.clone_from(&hit);
        } else if self.active_modal.is_none() || hit.is_some() {
            self.focus(view, hit.clone());
        }
        self.captured = hit.clone();
        hit.and_then(|key| {
            if view.nodes[&key].kind == Kind::Slider {
                slider(view, scene, &key, self.pointer.0)
            } else {
                None
            }
        })
    }
    pub fn pointer_up(&mut self, view: &View, scene: &Scene) -> Option<Event> {
        if let Some((key, generation)) = self.outside_down.take() {
            if self.overlay_generations.get(&key).copied() == Some(generation)
                && outside_dismiss_still_valid(view, scene, &key, self.pointer.0, self.pointer.1)
            {
                self.captured = None;
                return dismiss_event(view, &key);
            }
        }
        let key = self.captured.take()?;
        if Self::hit(view, scene, self.pointer.0, self.pointer.1).as_deref() != Some(&key) {
            return None;
        }
        activate(view, &key)
    }
    fn focus(&mut self, view: &View, key: Option<String>) {
        if self.focused != key {
            self.focused = key;
            self.active_option = None;
            self.effect_focus_fallback = self
                .focused
                .as_deref()
                .map(|key| effect_navigation_candidates(view, key))
                .unwrap_or_default();
            self.select_all = false;
            self.keyboard_pressed = None;
            self.vertical_column = None;
            self.caret_line = None;
            self.vertical_x = None;
            self.caret = self
                .focused
                .as_ref()
                .and_then(|k| view.nodes.get(k))
                .map_or(0, |n| n.value.chars().count());
        }
    }
    pub fn key(&mut self, view: &View, key: Key, down: bool, shift: bool) -> Option<Event> {
        self.key_with_text_layout(view, key, down, shift, None)
    }
    /// Native hosts supply current renderer geometry for soft-wrap navigation.
    /// Hosts without visual geometry retain hard-line navigation through `key`.
    pub fn key_with_text_layout(
        &mut self,
        view: &View,
        key: Key,
        down: bool,
        shift: bool,
        text_layout: Option<&TextLayout>,
    ) -> Option<Event> {
        if key == Key::Tab && down {
            let visible = visible_keys(view);
            let modal = top_modal(view);
            let keys: Vec<_> = view
                .order
                .iter()
                .filter(|k| {
                    visible.contains(k.as_str())
                        && modal.is_none_or(|modal| within(view, modal, k))
                        && view.nodes.get(*k).is_some_and(|n| {
                            enabled(n) && n.focusable() && tab_stop(view, n) == n.key
                        })
                })
                .cloned()
                .collect();
            if keys.is_empty() {
                self.focus(view, modal.map(str::to_owned));
                return None;
            }
            let index = self
                .focused
                .as_ref()
                .and_then(|k| view.nodes.get(k))
                .and_then(|n| keys.iter().position(|v| v == tab_stop(view, n)));
            let next = match index {
                Some(i) if shift => (i + keys.len() - 1) % keys.len(),
                Some(i) => (i + 1) % keys.len(),
                None if shift => keys.len() - 1,
                None => 0,
            };
            self.focus(view, Some(keys[next].clone()));
            return None;
        }
        if key == Key::Escape {
            self.cancel();
            return if down {
                dismiss_top_overlay(view)
            } else {
                None
            };
        }
        let target = self.focused.clone()?;
        let n = view.nodes.get(&target)?;
        if !enabled(n) {
            return None;
        }
        if n.kind == Kind::TextInput {
            let visible = visible_keys(view);
            if let Some(items) = open_combobox_options(view, &target, &visible) {
                if down && matches!(key, Key::Up | Key::Down | Key::Home | Key::End) {
                    if items.is_empty() {
                        self.active_option = None;
                    } else {
                        let current = self
                            .active_option
                            .as_deref()
                            .and_then(|active| items.iter().position(|item| item == active));
                        let index = match key {
                            Key::Up => current.map_or(items.len() - 1, |index| {
                                (index + items.len() - 1) % items.len()
                            }),
                            Key::Down => current.map_or(0, |index| (index + 1) % items.len()),
                            Key::Home => 0,
                            Key::End => items.len() - 1,
                            _ => unreachable!(),
                        };
                        self.active_option = Some(items[index].clone());
                    }
                    return None;
                }
                if key == Key::Enter {
                    if down {
                        return None;
                    }
                    if let Some(active) = self.active_option.as_deref() {
                        if items.iter().any(|item| item == active) {
                            return activate(view, active);
                        }
                    }
                }
            }
        }
        if down && grouped(n) {
            let items: Vec<_> = view
                .order
                .iter()
                .filter_map(|k| view.nodes.get(k))
                .filter(|item| item.kind == n.kind && item.group == n.group && enabled(item))
                .collect();
            let index = items.iter().position(|item| item.key == target)?;
            let next = match key {
                Key::Left | Key::Up => (index + items.len() - 1) % items.len(),
                Key::Right | Key::Down => (index + 1) % items.len(),
                Key::Home => 0,
                Key::End => items.len() - 1,
                _ => return self.activation_key(view, key, down, target),
            };
            let key = items[next].key.clone();
            self.focus(view, Some(key.clone()));
            return if n.kind == Kind::ToggleItem {
                None
            } else {
                activate(view, &key)
            };
        }
        if down && matches!(n.kind, Kind::MenuItem | Kind::OptionItem) {
            if let Some(next) = sibling_item(view, &target, key) {
                self.focus(view, Some(next));
                return None;
            }
        }
        if n.kind.editable() && down {
            let mut chars: Vec<char> = n.value.chars().collect();
            self.caret = self.caret.min(chars.len());
            if n.kind == Kind::TextArea && matches!(key, Key::Up | Key::Down | Key::Home | Key::End)
            {
                if let Some(layout) = text_layout {
                    if let Some((row, x)) = layout.caret(self.caret, self.caret_line) {
                        let next = match key {
                            Key::Up => row.saturating_sub(1),
                            Key::Down => (row + 1).min(layout.lines.len() - 1),
                            _ => row,
                        };
                        let stops = &layout.lines[next].carets;
                        let stop = match key {
                            Key::Home => stops.first(),
                            Key::End => stops.last(),
                            _ => {
                                let goal = *self.vertical_x.get_or_insert(x);
                                stops.iter().min_by(|a, b| {
                                    (a.1 - goal).abs().total_cmp(&(b.1 - goal).abs())
                                })
                            }
                        };
                        if let Some((at, _)) = stop {
                            self.caret = *at;
                            self.caret_line = Some(next);
                            self.select_all = false;
                            self.vertical_column = None;
                            if matches!(key, Key::Home | Key::End) {
                                self.vertical_x = None;
                            }
                            return None;
                        }
                    }
                }
            }
            self.caret_line = None;
            self.vertical_x = None;
            if !matches!(key, Key::Up | Key::Down) {
                self.vertical_column = None;
            }
            match key {
                Key::Up | Key::Down if n.kind == Kind::NumberInput => {
                    if n.read_only || n.step <= 0 || n.min > n.max {
                        return None;
                    }
                    let current = i64::from(n.value.parse::<i32>().unwrap_or(0));
                    let delta = i64::from(n.step) * if key == Key::Up { 1 } else { -1 };
                    let value = (current + delta).clamp(i64::from(n.min), i64::from(n.max));
                    let value = value.to_string();
                    self.caret = value.chars().count();
                    self.select_all = false;
                    return input_event(n, value);
                }
                Key::Up | Key::Down if n.kind == Kind::TextArea => {
                    let column = *self
                        .vertical_column
                        .get_or_insert_with(|| self.caret - line_start(&chars, self.caret));
                    self.caret = vertical_caret(&chars, self.caret, column, key == Key::Down);
                    self.select_all = false;
                }
                Key::Left => {
                    self.caret = self.caret.saturating_sub(1);
                    self.select_all = false;
                }
                Key::Right => {
                    self.caret = (self.caret + 1).min(chars.len());
                    self.select_all = false;
                }
                Key::Home => {
                    self.caret = if n.kind == Kind::TextArea {
                        line_start(&chars, self.caret)
                    } else {
                        0
                    };
                    self.select_all = false;
                }
                Key::End => {
                    self.caret = if n.kind == Kind::TextArea {
                        line_end(&chars, self.caret)
                    } else {
                        chars.len()
                    };
                    self.select_all = false;
                }
                Key::Backspace | Key::Delete => {
                    if n.read_only {
                        return None;
                    }
                    let previous = (self.caret, self.select_all);
                    if self.select_all {
                        chars.clear();
                        self.caret = 0;
                        self.select_all = false;
                    } else if key == Key::Backspace && self.caret > 0 {
                        self.caret -= 1;
                        chars.remove(self.caret);
                    } else if key == Key::Delete && self.caret < chars.len() {
                        chars.remove(self.caret);
                    } else {
                        return None;
                    }
                    let value = chars.into_iter().collect::<String>();
                    if n.kind == Kind::NumberInput && !integer_prefix(&value) {
                        (self.caret, self.select_all) = previous;
                        return None;
                    }
                    return input_event(n, value);
                }
                Key::Enter if n.kind == Kind::TextArea => return self.text(view, "\n"),
                _ => {}
            }
            return None;
        }
        if n.kind == Kind::Slider && down {
            let current = n.progress;
            let next = match key {
                Key::Left | Key::Down => (current - 1).max(0),
                Key::Right | Key::Up => (current + 1).min(100),
                Key::Home => 0,
                Key::End => 100,
                _ => return None,
            };
            return n
                .change
                .as_ref()
                .map(|m| event(&n.key, "Select", vec![m.clone(), json!(next.to_string())]));
        }
        self.activation_key(view, key, down, target)
    }
    fn activation_key(
        &mut self,
        view: &View,
        key: Key,
        down: bool,
        target: String,
    ) -> Option<Event> {
        if matches!(key, Key::Enter | Key::Space) {
            if down {
                if self.keyboard_pressed.is_none() {
                    self.keyboard_pressed = Some(target);
                }
            } else if self.keyboard_pressed.take().as_deref() == Some(&target) {
                return activate(view, &target);
            }
        }
        None
    }
    /// Replace selected content or insert at a Unicode scalar boundary.
    /// Platform IME commit passes through here; only TextArea accepts newlines.
    pub fn text(&mut self, view: &View, text: &str) -> Option<Event> {
        let n = self.focused.as_ref().and_then(|key| view.nodes.get(key))?;
        if !n.kind.editable()
            || !enabled(n)
            || n.read_only
            || text
                .chars()
                .any(|c| c.is_control() && !(n.kind == Kind::TextArea && c == '\n'))
        {
            return None;
        }
        let mut chars: Vec<char> = if self.select_all {
            Vec::new()
        } else {
            n.value.chars().collect()
        };
        let at = if self.select_all {
            0
        } else {
            self.caret.min(chars.len())
        };
        let inserted: Vec<_> = text.chars().collect();
        chars.splice(at..at, inserted.iter().copied());
        let value: String = chars.into_iter().collect();
        if value.len() > 16384 || (n.kind == Kind::NumberInput && !integer_prefix(&value)) {
            return None;
        }
        self.select_all = false;
        self.vertical_column = None;
        self.caret_line = None;
        self.vertical_x = None;
        self.caret = at + inserted.len();
        self.active_option = None;
        input_event(n, value)
    }
}
fn enabled(n: &ubi_ui_core::Node) -> bool {
    !n.disabled && !n.busy
}
fn grouped(n: &ubi_ui_core::Node) -> bool {
    matches!(n.kind, Kind::Radio | Kind::Tab | Kind::ToggleItem) && !n.group.is_empty()
}
fn tab_stop<'a>(view: &'a View, n: &'a ubi_ui_core::Node) -> &'a str {
    if !grouped(n) {
        return &n.key;
    }
    let items = || {
        view.order
            .iter()
            .filter_map(|k| view.nodes.get(k))
            .filter(|item| item.kind == n.kind && item.group == n.group && enabled(item))
    };
    items()
        .find(|item| item.checked)
        .or_else(|| items().next())
        .map_or(&n.key, |item| &item.key)
}
fn integer_prefix(value: &str) -> bool {
    value.is_empty()
        || value == "-"
        || (value
            .strip_prefix('-')
            .unwrap_or(value)
            .bytes()
            .all(|c| c.is_ascii_digit())
            && value.parse::<i32>().is_ok())
}
fn line_start(chars: &[char], at: usize) -> usize {
    chars[..at]
        .iter()
        .rposition(|c| *c == '\n')
        .map_or(0, |i| i + 1)
}
fn line_end(chars: &[char], at: usize) -> usize {
    chars[at..]
        .iter()
        .position(|c| *c == '\n')
        .map_or(chars.len(), |i| at + i)
}
fn vertical_caret(chars: &[char], at: usize, column: usize, down: bool) -> usize {
    let start = line_start(chars, at);
    if down {
        let end = line_end(chars, at);
        if end == chars.len() {
            return at;
        }
        let next = end + 1;
        (next + column).min(line_end(chars, next))
    } else if start == 0 {
        at
    } else {
        let previous_end = start - 1;
        (line_start(chars, previous_end) + column).min(previous_end)
    }
}
fn event(key: &str, tag: &str, values: Vec<Value>) -> Event {
    Event {
        key: key.into(),
        value: json!({"tag":tag,"values":values}),
    }
}
fn input_event(n: &ubi_ui_core::Node, value: String) -> Option<Event> {
    n.input
        .as_ref()
        .map(|m| event(&n.key, "Input", vec![m.clone(), json!(value)]))
}
fn activate(view: &View, key: &str) -> Option<Event> {
    let n = view.nodes.get(key)?;
    if n.disabled || n.busy {
        return None;
    }
    match n.kind {
        Kind::Checkbox | Kind::Switch | Kind::Toggle => n
            .toggle
            .as_ref()
            .map(|m| event(key, "Toggle", vec![m.clone(), json!(!n.checked)])),
        Kind::Radio | Kind::Tab | Kind::ToggleItem | Kind::OptionItem | Kind::TableHeader => n
            .change
            .as_ref()
            .map(|m| event(key, "Select", vec![m.clone(), json!(n.option)])),
        Kind::Button | Kind::IconButton | Kind::Link | Kind::MenuItem => n
            .activate
            .as_ref()
            .map(|m| event(key, "Activate", vec![m.clone()])),
        _ => None,
    }
}

fn visible_keys(view: &View) -> BTreeSet<String> {
    fn visit(view: &View, key: &str, visible: &mut BTreeSet<String>) {
        let Some(node) = view.nodes.get(key) else {
            return;
        };
        if (node.kind == Kind::Overlay && !node.open) || node.disclosure == Some(false) {
            return;
        }
        visible.insert(key.to_owned());
        for (index, child) in node.children.iter().enumerate() {
            if node
                .selected_child_index()
                .is_some_and(|selected| selected != index)
            {
                continue;
            }
            visit(view, child, visible);
        }
    }
    let mut visible = BTreeSet::new();
    visit(view, &view.root, &mut visible);
    visible
}

fn context_request_from(view: &View, target: &str) -> Option<(String, Event)> {
    let mut anchor = target;
    for _ in 0..64 {
        if let Some(menu) = view.order.iter().rev().find_map(|key| {
            let node = view.nodes.get(key)?;
            (node.kind == Kind::Overlay
                && node.overlay_kind == Some(OverlayKind::ContextMenu)
                && node.anchor.as_deref() == Some(anchor))
            .then_some(node)
        }) {
            let request = menu.open_request.as_ref()?;
            let event = Event {
                key: menu.key.clone(),
                value: json!({"tag":"Activate","values":[request]}),
            };
            return Some((menu.key.clone(), event));
        }
        let (parent, _) = view
            .nodes
            .iter()
            .find(|(_, node)| node.children.iter().any(|child| child == anchor))?;
        anchor = parent;
    }
    None
}

fn effect_navigation_candidates(view: &View, key: &str) -> Vec<String> {
    let mut current = key;
    for _ in 0..64 {
        let Some((parent_key, parent)) = view
            .nodes
            .iter()
            .find(|(_, node)| node.children.iter().any(|child| child == current))
        else {
            break;
        };
        let selected = parent.selected_child_index();
        if parent.kind == Kind::EffectSurface
            && selected.is_some_and(|index| {
                parent
                    .children
                    .get(index)
                    .is_some_and(|child| child == current)
            })
        {
            let Some((_, container)) = view
                .nodes
                .iter()
                .find(|(_, node)| node.children.iter().any(|child| child == parent_key))
            else {
                return Vec::new();
            };
            return container
                .children
                .iter()
                .filter(|sibling| sibling.as_str() != parent_key)
                .filter(|sibling| {
                    view.nodes
                        .get(*sibling)
                        .is_some_and(|node| node.focusable())
                })
                .cloned()
                .collect();
        }
        current = parent_key;
    }
    Vec::new()
}

fn modal_kind(kind: Option<OverlayKind>) -> bool {
    matches!(
        kind,
        Some(
            OverlayKind::Dialog
                | OverlayKind::AlertDialog
                | OverlayKind::Drawer
                | OverlayKind::CommandPalette
        )
    )
}

fn top_modal(view: &View) -> Option<&str> {
    let visible = visible_keys(view);
    view.order.iter().rev().find_map(|key| {
        let node = view.nodes.get(key)?;
        (visible.contains(key.as_str())
            && node.kind == Kind::Overlay
            && node.open
            && modal_kind(node.overlay_kind))
        .then_some(key.as_str())
    })
}

fn within(view: &View, root: &str, key: &str) -> bool {
    if root == key {
        return true;
    }
    let Some(node) = view.nodes.get(root) else {
        return false;
    };
    node.children.iter().any(|child| within(view, child, key))
}

fn focusable_enabled(view: &View, visible: &BTreeSet<String>, key: &str) -> bool {
    visible.contains(key)
        && view
            .nodes
            .get(key)
            .is_some_and(|node| node.focusable() && enabled(node))
}

fn focus_return(view: &View, overlay: &str, preferred: Option<String>) -> FocusReturn {
    let mut fallback = Vec::new();
    if let Some(node) = view.nodes.get(overlay) {
        if let Some(anchor) = node.anchor.as_ref() {
            fallback.push(anchor.clone());
        }
        let mut current = overlay;
        for _ in 0..32 {
            let Some((parent, _)) = view
                .nodes
                .iter()
                .find(|(_, node)| node.children.iter().any(|child| child == current))
            else {
                break;
            };
            fallback.push(parent.clone());
            current = parent;
        }
    }
    FocusReturn {
        preferred,
        fallback,
    }
}

fn focus_candidates(
    view: &View,
    visible: &BTreeSet<String>,
    modal: Option<&str>,
    restore: &FocusReturn,
) -> Option<String> {
    restore
        .preferred
        .iter()
        .chain(restore.fallback.iter())
        .find(|key| {
            focusable_enabled(view, visible, key)
                && modal.is_none_or(|modal| within(view, modal, key))
        })
        .cloned()
}

fn dismiss_event(view: &View, key: &str) -> Option<Event> {
    let node = view.nodes.get(key)?;
    let message = node.dismiss.as_ref()?;
    Some(event(key, "Activate", vec![message.clone()]))
}

fn dismiss_top_overlay(view: &View) -> Option<Event> {
    let visible = visible_keys(view);
    let key = view.order.iter().rev().find(|key| {
        visible.contains(key.as_str())
            && view
                .nodes
                .get(*key)
                .is_some_and(|node| node.kind == Kind::Overlay && node.open)
    })?;
    if view.nodes[key].overlay_kind == Some(OverlayKind::AlertDialog) {
        return None;
    }
    dismiss_event(view, key)
}

fn outside_dismiss_target(view: &View, scene: &Scene, x: f32, y: f32) -> Option<String> {
    let visible = visible_keys(view);
    let key = view.order.iter().rev().find(|key| {
        visible.contains(key.as_str())
            && view
                .nodes
                .get(*key)
                .is_some_and(|node| node.kind == Kind::Overlay && node.open)
    })?;
    let node = &view.nodes[key];
    if node.dismiss.is_none()
        || node.overlay_kind == Some(OverlayKind::AlertDialog)
        || overlay_contains(view, scene, key, x, y)
    {
        return None;
    }
    Some(key.clone())
}

fn outside_dismiss_still_valid(view: &View, scene: &Scene, key: &str, x: f32, y: f32) -> bool {
    view.nodes
        .get(key)
        .is_some_and(|node| node.kind == Kind::Overlay && node.open && node.dismiss.is_some())
        && !overlay_contains(view, scene, key, x, y)
}

fn overlay_contains(view: &View, scene: &Scene, root: &str, x: f32, y: f32) -> bool {
    let mut stack = vec![root];
    while let Some(key) = stack.pop() {
        if scene
            .boxes
            .get(key)
            .is_some_and(|b| b.bounds.intersect(b.clip).contains(x, y))
        {
            return true;
        }
        if let Some(node) = view.nodes.get(key) {
            stack.extend(node.children.iter().map(String::as_str));
        }
    }
    false
}

fn sibling_item(view: &View, key: &str, direction: Key) -> Option<String> {
    fn collect<'a>(view: &'a View, key: &str, out: &mut Vec<&'a ubi_ui_core::Node>) {
        let Some(parent) = view.nodes.get(key) else {
            return;
        };
        for child in &parent.children {
            let Some(node) = view.nodes.get(child) else {
                continue;
            };
            if matches!(node.kind, Kind::MenuItem | Kind::OptionItem) && enabled(node) {
                out.push(node);
            } else {
                collect(view, child, out);
            }
        }
    }
    let mut current = key;
    let mut fallback = None;
    let mut owner = None;
    for _ in 0..32 {
        let Some((parent_key, parent)) = view
            .nodes
            .iter()
            .find(|(_, node)| node.children.iter().any(|child| child == current))
        else {
            break;
        };
        fallback.get_or_insert(parent_key.as_str());
        let matching_role = match view.nodes[key].kind {
            Kind::MenuItem => parent.role == "menu",
            Kind::OptionItem => parent.role == "listbox",
            _ => false,
        };
        if matching_role {
            owner = Some(parent_key.as_str());
            break;
        }
        current = parent_key;
    }
    let owner = owner.or(fallback)?;
    let mut items = Vec::new();
    collect(view, owner, &mut items);
    if items.is_empty() {
        return None;
    }
    let index = items.iter().position(|item| item.key == key)?;
    let next = match direction {
        Key::Up | Key::Left => (index + items.len() - 1) % items.len(),
        Key::Down | Key::Right => (index + 1) % items.len(),
        Key::Home => 0,
        Key::End => items.len() - 1,
        _ => return None,
    };
    Some(items[next].key.clone())
}

fn typeahead_items<'a>(
    view: &'a View,
    key: &str,
    visible: &BTreeSet<String>,
) -> Vec<&'a ubi_ui_core::Node> {
    fn collect<'a>(
        view: &'a View,
        key: &str,
        kind: Kind,
        visible: &BTreeSet<String>,
        depth: usize,
        items: &mut Vec<&'a ubi_ui_core::Node>,
    ) {
        if depth >= 64 {
            return;
        }
        let Some(parent) = view.nodes.get(key) else {
            return;
        };
        for child in &parent.children {
            let Some(node) = view.nodes.get(child) else {
                continue;
            };
            if node.kind == kind {
                if visible.contains(&node.key) && enabled(node) {
                    items.push(node);
                }
            } else {
                collect(view, child, kind, visible, depth + 1, items);
            }
        }
    }

    let kind = view.nodes.get(key).map(|node| node.kind);
    let Some(kind @ (Kind::MenuItem | Kind::OptionItem)) = kind else {
        return Vec::new();
    };
    let mut current = key;
    let mut fallback = None;
    let mut owner = None;
    for _ in 0..64 {
        let Some((parent_key, parent)) = view
            .nodes
            .iter()
            .find(|(_, node)| node.children.iter().any(|child| child == current))
        else {
            break;
        };
        fallback.get_or_insert(parent_key.as_str());
        let matching_role = match kind {
            Kind::MenuItem => parent.role == "menu",
            Kind::OptionItem => parent.role == "listbox",
            _ => false,
        };
        if matching_role {
            owner = Some(parent_key.as_str());
            break;
        }
        current = parent_key;
    }
    let Some(owner) = owner.or(fallback) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    collect(view, owner, kind, visible, 0, &mut items);
    items
}

fn open_combobox_options(
    view: &View,
    anchor: &str,
    visible: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let overlay = view.order.iter().rev().find(|key| {
        view.nodes.get(*key).is_some_and(|node| {
            node.kind == Kind::Overlay
                && node.overlay_kind == Some(OverlayKind::Combobox)
                && node.open
                && node.anchor.as_deref() == Some(anchor)
        })
    })?;
    Some(overlay_items(view, overlay, Kind::OptionItem, visible))
}

fn combobox_anchor_for_option(view: &View, option: &str) -> Option<String> {
    view.order.iter().rev().find_map(|key| {
        let node = view.nodes.get(key)?;
        if node.kind != Kind::Overlay
            || node.overlay_kind != Some(OverlayKind::Combobox)
            || !node.open
            || !within(view, key, option)
        {
            return None;
        }
        let anchor = node.anchor.as_deref()?;
        view.nodes
            .get(anchor)
            .is_some_and(|node| node.kind == Kind::TextInput)
            .then(|| anchor.to_owned())
    })
}

fn overlay_items(view: &View, root: &str, kind: Kind, visible: &BTreeSet<String>) -> Vec<String> {
    fn collect(
        view: &View,
        key: &str,
        kind: Kind,
        visible: &BTreeSet<String>,
        depth: usize,
        items: &mut Vec<String>,
    ) {
        if depth >= 32 {
            return;
        }
        let Some(parent) = view.nodes.get(key) else {
            return;
        };
        for child in &parent.children {
            let Some(node) = view.nodes.get(child) else {
                continue;
            };
            if node.kind == kind {
                if visible.contains(child) && enabled(node) {
                    items.push(child.clone());
                }
            } else {
                collect(view, child, kind, visible, depth + 1, items);
            }
        }
    }

    let mut items = Vec::new();
    collect(view, root, kind, visible, 0, &mut items);
    items
}

fn slider(view: &View, scene: &Scene, key: &str, x: f32) -> Option<Event> {
    let n = view.nodes.get(key)?;
    if n.kind != Kind::Slider || !enabled(n) {
        return None;
    }
    let b = scene.boxes.get(key)?.bounds;
    let value =
        (((x - b.x - 8.0) / (b.width - 16.0).max(1.0)).clamp(0.0, 1.0) * 100.0).round() as i32;
    n.change
        .as_ref()
        .map(|m| event(key, "Select", vec![m.clone(), json!(value.to_string())]))
}
