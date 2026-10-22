//! Validated, flat Ubi views and retained keyed reconciliation.
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Public validation/runtime error. Invalid updates never mutate a retained tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

/// Only implemented native components are represented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Window,
    View,
    Column,
    Row,
    Flex,
    Grid,
    ScrollView,
    Card,
    Text,
    Heading,
    Separator,
    Button,
    IconButton,
    Checkbox,
    Switch,
    Slider,
    TextInput,
    PasswordInput,
    Badge,
    ProgressBar,
    Radio,
    Tab,
    Toggle,
    ToggleItem,
    Link,
    TextArea,
    NumberInput,
    Avatar,
    Alert,
    Kbd,
    Skeleton,
    Label,
}
impl Kind {
    pub fn container(self) -> bool {
        matches!(
            self,
            Self::Window
                | Self::View
                | Self::Column
                | Self::Row
                | Self::Flex
                | Self::Grid
                | Self::ScrollView
                | Self::Card
        )
    }
    pub fn focusable(self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::IconButton
                | Self::Checkbox
                | Self::Switch
                | Self::Slider
                | Self::TextInput
                | Self::PasswordInput
                | Self::Radio
                | Self::Tab
                | Self::Toggle
                | Self::ToggleItem
                | Self::Link
                | Self::TextArea
                | Self::NumberInput
        )
    }
    pub fn editable(self) -> bool {
        matches!(
            self,
            Self::TextInput | Self::PasswordInput | Self::TextArea | Self::NumberInput
        )
    }
    pub fn role(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Button | Self::IconButton => "button",
            Self::Checkbox => "checkbox",
            Self::Switch => "switch",
            Self::Slider => "slider",
            Self::TextInput | Self::PasswordInput => "textbox",
            Self::Heading => "heading",
            Self::Separator => "separator",
            Self::ProgressBar => "progressbar",
            Self::Text => "text",
            Self::Radio => "radio",
            Self::Tab => "tab",
            Self::Toggle | Self::ToggleItem => "button",
            Self::Link => "link",
            Self::TextArea => "textbox",
            Self::NumberInput => "spinbutton",
            Self::Avatar => "img",
            Self::Alert => "alert",
            Self::Kbd | Self::Label => "text",
            _ => "group",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
}

/// Logical-pixel geometry, independent of renderer and platform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Self {
            x,
            y,
            width: ((self.x + self.width).min(other.x + other.width) - x).max(0.0),
            height: ((self.y + self.height).min(other.y + other.height) - y).max(0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub width: Length,
    pub height: Length,
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub gap: f32,
    pub padding: f32,
    pub grow: f32,
    pub columns: u16,
    pub absolute: Option<(f32, f32)>,
    pub center: bool,
    pub clip: bool,
    pub font_size: f32,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            width: Length::Auto,
            height: Length::Auto,
            min_width: 0.0,
            min_height: 0.0,
            max_width: None,
            max_height: None,
            gap: 12.0,
            padding: 0.0,
            grow: 0.0,
            columns: 2,
            absolute: None,
            center: false,
            clip: false,
            font_size: 14.0,
        }
    }
}

/// Normalized component properties; messages remain typed Ubi enum JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub key: String,
    pub kind: Kind,
    pub children: Vec<String>,
    pub style: Style,
    pub content: String,
    pub label: String,
    pub value: String,
    pub checked: bool,
    pub disabled: bool,
    pub busy: bool,
    pub pressed: bool,
    pub progress: i32,
    pub variant: String,
    pub tone: String,
    pub theme: Option<String>,
    pub activate: Option<Value>,
    pub input: Option<Value>,
    pub toggle: Option<Value>,
    pub change: Option<Value>,
    pub option: String,
    pub group: String,
    pub read_only: bool,
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub role: String,
    pub expanded: Option<bool>,
}

/// A connected, bounded tree with globally unique stable keys.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub root: String,
    pub nodes: BTreeMap<String, Node>,
    pub order: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tagged {
    tag: String,
    values: Vec<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    key: String,
    kind: Tagged,
    props: Vec<Tagged>,
    children: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawView {
    schema_version: u32,
    root: String,
    nodes: Vec<RawNode>,
}

impl View {
    /// Decode the source package's schema-1 View<M> ABI, rejecting unknown props,
    /// duplicate keys/properties, cycles, aliases, unreachable nodes and bad sizes.
    pub fn decode(value: Value) -> Result<Self, Error> {
        let raw: RawView =
            serde_json::from_value(value).map_err(|e| Error(format!("invalid view: {e}")))?;
        if raw.schema_version != 1 || raw.nodes.is_empty() || raw.nodes.len() > 4096 {
            return Err("unsupported view version or node count".into());
        }
        let mut nodes = BTreeMap::new();
        for raw in raw.nodes {
            if raw.key.is_empty()
                || raw.key.len() > 256
                || raw.key.chars().any(char::is_control)
                || !raw.kind.values.is_empty()
                || raw.props.len() > 64
            {
                return Err("invalid node key/kind/property count".into());
            }
            let kind = match raw.kind.tag.as_str() {
                "Window" => Kind::Window,
                "View" => Kind::View,
                "Column" => Kind::Column,
                "Row" => Kind::Row,
                "Flex" => Kind::Flex,
                "Grid" => Kind::Grid,
                "ScrollView" => Kind::ScrollView,
                "Card" => Kind::Card,
                "Text" => Kind::Text,
                "Heading" => Kind::Heading,
                "Separator" => Kind::Separator,
                "Button" => Kind::Button,
                "IconButton" => Kind::IconButton,
                "Checkbox" => Kind::Checkbox,
                "Switch" => Kind::Switch,
                "Slider" => Kind::Slider,
                "TextInput" => Kind::TextInput,
                "PasswordInput" => Kind::PasswordInput,
                "Badge" => Kind::Badge,
                "ProgressBar" => Kind::ProgressBar,
                "Radio" => Kind::Radio,
                "Tab" => Kind::Tab,
                "Toggle" => Kind::Toggle,
                "ToggleItem" => Kind::ToggleItem,
                "Link" => Kind::Link,
                "TextArea" => Kind::TextArea,
                "NumberInput" => Kind::NumberInput,
                "Avatar" => Kind::Avatar,
                "Alert" => Kind::Alert,
                "Kbd" => Kind::Kbd,
                "Skeleton" => Kind::Skeleton,
                "Label" => Kind::Label,
                name => return Err(Error(format!("unsupported native component: {name}"))),
            };
            if !kind.container() && !raw.children.is_empty() {
                return Err("leaf components cannot own children".into());
            }
            let mut node = Node {
                key: raw.key,
                kind,
                children: raw.children,
                style: Style::default(),
                content: String::new(),
                label: String::new(),
                value: String::new(),
                checked: false,
                disabled: false,
                busy: false,
                pressed: false,
                progress: 0,
                variant: "Primary".into(),
                tone: "Neutral".into(),
                theme: None,
                activate: None,
                input: None,
                toggle: None,
                change: None,
                option: String::new(),
                group: String::new(),
                read_only: false,
                min: i32::MIN,
                max: i32::MAX,
                step: 1,
                role: String::new(),
                expanded: None,
            };
            if kind == Kind::Heading {
                node.style.font_size = 26.0;
            }
            if kind == Kind::Window {
                node.style.padding = 32.0;
                node.style.gap = 24.0;
            }
            if kind == Kind::Card {
                node.style.padding = 24.0;
                node.style.gap = 16.0;
            }
            let mut seen = BTreeSet::new();
            for prop in raw.props {
                if !seen.insert(prop.tag.clone()) {
                    return Err(Error(format!("duplicate property: {}", prop.tag)));
                }
                let values = prop.values;
                let one = || {
                    if values.len() == 1 {
                        Ok(&values[0])
                    } else {
                        Err(Error("property requires one value".into()))
                    }
                };
                let text = || {
                    one()?
                        .as_str()
                        .filter(|s| s.len() <= 16384)
                        .map(str::to_owned)
                        .ok_or_else(|| Error("expected bounded text property".into()))
                };
                let boolean = || {
                    one()?
                        .as_bool()
                        .ok_or_else(|| Error("expected bool property".into()))
                };
                let number = || {
                    one()?
                        .as_i64()
                        .filter(|n| (0..=100_000).contains(n))
                        .map(|n| n as f32)
                        .ok_or_else(|| Error("expected size in 0..100000".into()))
                };
                match prop.tag.as_str() {
                    "Content" => node.content = text()?,
                    "Label" => node.label = text()?,
                    "Value" => node.value = text()?,
                    "Option" => node.option = text()?,
                    "Group" => node.group = text()?,
                    "ReadOnly" => node.read_only = boolean()?,
                    "Expanded" => node.expanded = Some(boolean()?),
                    "Selected" => node.checked = boolean()?,
                    "Min" | "Max" | "Step" => {
                        let n = one()?
                            .as_i64()
                            .and_then(|n| i32::try_from(n).ok())
                            .ok_or("expected 32-bit integer")?;
                        match prop.tag.as_str() {
                            "Min" => node.min = n,
                            "Max" => node.max = n,
                            _ => node.step = n,
                        }
                    }
                    "Role" => {
                        let role = text()?;
                        if ![
                            "group",
                            "radiogroup",
                            "tablist",
                            "tabpanel",
                            "region",
                            "alert",
                            "status",
                            "navigation",
                            "list",
                            "listitem",
                            "button",
                            "heading",
                            "link",
                            "text",
                            "label",
                        ]
                        .contains(&role.as_str())
                        {
                            return Err("unsupported semantic role".into());
                        }
                        node.role = role;
                    }
                    "Disabled" => node.disabled = boolean()?,
                    "Busy" => node.busy = boolean()?,
                    "Pressed" => node.pressed = boolean()?,
                    "Checked" => node.checked = boolean()?,
                    "Activate" => node.activate = Some(one()?.clone()),
                    "Input" => node.input = Some(one()?.clone()),
                    "Toggle" => node.toggle = Some(one()?.clone()),
                    "Change" => node.change = Some(one()?.clone()),
                    "Width" => node.style.width = Length::Px(number()?),
                    "Height" => node.style.height = Length::Px(number()?),
                    "WidthPercent" | "HeightPercent" => {
                        let n = number()?;
                        if n > 100.0 {
                            return Err("percent must be 0..100".into());
                        }
                        if prop.tag == "WidthPercent" {
                            node.style.width = Length::Percent(n / 100.0);
                        } else {
                            node.style.height = Length::Percent(n / 100.0);
                        }
                    }
                    "MinWidth" => node.style.min_width = number()?,
                    "MinHeight" => node.style.min_height = number()?,
                    "MaxWidth" => node.style.max_width = Some(number()?),
                    "MaxHeight" => node.style.max_height = Some(number()?),
                    "Padding" => node.style.padding = number()?,
                    "Grow" => node.style.grow = number()?,
                    "Columns" => {
                        let n = number()?;
                        if !(1.0..=64.0).contains(&n) {
                            return Err("columns must be 1..64".into());
                        }
                        node.style.columns = n as u16;
                    }
                    "FontSize" => {
                        let n = number()?;
                        if !(8.0..=96.0).contains(&n) {
                            return Err("font size must be 8..96".into());
                        }
                        node.style.font_size = n;
                    }
                    "Center" => node.style.center = boolean()?,
                    "Clip" => node.style.clip = boolean()?,
                    "Absolute" => {
                        if values.len() != 2 {
                            return Err("absolute requires x,y".into());
                        }
                        let coords = values
                            .iter()
                            .map(|v| {
                                v.as_i64()
                                    .filter(|n| n.unsigned_abs() <= 100_000)
                                    .map(|n| n as f32)
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or("invalid position")?;
                        node.style.absolute = Some((coords[0], coords[1]));
                    }
                    "Progress" => {
                        let n = number()?;
                        if n > 100.0 {
                            return Err("progress must be 0..100".into());
                        }
                        node.progress = n as i32;
                    }
                    "Level" => {
                        let n = number()?;
                        if !(1.0..=6.0).contains(&n) {
                            return Err("heading level must be 1..6".into());
                        }
                        node.style.font_size = [32.0, 26.0, 22.0, 18.0, 16.0, 14.0][n as usize - 1];
                    }
                    "Gap" => {
                        node.style.gap =
                            match variant(one()?, &["None", "Xs", "Sm", "Md", "Lg", "Xl", "Xxl"])?
                                .as_str()
                            {
                                "None" => 0.0,
                                "Xs" => 4.0,
                                "Sm" => 8.0,
                                "Md" => 12.0,
                                "Lg" => 16.0,
                                "Xl" => 24.0,
                                _ => 32.0,
                            };
                    }
                    "Variant" => {
                        node.variant = variant(
                            one()?,
                            &["Primary", "Secondary", "Outline", "Ghost", "Destructive"],
                        )?
                    }
                    "Tone" => {
                        node.tone = variant(
                            one()?,
                            &["Neutral", "Primary", "Success", "Warning", "Danger"],
                        )?
                    }
                    "Theme" => node.theme = Some(variant(one()?, &["Light", "Dark", "System"])?),
                    tag => return Err(Error(format!("unknown native property: {tag}"))),
                }
            }
            if node
                .style
                .max_width
                .is_some_and(|n| n < node.style.min_width)
                || node
                    .style
                    .max_height
                    .is_some_and(|n| n < node.style.min_height)
            {
                return Err("min dimension exceeds max".into());
            }
            if node.kind == Kind::IconButton && node.label.is_empty() {
                return Err("IconButton requires an accessible Label".into());
            }
            if node.min > node.max || node.step <= 0 {
                return Err("invalid numeric range or step".into());
            }
            if seen.contains("Checked") && seen.contains("Selected") {
                return Err("Checked and Selected cannot both be specified".into());
            }
            if nodes.insert(node.key.clone(), node).is_some() {
                return Err("duplicate node key".into());
            }
        }
        let mut view = Self {
            root: raw.root,
            nodes,
            order: Vec::new(),
        };
        let mut visited = BTreeSet::new();
        fn visit(
            view: &View,
            key: &str,
            depth: usize,
            seen: &mut BTreeSet<String>,
            order: &mut Vec<String>,
        ) -> Result<(), Error> {
            if depth > 32 || !seen.insert(key.into()) {
                return Err("tree depth exceeded, cycle or multiple parents".into());
            }
            let node = view.nodes.get(key).ok_or("missing child/root")?;
            order.push(key.into());
            for child in &node.children {
                visit(view, child, depth + 1, seen, order)?;
            }
            Ok(())
        }
        let mut order = Vec::new();
        visit(&view, &view.root, 0, &mut visited, &mut order)?;
        if visited.len() != view.nodes.len() {
            return Err("unreachable view node".into());
        }
        view.order = order;
        Ok(view)
    }
}

fn variant(value: &Value, options: &[&str]) -> Result<String, Error> {
    let tagged: Tagged =
        serde_json::from_value(value.clone()).map_err(|_| Error("invalid enum property".into()))?;
    if !tagged.values.is_empty() || !options.contains(&tagged.tag.as_str()) {
        return Err("invalid enum variant".into());
    }
    Ok(tagged.tag)
}

/// Lifecycle changes in deterministic document order. Kind changes remount keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub mounted: Vec<String>,
    pub updated: Vec<String>,
    pub unmounted: Vec<String>,
}
impl Changes {
    pub fn is_empty(&self) -> bool {
        self.mounted.is_empty() && self.updated.is_empty() && self.unmounted.is_empty()
    }
}
pub struct Tree {
    view: View,
    pub revision: u64,
}
impl Tree {
    pub fn new(view: View) -> Self {
        Self { view, revision: 1 }
    }
    pub fn view(&self) -> &View {
        &self.view
    }
    pub fn reconcile(&mut self, next: View) -> Changes {
        let mut delta = Changes::default();
        for key in &self.view.order {
            if next
                .nodes
                .get(key)
                .is_none_or(|n| n.kind != self.view.nodes[key].kind)
            {
                delta.unmounted.push(key.clone());
            }
        }
        for key in &next.order {
            match self.view.nodes.get(key) {
                None => delta.mounted.push(key.clone()),
                Some(old) if old.kind != next.nodes[key].kind => delta.mounted.push(key.clone()),
                Some(old) if old != &next.nodes[key] => delta.updated.push(key.clone()),
                _ => {}
            }
        }
        if !delta.is_empty() || self.view.root != next.root {
            self.revision += 1;
        }
        self.view = next;
        delta
    }
}

/// Platform-independent accessibility snapshot. Password values are redacted.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessibleNode {
    pub key: String,
    pub role: String,
    pub name: String,
    pub value: String,
    pub checked: bool,
    pub disabled: bool,
    pub focused: bool,
    pub bounds: Rect,
    pub children: Vec<String>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub pressed: Option<bool>,
    pub read_only: bool,
    pub multiline: bool,
}
impl Node {
    pub fn accessible(&self, bounds: Rect, focused: bool) -> AccessibleNode {
        AccessibleNode {
            key: self.key.clone(),
            role: if self.role.is_empty() {
                self.kind.role().into()
            } else {
                self.role.clone()
            },
            name: if self.label.is_empty() {
                self.content.clone()
            } else {
                self.label.clone()
            },
            value: if matches!(self.kind, Kind::Slider | Kind::ProgressBar) {
                self.progress.to_string()
            } else if self.kind == Kind::PasswordInput {
                String::new()
            } else {
                self.value.clone()
            },
            checked: self.checked,
            disabled: self.disabled || self.busy,
            focused,
            bounds,
            children: self.children.clone(),
            selected: if matches!(self.kind, Kind::Radio | Kind::Tab) {
                Some(self.checked)
            } else {
                None
            },
            expanded: self.expanded,
            pressed: if matches!(self.kind, Kind::Toggle | Kind::ToggleItem) {
                Some(self.checked)
            } else {
                None
            },
            read_only: self.read_only,
            multiline: self.kind == Kind::TextArea,
        }
    }
}
