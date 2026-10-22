//! Centralized controlled input, focus and pointer capture.
use serde_json::{json, Value};
use ubi_ui_core::{Kind, View};
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
    keyboard_pressed: Option<String>,
    vertical_column: Option<usize>,
    vertical_x: Option<f32>,
}
impl Input {
    pub fn sync(&mut self, view: &View) {
        let valid = |key: &String| {
            view.nodes
                .get(key)
                .is_some_and(|n| n.kind.focusable() && !n.disabled && !n.busy)
        };
        if self.focused.as_ref().is_some_and(|key| !valid(key)) {
            self.focused = None;
            self.select_all = false;
            self.vertical_column = None;
            self.caret_line = None;
            self.vertical_x = None;
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
    }
    pub fn cancel(&mut self) {
        self.captured = None;
        self.keyboard_pressed = None;
        self.hovered = None;
    }
    pub fn pressed(&self, key: &str) -> bool {
        self.captured.as_deref() == Some(key) || self.keyboard_pressed.as_deref() == Some(key)
    }
    pub fn hit(view: &View, scene: &Scene, x: f32, y: f32) -> Option<String> {
        scene
            .order
            .iter()
            .rev()
            .find(|k| {
                view.nodes.get(*k).is_some_and(|n| n.kind.focusable())
                    && scene
                        .boxes
                        .get(*k)
                        .is_some_and(|b| b.bounds.intersect(b.clip).contains(x, y))
            })
            .cloned()
    }
    pub fn pointer_move(&mut self, view: &View, scene: &Scene, x: f32, y: f32) -> Option<Event> {
        self.pointer = (x, y);
        self.hovered = Self::hit(view, scene, x, y);
        self.captured.as_ref().and_then(|key| {
            if view.nodes.get(key).is_some_and(|n| n.kind == Kind::Slider) {
                slider(view, scene, key, x)
            } else {
                None
            }
        })
    }
    pub fn pointer_down(&mut self, view: &View, scene: &Scene) -> Option<Event> {
        let hit = Self::hit(view, scene, self.pointer.0, self.pointer.1)
            .filter(|k| !view.nodes[k].disabled && !view.nodes[k].busy);
        self.focus(view, hit.clone());
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
        let key = self.captured.take()?;
        if Self::hit(view, scene, self.pointer.0, self.pointer.1).as_deref() != Some(&key) {
            return None;
        }
        activate(view, &key)
    }
    fn focus(&mut self, view: &View, key: Option<String>) {
        if self.focused != key {
            self.focused = key;
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
            let keys: Vec<_> = view
                .order
                .iter()
                .filter(|k| {
                    view.nodes.get(*k).is_some_and(|n| {
                        enabled(n) && n.kind.focusable() && tab_stop(view, n) == n.key
                    })
                })
                .cloned()
                .collect();
            if keys.is_empty() {
                self.focus(view, None);
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
            return None;
        }
        let target = self.focused.clone()?;
        let n = view.nodes.get(&target)?;
        if !enabled(n) {
            return None;
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
        Kind::Radio | Kind::Tab | Kind::ToggleItem => n
            .change
            .as_ref()
            .map(|m| event(key, "Select", vec![m.clone(), json!(n.option)])),
        Kind::Button | Kind::IconButton | Kind::Link => n
            .activate
            .as_ref()
            .map(|m| event(key, "Activate", vec![m.clone()])),
        _ => None,
    }
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
