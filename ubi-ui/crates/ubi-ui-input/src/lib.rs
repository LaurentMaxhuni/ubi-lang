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
    Home,
    End,
}

/// Focus/capture is keyed and cleaned when nodes unmount, change kind or disable.
#[derive(Default)]
pub struct Input {
    pub focused: Option<String>,
    pub hovered: Option<String>,
    pub captured: Option<String>,
    pub caret: usize,
    pub select_all: bool,
    pub pointer: (f32, f32),
    keyboard_pressed: Option<String>,
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
            if view.nodes[key].kind == Kind::Slider {
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
            self.caret = self
                .focused
                .as_ref()
                .and_then(|k| view.nodes.get(k))
                .map_or(0, |n| n.value.chars().count());
        }
    }
    pub fn key(&mut self, view: &View, key: Key, down: bool, shift: bool) -> Option<Event> {
        if key == Key::Tab && down {
            let keys: Vec<_> = view
                .order
                .iter()
                .filter(|k| {
                    let n = &view.nodes[*k];
                    n.kind.focusable() && !n.disabled && !n.busy
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
                .and_then(|k| keys.iter().position(|v| v == k));
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
        let n = &view.nodes[&target];
        if n.disabled || n.busy {
            return None;
        }
        if n.kind.editable() && down {
            let mut chars: Vec<char> = n.value.chars().collect();
            self.caret = self.caret.min(chars.len());
            match key {
                Key::Left => {
                    self.caret = self.caret.saturating_sub(1);
                    self.select_all = false;
                }
                Key::Right => {
                    self.caret = (self.caret + 1).min(chars.len());
                    self.select_all = false;
                }
                Key::Home => {
                    self.caret = 0;
                    self.select_all = false;
                }
                Key::End => {
                    self.caret = chars.len();
                    self.select_all = false;
                }
                Key::Backspace | Key::Delete => {
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
                    return input_event(n, chars.into_iter().collect());
                }
                _ => {}
            }
            return None;
        }
        if n.kind == Kind::Slider && down {
            let current = n.progress;
            let next = match key {
                Key::Left => (current - 1).max(0),
                Key::Right => (current + 1).min(100),
                Key::Home => 0,
                Key::End => 100,
                _ => return None,
            };
            return n
                .change
                .as_ref()
                .map(|m| event(&n.key, "Select", vec![m.clone(), json!(next.to_string())]));
        }
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
    /// Platform IME commit passes through here; control characters are rejected.
    pub fn text(&mut self, view: &View, text: &str) -> Option<Event> {
        let n = self.focused.as_ref().and_then(|key| view.nodes.get(key))?;
        if !n.kind.editable() || n.disabled || n.busy || text.chars().any(char::is_control) {
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
        if value.len() > 16384 {
            return None;
        }
        self.select_all = false;
        self.caret = at + inserted.len();
        input_event(n, value)
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
        Kind::Checkbox | Kind::Switch => n
            .toggle
            .as_ref()
            .map(|m| event(key, "Toggle", vec![m.clone(), json!(!n.checked)])),
        Kind::Button | Kind::IconButton => n
            .activate
            .as_ref()
            .map(|m| event(key, "Activate", vec![m.clone()])),
        _ => None,
    }
}
fn slider(view: &View, scene: &Scene, key: &str, x: f32) -> Option<Event> {
    let n = &view.nodes[key];
    let b = scene.boxes[key].bounds;
    let value =
        (((x - b.x - 8.0) / (b.width - 16.0).max(1.0)).clamp(0.0, 1.0) * 100.0).round() as i32;
    n.change
        .as_ref()
        .map(|m| event(key, "Select", vec![m.clone(), json!(value.to_string())]))
}
