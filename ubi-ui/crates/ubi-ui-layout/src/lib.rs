//! Retained Taffy layout, logical-pixel clipping and nested scrolling.
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
}

/// Layout nodes and Taffy caches survive keyed view updates. No-op frames reuse
/// computed geometry; scrolling only rebuilds translated/clipped scene boxes.
pub struct Layout {
    tree: TaffyTree<String>,
    ids: BTreeMap<String, NodeId>,
    previous: Option<View>,
    scroll: BTreeMap<String, f32>,
    viewport: (f32, f32),
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
            dirty: true,
            computations: 0,
        }
    }
}

impl Layout {
    pub fn sync(&mut self, view: &View) -> Result<(), Error> {
        if self.previous.as_ref() == Some(view) { return Ok(()); }
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
                    .new_leaf_with_context(style(node), key.clone())
                    .map_err(err)?;
                self.ids.insert(key.clone(), id);
                self.dirty = true;
            } else if self
                .previous
                .as_ref()
                .is_none_or(|p| p.nodes.get(key).is_none_or(|old| !layout_equal(old, node)))
            {
                self.tree
                    .set_style(self.ids[key], style(node))
                    .map_err(err)?;
                self.tree.mark_dirty(self.ids[key]).map_err(err)?;
                self.dirty = true;
            }
        }
        for key in &view.order {
            if self.previous.as_ref().is_none_or(|p| {
                p.nodes
                    .get(key)
                    .is_none_or(|n| n.children != view.nodes[key].children)
            }) {
                let children: Vec<_> = view.nodes[key]
                    .children
                    .iter()
                    .map(|k| self.ids[k])
                    .collect();
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
        if self.dirty || self.viewport != (width, height) {
            let root = self.ids[&view.root];
            let mut root_style = style(&view.nodes[&view.root]);
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
        let mut scene = Scene::default();
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
        let child_clip = if node.kind == Kind::ScrollView || node.style.clip {
            clip.intersect(bounds)
        } else {
            clip
        };
        for child in &node.children {
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
fn style(node: &Node) -> taffy::Style {
    let s = &node.style;
    let mut result = taffy::Style {
        display: if node.kind == Kind::Grid {
            Display::Grid
        } else {
            Display::Flex
        },
        flex_direction: if matches!(node.kind, Kind::Row | Kind::Flex) {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        flex_wrap: if node.kind == Kind::Flex {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        flex_shrink: 0.0,
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
    if node.kind == Kind::ScrollView {
        result.overflow = Point {
            x: Overflow::Hidden,
            y: Overflow::Scroll,
        };
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
}
fn intrinsic(node: &Node, max: f32, text: &mut impl TextMeasure) -> (f32, f32) {
    let (w, h) = text.measure(
        &node.content,
        node.style.font_size,
        if matches!(node.kind, Kind::Text | Kind::Heading) {
            max
        } else {
            f32::INFINITY
        },
    );
    match node.kind {
        Kind::Button | Kind::IconButton => (w + 32.0, 40.0),
        Kind::Checkbox | Kind::Switch => (w + 56.0, 40.0),
        Kind::TextInput | Kind::PasswordInput => (240.0, 40.0),
        Kind::Slider => (240.0, 40.0),
        Kind::Badge => (w + 20.0, 28.0),
        Kind::Separator => (1.0, 1.0),
        Kind::ProgressBar => (200.0, 8.0),
        Kind::Text | Kind::Heading => (w, h),
        _ => (0.0, 0.0),
    }
}
