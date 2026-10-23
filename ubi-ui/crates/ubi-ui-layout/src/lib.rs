//! Retained Taffy layout, logical-pixel clipping and nested scrolling.
mod presentation;
pub use presentation::{place_overlay, present, PresentedScene};
use std::collections::BTreeMap;
use taffy::prelude::*;
use taffy::{Overflow, Point};
use ubi_ui_core::{Error, Kind, Length, Node, Rect as Bounds, View};

/// Text backend supplies identical intrinsic measurement to layout and drawing.
pub trait TextMeasure {
    fn measure(&mut self, text: &str, size: f32, max_width: f32) -> (f32, f32);
}
#[derive(Clone, Debug, PartialEq)]
pub struct BoxLayout {
    pub bounds: Bounds,
    pub clip: Bounds,
    pub content_height: f32,
}
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub boxes: BTreeMap<String, BoxLayout>,
    pub order: Vec<String>,
    pub viewport: Bounds,
    pub context_points: BTreeMap<String, (f32, f32)>,
}
impl Scene {
    /// Stable semantic records for a future OS accessibility bridge.
    pub fn accessibility(
        &self,
        view: &View,
        focused: Option<&str>,
    ) -> Vec<ubi_ui_core::AccessibleNode> {
        let mut accessible: Vec<_> = self
            .order
            .iter()
            .filter_map(|key| {
                let node = view.nodes.get(key)?;
                let b = self.boxes.get(key)?;
                let mut accessible =
                    node.accessible(b.bounds.intersect(b.clip), focused == Some(key.as_str()));
                if let Some(labelled_by) = node
                    .labelled_by
                    .as_deref()
                    .and_then(|key| view.nodes.get(key))
                {
                    accessible.name = if labelled_by.label.is_empty() {
                        labelled_by.content.clone()
                    } else {
                        labelled_by.label.clone()
                    };
                }
                let description_key = node.described_by.as_deref().or_else(|| {
                    view.nodes
                        .values()
                        .find(|candidate| {
                            candidate.kind == Kind::Overlay
                                && candidate.open
                                && candidate.overlay_kind == Some(ubi_ui_core::OverlayKind::Tooltip)
                                && candidate.anchor.as_deref() == Some(node.key.as_str())
                        })
                        .map(|tooltip| tooltip.key.as_str())
                });
                accessible.description =
                    description_key
                        .and_then(|key| view.nodes.get(key))
                        .map(|description| {
                            if description.label.is_empty() {
                                description.content.clone()
                            } else {
                                description.label.clone()
                            }
                        });
                Some(accessible)
            })
            .collect();
        let hidden: std::collections::BTreeSet<_> = view
            .nodes
            .values()
            .filter(|node| {
                node.kind == Kind::Overlay && !node.open || node.disclosure == Some(false)
            })
            .flat_map(|node| descendants(view, &node.key))
            .collect();
        accessible.retain(|node| !hidden.contains(node.key.as_str()));
        accessible
    }
}

/// Layout nodes and Taffy caches survive keyed view updates. No-op frames reuse
/// computed geometry; scrolling only rebuilds translated/clipped scene boxes.
pub struct Layout {
    tree: TaffyTree<String>,
    ids: BTreeMap<String, NodeId>,
    previous: Option<View>,
    scroll: BTreeMap<String, f32>,
    viewport: (f32, f32),
    context_points: BTreeMap<String, (f32, f32)>,
    dirty: bool,
    pub computations: u64,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            tree: TaffyTree::new(),
            ids: BTreeMap::new(),
            previous: None,
            scroll: BTreeMap::new(),
            viewport: (0.0, 0.0),
            context_points: BTreeMap::new(),
            dirty: true,
            computations: 0,
        }
    }
}

impl Layout {
    /// Retain a pointer invocation point for an anchored ContextMenu.
    pub fn set_context_point(&mut self, key: &str, point: Option<(f32, f32)>) -> bool {
        match point {
            Some((x, y)) if x.is_finite() && y.is_finite() => {
                self.context_points.insert(key.to_owned(), (x, y)) != Some((x, y))
            }
            _ => self.context_points.remove(key).is_some(),
        }
    }

    pub fn sync(&mut self, view: &View) -> Result<(), Error> {
        if self.previous.as_ref() == Some(view) {
            return Ok(());
        }
        let removed: Vec<_> = self
            .ids
            .keys()
            .filter(|key| {
                view.nodes.get(*key).is_none_or(|n| {
                    self.previous
                        .as_ref()
                        .is_some_and(|p| p.nodes[*key].kind != n.kind)
                })
            })
            .cloned()
            .collect();
        for key in removed {
            self.tree
                .remove(self.ids.remove(&key).unwrap())
                .map_err(err)?;
            self.scroll.remove(&key);
            self.dirty = true;
        }
        for key in &view.order {
            let node = &view.nodes[key];
            if !self.ids.contains_key(key) {
                let id = self
                    .tree
                    .new_leaf_with_context(style(view, key, node), key.clone())
                    .map_err(err)?;
                self.ids.insert(key.clone(), id);
                self.dirty = true;
            } else if self
                .previous
                .as_ref()
                .is_none_or(|p| p.nodes.get(key).is_none_or(|old| !layout_equal(old, node)))
            {
                self.tree
                    .set_style(self.ids[key], style(view, key, node))
                    .map_err(err)?;
                self.tree.mark_dirty(self.ids[key]).map_err(err)?;
                self.dirty = true;
            }
        }
        for key in &view.order {
            let children: Vec<_> = view.nodes[key]
                .children
                .iter()
                .map(|k| self.ids[k])
                .collect();
            if self.tree.children(self.ids[key]).map_err(err)? != children {
                self.tree
                    .set_children(self.ids[key], &children)
                    .map_err(err)?;
                self.dirty = true;
            }
        }
        if self.previous.as_ref().is_none_or(|p| p.root != view.root) {
            self.dirty = true;
        }
        self.previous = Some(view.clone());
        Ok(())
    }

    pub fn compute(
        &mut self,
        view: &View,
        width: f32,
        height: f32,
        text: &mut impl TextMeasure,
    ) -> Result<Scene, Error> {
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || width > 32768.0
            || height > 32768.0
        {
            return Err("invalid logical viewport".into());
        }
        self.sync(view)?;
        self.context_points.retain(|key, _| {
            view.nodes.get(key).is_some_and(|node| {
                node.overlay_kind == Some(ubi_ui_core::OverlayKind::ContextMenu)
            })
        });
        if self.dirty || self.viewport != (width, height) {
            let root = self.ids[&view.root];
            let mut root_style = style(view, &view.root, &view.nodes[&view.root]);
            if view.nodes[&view.root].style.width == Length::Auto {
                root_style.size.width = Dimension::length(width);
            }
            if view.nodes[&view.root].style.height == Length::Auto {
                root_style.size.height = Dimension::length(height);
            }
            self.tree.set_style(root, root_style).map_err(err)?;
            self.tree
                .compute_layout_with_measure(
                    root,
                    Size {
                        width: AvailableSpace::Definite(width),
                        height: AvailableSpace::Definite(height),
                    },
                    |input, _, context, _| {
                        let Some(node) = context.and_then(|k| view.nodes.get(k)) else {
                            return taffy::LayoutOutput::from_outer_size(Size::ZERO);
                        };
                        let known = input.known_dimensions;
                        let max = known.width.unwrap_or(match input.available_space.width {
                            AvailableSpace::Definite(w) => w,
                            _ => f32::INFINITY,
                        });
                        let (w, h) = intrinsic(node, max, text);
                        taffy::LayoutOutput::from_outer_size(Size {
                            width: known.width.unwrap_or(w),
                            height: known.height.unwrap_or(h),
                        })
                    },
                )
                .map_err(err)?;
            self.viewport = (width, height);
            self.dirty = false;
            self.computations += 1;
        }
        let mut scene = Scene {
            viewport: Bounds {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            context_points: self.context_points.clone(),
            ..Scene::default()
        };
        self.collect(
            view,
            &view.root,
            0.0,
            0.0,
            Bounds {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            &mut scene,
        )?;
        presentation::place_initial_overlays(view, &mut scene);
        Ok(scene)
    }

    fn collect(
        &mut self,
        view: &View,
        key: &str,
        x: f32,
        y: f32,
        clip: Bounds,
        scene: &mut Scene,
    ) -> Result<(), Error> {
        let l = *self.tree.layout(self.ids[key]).map_err(err)?;
        let bounds = Bounds {
            x: x + l.location.x,
            y: y + l.location.y,
            width: l.size.width,
            height: l.size.height,
        };
        let node = &view.nodes[key];
        if node.kind == Kind::Overlay && !node.open {
            return Ok(());
        }
        let content_height = l.scrollable_overflow_rect.bottom.max(bounds.height);
        let scroll = self.scroll.entry(key.into()).or_default();
        *scroll = scroll.clamp(0.0, (content_height - bounds.height).max(0.0));
        let offset = if node.kind == Kind::ScrollView {
            *scroll
        } else {
            0.0
        };
        scene.order.push(key.into());
        scene.boxes.insert(
            key.into(),
            BoxLayout {
                bounds,
                clip,
                content_height,
            },
        );
        let child_clip =
            if node.kind == Kind::ScrollView || node.style.clip || node.disclosure.is_some() {
                clip.intersect(bounds)
            } else {
                clip
            };
        for (index, child) in node.children.iter().enumerate() {
            if node
                .selected_child_index()
                .is_some_and(|selected| selected != index)
            {
                continue;
            }
            self.collect(view, child, bounds.x, bounds.y - offset, child_clip, scene)?;
        }
        Ok(())
    }

    /// Scroll the deepest hit scroll container that can consume this delta.
    pub fn scroll_at(&mut self, view: &View, scene: &Scene, x: f32, y: f32, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        for key in scene.order.iter().rev() {
            let b = &scene.boxes[key];
            if view.nodes[key].kind != Kind::ScrollView
                || !b.bounds.intersect(b.clip).contains(x, y)
            {
                continue;
            }
            let old = *self.scroll.get(key).unwrap_or(&0.0);
            let new = (old + delta).clamp(0.0, (b.content_height - b.bounds.height).max(0.0));
            if old != new {
                self.scroll.insert(key.clone(), new);
                return true;
            }
        }
        false
    }
    pub fn node_count(&self) -> usize {
        self.ids.len()
    }
    /// Reveal a keyboard-focused descendant through nested scroll ancestors.
    pub fn reveal(&mut self, view: &View, scene: &Scene, key: &str) -> bool {
        let Some(layout) = scene.boxes.get(key) else {
            return false;
        };
        let mut target = layout.bounds;
        let mut current = key;
        let mut changed = false;
        for _ in 0..32 {
            let Some((parent, node)) = view
                .nodes
                .iter()
                .find(|(_, node)| node.children.iter().any(|child| child == current))
            else {
                break;
            };
            if node.kind == Kind::ScrollView {
                let b = &scene.boxes[parent];
                let delta = if target.y < b.bounds.y {
                    target.y - b.bounds.y
                } else {
                    (target.y + target.height - b.bounds.y - b.bounds.height).max(0.0)
                };
                let offset = self.scroll.entry(parent.clone()).or_default();
                let old = *offset;
                *offset =
                    (*offset + delta).clamp(0.0, (b.content_height - b.bounds.height).max(0.0));
                target.y -= *offset - old;
                changed |= *offset != old;
            }
            current = parent;
        }
        changed
    }
    pub fn scroll_offset(&self, key: &str) -> f32 {
        *self.scroll.get(key).unwrap_or(&0.0)
    }
}
fn err(e: taffy::TaffyError) -> Error {
    Error(format!("layout: {e}"))
}
fn dimension(l: Length) -> Dimension {
    match l {
        Length::Auto => Dimension::auto(),
        Length::Px(n) => Dimension::length(n),
        Length::Percent(n) => Dimension::percent(n),
    }
}
fn style(view: &View, key: &str, node: &Node) -> taffy::Style {
    let s = &node.style;
    let mut result = taffy::Style {
        display: if node.kind == Kind::Overlay && !node.open {
            Display::None
        } else if node.kind == Kind::Grid {
            Display::Grid
        } else {
            Display::Flex
        },
        flex_direction: if matches!(node.kind, Kind::Row | Kind::Flex | Kind::Pagination) {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        flex_wrap: if node.kind == Kind::Flex {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        flex_shrink: if s.grow > 0.0 { 1.0 } else { 0.0 },
        flex_basis: if s.grow > 0.0 && s.min_width > 0.0 {
            Dimension::length(s.min_width)
        } else {
            Dimension::auto()
        },
        flex_grow: s.grow,
        size: Size {
            width: dimension(s.width),
            height: dimension(s.height),
        },
        min_size: Size {
            width: LengthPercentageAuto::length(s.min_width),
            height: LengthPercentageAuto::length(s.min_height),
        },
        max_size: Size {
            width: s
                .max_width
                .map_or(LengthPercentageAuto::auto(), LengthPercentageAuto::length),
            height: s
                .max_height
                .map_or(LengthPercentageAuto::auto(), LengthPercentageAuto::length),
        },
        gap: Size {
            width: LengthPercentage::length(s.gap),
            height: LengthPercentage::length(s.gap),
        },
        padding: Rect {
            left: LengthPercentage::length(s.padding),
            right: LengthPercentage::length(s.padding),
            top: LengthPercentage::length(s.padding),
            bottom: LengthPercentage::length(s.padding),
        },
        align_items: if s.center {
            AlignItems::CENTER
        } else {
            AlignItems::STRETCH
        },
        justify_content: if s.center {
            JustifyContent::CENTER
        } else {
            JustifyContent::START
        },
        ..Default::default()
    };
    if node.kind == Kind::Grid {
        result.grid_template_columns = vec![fr(1.0); s.columns as usize];
    }
    if node.kind == Kind::TableRow {
        result.display = Display::Grid;
        let columns = view
            .nodes
            .values()
            .find(|parent| {
                parent.kind == Kind::Table && parent.children.iter().any(|child| child == key)
            })
            .and_then(|table| table.children.first())
            .and_then(|first| view.nodes.get(first))
            .map_or(1, |header| header.children.len().max(1));
        result.grid_template_columns = vec![fr(1.0); columns];
        result.gap.width = LengthPercentage::length(0.0);
        result.gap.height = LengthPercentage::length(0.0);
    }
    if node.kind == Kind::Table {
        result.gap.width = LengthPercentage::length(0.0);
        result.gap.height = LengthPercentage::length(0.0);
    }
    if matches!(node.kind, Kind::Avatar | Kind::Kbd | Kind::Link) {
        result.align_self = Some(AlignSelf::START);
    }
    if node.kind == Kind::Avatar {
        result.aspect_ratio = Some(1.0);
    }
    if node.kind == Kind::ScrollView {
        result.overflow = Point {
            x: Overflow::Hidden,
            y: Overflow::Scroll,
        };
    }
    if node.kind == Kind::Overlay {
        result.position = Position::Absolute;
    }
    if let Some((x, y)) = s.absolute {
        result.position = Position::Absolute;
        result.inset.left = LengthPercentageAuto::length(x);
        result.inset.top = LengthPercentageAuto::length(y);
    }
    result
}
fn layout_equal(a: &Node, b: &Node) -> bool {
    a.kind == b.kind
        && a.style == b.style
        && a.content == b.content
        && a.value == b.value
        && a.label == b.label
        && a.open == b.open
        && a.disclosure == b.disclosure
        && a.effect == b.effect
        && a.index == b.index
        && a.number == b.number
}

fn descendants<'a>(view: &'a View, root: &'a str) -> Vec<&'a str> {
    fn collect<'a>(view: &'a View, key: &'a str, output: &mut Vec<&'a str>) {
        output.push(key);
        if let Some(node) = view.nodes.get(key) {
            for child in &node.children {
                collect(view, child, output);
            }
        }
    }
    let mut result = Vec::new();
    collect(view, root, &mut result);
    result
}
fn intrinsic(node: &Node, max: f32, text: &mut impl TextMeasure) -> (f32, f32) {
    let (w, h) = text.measure(
        &node.content,
        node.style.font_size,
        if matches!(node.kind, Kind::Text | Kind::Heading | Kind::Label) {
            max
        } else {
            f32::INFINITY
        },
    );
    match node.kind {
        Kind::Button | Kind::IconButton | Kind::Tab | Kind::Toggle | Kind::ToggleItem => {
            (w + 32.0, 40.0)
        }
        Kind::MenuItem | Kind::OptionItem => (w + 32.0, 40.0),
        Kind::TableHeader | Kind::TableCell => (w + 16.0, 36.0),
        Kind::EffectSurface if node.effect == Some(ubi_ui_core::Effect::NumberTicker) => {
            let value = node.number.unwrap_or_default().to_string();
            let (w, h) = text.measure(&value, node.style.font_size, max);
            (w, h.max(24.0))
        }
        Kind::EffectSurface if node.effect == Some(ubi_ui_core::Effect::TextReveal) => {
            let (w, h) = text.measure(&node.content, node.style.font_size, max);
            (w, h.max(node.style.font_size * 1.5))
        }
        Kind::Overlay if node.children.is_empty() => {
            let label = if node.label.is_empty() {
                &node.content
            } else {
                &node.label
            };
            let (w, h) = text.measure(label, node.style.font_size, (max - 32.0).max(1.0));
            (
                (w + 32.0).min(max),
                h.max(node.style.font_size * 1.5) + 16.0,
            )
        }
        Kind::Checkbox | Kind::Switch | Kind::Radio => (w + 56.0, 40.0),
        Kind::TextInput | Kind::PasswordInput | Kind::NumberInput => (240.0, 40.0),
        Kind::TextArea => (280.0, 120.0),
        Kind::Avatar => (40.0, 40.0),
        Kind::Alert => {
            let (w, h) = text.measure(&node.content, node.style.font_size, (max - 58.0).max(1.0));
            (w + 58.0, (h + 24.0).max(72.0))
        }
        Kind::Kbd => (w + 16.0, 26.0),
        Kind::Skeleton => (200.0, 20.0),
        Kind::Label | Kind::Link => (w, h.max(24.0)),
        Kind::Slider => (240.0, 40.0),
        Kind::Badge => (w + 20.0, 28.0),
        Kind::Separator => (1.0, 1.0),
        Kind::ProgressBar => (200.0, 8.0),
        Kind::Text | Kind::Heading => (w, h),
        _ => (0.0, 0.0),
    }
}
