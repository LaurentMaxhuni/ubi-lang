use super::{Frame, Text, Vertex};
use fontdue::layout::GlyphRasterConfig;
use std::collections::{BTreeMap, HashMap};
use ubi_ui_core::{Effect, Error, Kind, Node, OverlayKind, Rect, View};
use ubi_ui_input::Input;
use ubi_ui_layout::{PresentedScene, Scene, TextMeasure};
use ubi_ui_theme::{Color, Theme};
const ATLAS: u32 = 2048;
pub(crate) struct Atlas {
    pub texture: wgpu::Texture,
    glyphs: HashMap<GlyphRasterConfig, [f32; 4]>,
    x: u32,
    y: u32,
    row: u32,
}
impl Atlas {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            texture: device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Ubi glyph atlas"),
                size: wgpu::Extent3d {
                    width: ATLAS,
                    height: ATLAS,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }),
            glyphs: HashMap::new(),
            x: 1,
            y: 1,
            row: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }
    pub fn reset(&mut self, queue: &wgpu::Queue) {
        self.glyphs.clear();
        self.x = 1;
        self.y = 1;
        self.row = 0;
        queue.write_texture(
            self.texture.as_image_copy(),
            &vec![0; (ATLAS * ATLAS) as usize],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS),
                rows_per_image: Some(ATLAS),
            },
            wgpu::Extent3d {
                width: ATLAS,
                height: ATLAS,
                depth_or_array_layers: 1,
            },
        );
    }
    pub fn nearly_full(&self) -> bool {
        self.y + self.row + 256 >= ATLAS
    }
    fn glyph(
        &mut self,
        text: &Text,
        queue: &wgpu::Queue,
        key: GlyphRasterConfig,
    ) -> Result<[f32; 4], Error> {
        if let Some(rect) = self.glyphs.get(&key) {
            return Ok(*rect);
        }
        let (metrics, bitmap) = text.font.rasterize_config(key);
        let w = metrics.width as u32;
        let h = metrics.height as u32;
        if w == 0 || h == 0 {
            return Ok([0.0; 4]);
        }
        if self.x + w + 1 >= ATLAS {
            self.x = 1;
            self.y += self.row + 2;
            self.row = 0;
        }
        if self.y + h + 1 >= ATLAS {
            return Err("glyph atlas capacity exceeded; reduce simultaneous text sizes".into());
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.x,
                    y: self.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let uv = [
            self.x as f32 / ATLAS as f32,
            self.y as f32 / ATLAS as f32,
            (self.x + w) as f32 / ATLAS as f32,
            (self.y + h) as f32 / ATLAS as f32,
        ];
        self.x += w + 2;
        self.row = self.row.max(h);
        self.glyphs.insert(key, uv);
        Ok(uv)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    view: &View,
    scene: &Scene,
    input: &Input,
    theme: &Theme,
    system_dark: bool,
    dpi: f32,
    viewport: (f32, f32),
    presented: Option<&PresentedScene>,
    text: &mut Text,
    atlas: &mut Atlas,
    queue: &wgpu::Queue,
) -> Result<Frame, Error> {
    let mut p = Painter {
        text,
        atlas,
        queue,
        dpi,
        viewport,
        vertices: Vec::new(),
        alpha: 1.0,
        channels: presented.map_or_else(BTreeMap::new, |frame| frame.channels.clone()),
    };
    let mut themes = HashMap::new();
    themes.insert(
        view.root.clone(),
        theme.inherit(view.nodes[&view.root].theme.as_deref(), system_dark),
    );
    let clear = themes[&view.root].background;
    let mut overlay_keys = std::collections::BTreeSet::new();
    fn add_overlay_tree(view: &View, key: &str, keys: &mut std::collections::BTreeSet<String>) {
        if !keys.insert(key.into()) {
            return;
        }
        if let Some(node) = view.nodes.get(key) {
            for child in &node.children {
                add_overlay_tree(view, child, keys);
            }
        }
    }
    for node in view
        .nodes
        .values()
        .filter(|node| node.kind == Kind::Overlay && node.open)
    {
        add_overlay_tree(view, &node.key, &mut overlay_keys);
    }
    overlay_keys.retain(|key| scene.boxes.contains_key(key));
    for key in &scene.order {
        let node = &view.nodes[key];
        let t = themes
            .get(key)
            .cloned()
            .unwrap_or_else(|| theme.clone())
            .inherit(node.theme.as_deref(), system_dark);
        for child in &node.children {
            themes.insert(child.clone(), t.clone());
        }
    }
    for key in scene
        .order
        .iter()
        .filter(|key| !overlay_keys.contains(*key))
    {
        let node = &view.nodes[key];
        let b = &scene.boxes[key];
        if b.bounds.intersect(b.clip).width <= 0.0 || b.bounds.intersect(b.clip).height <= 0.0 {
            continue;
        }
        let t = themes
            .get(key)
            .cloned()
            .unwrap_or_else(|| theme.clone())
            .inherit(node.theme.as_deref(), system_dark);
        p.alpha = presented
            .and_then(|presented| presented.opacity.get(key))
            .copied()
            .unwrap_or(node.opacity as f32)
            .clamp(0.0, 1.0);
        p.component(node, b.bounds, b.clip, input, &t)?;
        if node.kind == Kind::ScrollView && b.content_height > b.bounds.height {
            let height = (b.bounds.height * b.bounds.height / b.content_height).max(16.0);
            let first = node
                .children
                .first()
                .and_then(|k| scene.boxes.get(k))
                .map_or(b.bounds.y, |b| b.bounds.y);
            let offset = (b.bounds.y - first).max(0.0);
            let travel = (b.bounds.height - height).max(0.0);
            p.rect(
                Rect {
                    x: b.bounds.x + b.bounds.width - 5.0,
                    y: b.bounds.y + offset / (b.content_height - b.bounds.height) * travel,
                    width: 3.0,
                    height,
                },
                t.border,
                2.0,
                b.clip,
            );
        }
    }
    if let Some(presented) = presented {
        let no_input = Input::default();
        for exit in &presented.exits {
            let opacity = exit.channels.get("opacity").copied().unwrap_or(1.0) as f32;
            for entry in &exit.snapshot.nodes {
                let clip = entry.clip.intersect(scene.viewport);
                if entry.bounds.intersect(clip).width <= 0.0
                    || entry.bounds.intersect(clip).height <= 0.0
                {
                    continue;
                }
                let node_theme = themes
                    .get(&exit.snapshot.root)
                    .cloned()
                    .unwrap_or_else(|| theme.clone())
                    .inherit(entry.node.theme.as_deref(), system_dark);
                p.alpha = (opacity * entry.node.opacity as f32).clamp(0.0, 1.0);
                p.component(&entry.node, entry.bounds, clip, &no_input, &node_theme)?;
            }
        }
    }
    for key in scene.order.iter().filter(|key| {
        view.nodes
            .get(*key)
            .is_some_and(|node| has_selection_indicator(view, node))
    }) {
        let Some(values) = p.channels.get(key) else {
            continue;
        };
        let (Some(x), Some(width)) = (
            values.get("tab-indicator-x"),
            values.get("tab-indicator-width"),
        ) else {
            continue;
        };
        let Some(box_layout) = scene.boxes.get(key) else {
            continue;
        };
        let b = box_layout.bounds;
        if *width <= 0.0 {
            continue;
        }
        let t = themes.get(key).cloned().unwrap_or_else(|| theme.clone());
        p.alpha = presented
            .and_then(|frame| frame.opacity.get(key))
            .copied()
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        p.rect(
            Rect {
                x: b.x + *x,
                y: b.y + b.height - 2.0,
                width: *width,
                height: 2.0,
            },
            t.accent,
            1.0,
            box_layout.clip,
        );
    }
    for key in scene.order.iter().filter(|key| overlay_keys.contains(*key)) {
        let node = &view.nodes[key];
        let b = &scene.boxes[key];
        if b.bounds.intersect(b.clip).width <= 0.0 || b.bounds.intersect(b.clip).height <= 0.0 {
            continue;
        }
        let t = themes
            .get(key)
            .cloned()
            .unwrap_or_else(|| theme.clone())
            .inherit(node.theme.as_deref(), system_dark);
        p.alpha = presented
            .and_then(|presented| presented.opacity.get(key))
            .copied()
            .unwrap_or(node.opacity as f32)
            .clamp(0.0, 1.0);
        p.component(node, b.bounds, b.clip, input, &t)?;
    }
    if p.vertices.len() > 600_000 {
        return Err("native frame primitive limit exceeded".into());
    }
    Ok(Frame {
        vertices: p.vertices,
        clear,
    })
}
struct Painter<'a> {
    text: &'a mut Text,
    atlas: &'a mut Atlas,
    queue: &'a wgpu::Queue,
    dpi: f32,
    viewport: (f32, f32),
    vertices: Vec<Vertex>,
    alpha: f32,
    channels: BTreeMap<String, BTreeMap<String, f32>>,
}

fn has_selection_indicator(view: &View, node: &Node) -> bool {
    if node.kind != Kind::Row {
        return false;
    }
    if node.role == "tablist" {
        return node.children.iter().any(|key| {
            view.nodes
                .get(key)
                .is_some_and(|child| child.kind == Kind::Tab && child.checked)
        });
    }
    node.role == "group"
        && !node.children.is_empty()
        && node.children.iter().all(|key| {
            view.nodes
                .get(key)
                .is_some_and(|child| child.kind == Kind::ToggleItem && child.group == node.key)
        })
        && node
            .children
            .iter()
            .filter_map(|key| view.nodes.get(key))
            .filter(|child| child.checked)
            .count()
            == 1
}

fn revealed_word_prefix(text: &str, progress: f32) -> &str {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if progress >= 1.0 {
        return text;
    }
    if progress <= 0.0 {
        return "";
    }
    let words = text.split_whitespace().count();
    let visible_words = ((words as f32 * progress).ceil() as usize).min(words);
    if visible_words == 0 {
        return "";
    }
    let mut word_count = 0;
    let mut in_word = false;
    let mut end = 0;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            in_word = false;
            if word_count == 0 {
                end = index + character.len_utf8();
            }
        } else {
            if !in_word {
                word_count += 1;
                in_word = true;
            }
            if word_count <= visible_words {
                end = index + character.len_utf8();
            }
        }
    }
    &text[..end]
}

impl Painter<'_> {
    fn channel(&self, key: &str, channel: &str) -> Option<f32> {
        self.channels.get(key)?.get(channel).copied()
    }
    fn quad(&mut self, b: Rect, color: Color, radius: f32, clip: Rect, uv: [f32; 4], glyph: bool) {
        if self.vertices.len() >= 600_006 {
            return;
        }
        for (x, y) in [
            (0.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (0.0, 1.0),
            (1.0, 0.0),
            (1.0, 1.0),
        ] {
            let world = [b.x + x * b.width, b.y + y * b.height];
            let mut rgba = color.array();
            rgba[3] *= self.alpha;
            self.vertices.push(Vertex {
                position: [
                    world[0] / self.viewport.0 * 2.0 - 1.0,
                    1.0 - world[1] / self.viewport.1 * 2.0,
                ],
                world,
                local: [x * b.width, y * b.height],
                size: [b.width, b.height],
                uv: [uv[0] + x * (uv[2] - uv[0]), uv[1] + y * (uv[3] - uv[1])],
                color: rgba,
                clip: [clip.x, clip.y, clip.x + clip.width, clip.y + clip.height],
                params: [radius, if glyph { 1.0 } else { 0.0 }],
            });
        }
    }
    fn rect(&mut self, b: Rect, color: Color, r: f32, clip: Rect) {
        self.quad(b, color, r, clip, [0.0; 4], false);
    }
    fn border(&mut self, b: Rect, fill: Color, border: Color, r: f32, clip: Rect) {
        self.rect(b, border, r, clip);
        self.rect(
            Rect {
                x: b.x + 1.0,
                y: b.y + 1.0,
                width: (b.width - 2.0).max(0.0),
                height: (b.height - 2.0).max(0.0),
            },
            fill,
            (r - 1.0).max(0.0),
            clip,
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn label(
        &mut self,
        text: &str,
        b: Rect,
        size: f32,
        color: Color,
        clip: Rect,
        center: bool,
        wrap: bool,
    ) -> Result<(), Error> {
        self.label_aligned(text, b, size, color, clip, center, wrap, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn label_aligned(
        &mut self,
        text: &str,
        b: Rect,
        size: f32,
        color: Color,
        clip: Rect,
        center: bool,
        wrap: bool,
        center_y: bool,
    ) -> Result<(), Error> {
        let max = if wrap {
            b.width * self.dpi
        } else {
            f32::INFINITY
        };
        let (glyphs, height) = self.text.glyphs(text, size * self.dpi, max);
        let width = glyphs
            .iter()
            .map(|g| g.x + g.width as f32)
            .fold(0.0, f32::max)
            / self.dpi;
        let x = b.x
            + if center {
                ((b.width - width) * 0.5).max(0.0)
            } else {
                0.0
            };
        let y = if center_y {
            b.y + ((b.height - height / self.dpi) * 0.5).max(0.0)
        } else {
            b.y
        };
        for g in glyphs {
            if g.width == 0 || g.height == 0 {
                continue;
            }
            let uv = self.atlas.glyph(self.text, self.queue, g.key)?;
            self.quad(
                Rect {
                    x: x + g.x / self.dpi,
                    y: y + g.y / self.dpi,
                    width: g.width as f32 / self.dpi,
                    height: g.height as f32 / self.dpi,
                },
                color,
                0.0,
                clip,
                uv,
                true,
            );
        }
        Ok(())
    }
    fn component(
        &mut self,
        n: &Node,
        b: Rect,
        clip: Rect,
        input: &Input,
        t: &Theme,
    ) -> Result<(), Error> {
        let disabled = n.disabled || n.busy;
        let hovered = input.hovered.as_deref() == Some(&n.key) && !disabled;
        let pressed = (input.pressed(&n.key) || n.pressed) && !disabled;
        let focused = input.focused.as_deref() == Some(&n.key);
        let fg = if disabled {
            t.muted.mix(t.surface, 0.35)
        } else {
            t.text
        };
        if focused {
            self.rect(
                Rect {
                    x: b.x - 3.0,
                    y: b.y - 3.0,
                    width: b.width + 6.0,
                    height: b.height + 6.0,
                },
                t.accent,
                t.radius + 3.0,
                clip,
            );
            self.rect(b, t.surface, t.radius, clip);
        }
        match n.kind {
            Kind::Window => {}
            Kind::Card => {
                self.rect(
                    Rect {
                        x: b.x,
                        y: b.y + 2.0,
                        ..b
                    },
                    t.shadow,
                    t.card_radius,
                    clip,
                );
                self.border(b, t.surface, t.border, t.card_radius, clip);
            }
            Kind::Button | Kind::IconButton | Kind::Tab | Kind::Toggle | Kind::ToggleItem => {
                if n.expanded.is_some() {
                    let fill = if hovered { t.raised } else { t.surface };
                    self.rect(b, fill, t.radius, clip);
                    self.rect(
                        Rect {
                            x: b.x,
                            y: b.y + b.height - 1.0,
                            width: b.width,
                            height: 1.0,
                        },
                        t.border,
                        0.0,
                        clip,
                    );
                    self.label(
                        &n.content,
                        Rect {
                            x: b.x + 12.0,
                            width: (b.width - 48.0).max(0.0),
                            ..b
                        },
                        n.style.font_size,
                        fg,
                        clip,
                        false,
                        false,
                    )?;
                    self.label(
                        if n.checked { "−" } else { "+" },
                        Rect {
                            x: b.x + b.width - 32.0,
                            width: 20.0,
                            ..b
                        },
                        18.0,
                        fg,
                        clip,
                        true,
                        false,
                    )?;
                    return Ok(());
                }
                let selection = self
                    .channel(&n.key, "tab-selection")
                    .unwrap_or(if n.checked { 1.0 } else { 0.0 })
                    .clamp(0.0, 1.0);
                let (fill, ink) = if matches!(n.kind, Kind::Tab | Kind::Toggle | Kind::ToggleItem) {
                    (t.surface.mix(t.raised, selection), fg)
                } else {
                    match n.variant.as_str() {
                        "Primary" => (t.accent, t.on_accent),
                        "Destructive" => (t.danger, t.on_accent),
                        "Ghost" => (if hovered { t.raised } else { t.surface }, fg),
                        _ => (t.surface, fg),
                    }
                };
                let fill = if disabled {
                    fill.mix(t.surface, 0.65)
                } else if pressed {
                    fill.mix(t.text, 0.18)
                } else if hovered {
                    fill.mix(t.text, 0.08)
                } else {
                    fill
                };
                self.border(
                    b,
                    fill,
                    if n.variant == "Outline" {
                        t.border
                    } else {
                        fill
                    },
                    t.radius,
                    clip,
                );
                self.label(
                    if n.busy { "Working…" } else { &n.content },
                    b,
                    n.style.font_size,
                    if disabled { t.muted } else { ink },
                    clip,
                    true,
                    false,
                )?;
                if n.kind == Kind::Tab
                    && n.checked
                    && self.channel(&n.key, "tab-selection").is_none()
                {
                    self.rect(
                        Rect {
                            x: b.x + 8.0,
                            y: b.y + b.height - 2.0,
                            width: (b.width - 16.0).max(0.0),
                            height: 2.0,
                        },
                        t.accent,
                        1.0,
                        clip,
                    );
                }
            }
            Kind::Text | Kind::Heading | Kind::Label => {
                self.label(&n.content, b, n.style.font_size, fg, clip, false, true)?
            }
            Kind::Badge => {
                let tone = t.tone(&n.tone);
                self.rect(b, t.surface.mix(tone, 0.12), b.height * 0.5, clip);
                self.label(&n.content, b, 12.0, tone, clip, true, false)?;
            }
            Kind::Link => {
                let ink = if disabled { t.muted } else { t.accent };
                self.label(&n.content, b, n.style.font_size, ink, clip, false, false)?;
                if hovered || focused {
                    let (w, _) = self
                        .text
                        .measure(&n.content, n.style.font_size, f32::INFINITY);
                    self.rect(
                        Rect {
                            x: b.x,
                            y: b.y + b.height - 2.0,
                            width: w.min(b.width),
                            height: 1.0,
                        },
                        ink,
                        0.0,
                        clip,
                    );
                }
            }
            Kind::Avatar => {
                self.border(b, t.raised, t.border, b.height.min(b.width) * 0.5, clip);
                self.label(&n.content, b, 16.0, t.text, clip, true, false)?;
            }
            Kind::Kbd => {
                self.rect(Rect { y: b.y + 2.0, ..b }, t.border, 5.0, clip);
                self.border(b, t.surface, t.border, 5.0, clip);
                self.label(&n.content, b, 12.0, t.muted, clip, true, false)?;
            }
            Kind::Skeleton => {
                self.rect(b, t.raised, t.radius.min(b.height * 0.5), clip);
                if let Some(phase) = self.channel(&n.key, "skeleton-phase") {
                    let width = (b.width * 0.32).clamp(18.0, 64.0).min(b.width);
                    let x = b.x - width + (b.width + width) * phase.clamp(0.0, 1.0);
                    let alpha = self.alpha;
                    self.alpha *= 0.18;
                    self.rect(
                        Rect { x, width, ..b },
                        t.raised.mix(t.text, 0.2),
                        t.radius.min(b.height * 0.5),
                        clip.intersect(b),
                    );
                    self.alpha = alpha;
                }
            }
            Kind::Alert => {
                let tone = t.tone(&n.tone);
                self.border(
                    b,
                    t.surface.mix(tone, 0.06),
                    t.border.mix(tone, 0.3),
                    t.radius,
                    clip,
                );
                self.label(
                    "!",
                    Rect {
                        x: b.x + 14.0,
                        y: b.y + 14.0,
                        width: 18.0,
                        height: 24.0,
                    },
                    16.0,
                    tone,
                    clip,
                    true,
                    false,
                )?;
                self.label(
                    &n.content,
                    Rect {
                        x: b.x + 42.0,
                        y: b.y + 12.0,
                        width: (b.width - 58.0).max(0.0),
                        height: (b.height - 24.0).max(0.0),
                    },
                    n.style.font_size,
                    fg,
                    clip,
                    false,
                    true,
                )?;
            }
            Kind::Radio => {
                let selection = self
                    .channel(&n.key, "tab-selection")
                    .unwrap_or(if n.checked { 1.0 } else { 0.0 })
                    .clamp(0.0, 1.0);
                let control = Rect {
                    x: b.x + 2.0,
                    y: b.y + (b.height - 20.0) * 0.5,
                    width: 20.0,
                    height: 20.0,
                };
                self.border(
                    control,
                    t.surface,
                    t.border.mix(t.accent, selection),
                    10.0,
                    clip,
                );
                if selection > 0.0 {
                    let alpha = self.alpha;
                    self.alpha *= selection;
                    self.rect(
                        Rect {
                            x: control.x + 5.0,
                            y: control.y + 5.0,
                            width: 10.0,
                            height: 10.0,
                        },
                        if disabled { t.muted } else { t.accent },
                        5.0,
                        clip,
                    );
                    self.alpha = alpha;
                }
                self.label(
                    &n.content,
                    Rect {
                        x: b.x + 36.0,
                        width: (b.width - 36.0).max(0.0),
                        ..b
                    },
                    n.style.font_size,
                    fg,
                    clip,
                    false,
                    false,
                )?;
            }
            Kind::Separator => self.rect(b, t.border, 0.0, clip),
            Kind::Checkbox | Kind::Switch => {
                let switch_position = self
                    .channel(&n.key, "switch-thumb")
                    .unwrap_or(if n.checked { 1.0 } else { 0.0 })
                    .clamp(0.0, 1.0);
                let control = Rect {
                    x: b.x,
                    y: b.y + (b.height - 22.0) * 0.5,
                    width: if n.kind == Kind::Switch { 38.0 } else { 22.0 },
                    height: 22.0,
                };
                let fill = if n.kind == Kind::Switch {
                    t.raised.mix(t.accent, switch_position)
                } else if n.checked {
                    if disabled {
                        t.muted
                    } else {
                        t.accent
                    }
                } else {
                    t.raised
                };
                self.border(
                    control,
                    fill,
                    if n.kind == Kind::Switch || n.checked {
                        fill
                    } else {
                        t.border
                    },
                    if n.kind == Kind::Switch { 11.0 } else { 5.0 },
                    clip,
                );
                if n.kind == Kind::Switch {
                    self.rect(
                        Rect {
                            x: control.x + 3.0 + switch_position * 16.0,
                            y: control.y + 3.0,
                            width: 16.0,
                            height: 16.0,
                        },
                        if switch_position > 0.5 {
                            t.on_accent
                        } else {
                            t.muted
                        },
                        8.0,
                        clip,
                    );
                } else if n.checked {
                    self.label("✓", control, 16.0, t.on_accent, clip, true, false)?;
                }
                self.label(
                    &n.content,
                    Rect {
                        x: b.x + 50.0,
                        width: (b.width - 50.0).max(0.0),
                        ..b
                    },
                    n.style.font_size,
                    fg,
                    clip,
                    false,
                    false,
                )?;
            }
            Kind::Slider => {
                let track = Rect {
                    x: b.x + 8.0,
                    y: b.y + b.height * 0.5 - 2.0,
                    width: (b.width - 16.0).max(0.0),
                    height: 4.0,
                };
                self.rect(track, t.border, 2.0, clip);
                self.rect(
                    Rect {
                        width: track.width * n.progress as f32 / 100.0,
                        ..track
                    },
                    if disabled { t.muted } else { t.accent },
                    2.0,
                    clip,
                );
                self.border(
                    Rect {
                        x: track.x + track.width * n.progress as f32 / 100.0 - 8.0,
                        y: b.y + b.height * 0.5 - 8.0,
                        width: 16.0,
                        height: 16.0,
                    },
                    t.surface,
                    if disabled { t.muted } else { t.accent },
                    8.0,
                    clip,
                );
            }
            Kind::TextInput | Kind::PasswordInput | Kind::NumberInput => {
                self.border(
                    b,
                    if n.read_only { t.raised } else { t.surface },
                    if focused {
                        t.accent
                    } else if hovered {
                        t.muted
                    } else {
                        t.border
                    },
                    t.radius,
                    clip,
                );
                let inner = Rect {
                    x: b.x + 12.0,
                    y: b.y,
                    width: (b.width - 24.0).max(0.0),
                    height: b.height,
                };
                let text_clip = clip.intersect(inner);
                let value = if n.value.is_empty() {
                    n.content.clone()
                } else if n.kind == Kind::PasswordInput {
                    "•".repeat(n.value.chars().count())
                } else {
                    n.value.clone()
                };
                let prefix: String = if n.value.is_empty() {
                    String::new()
                } else {
                    value.chars().take(input.caret).collect()
                };
                let (caret_width, _) = self.text.measure(&prefix, n.style.font_size, f32::INFINITY);
                let offset = if focused {
                    (caret_width - inner.width + 2.0).max(0.0)
                } else {
                    0.0
                };
                let inner = Rect {
                    x: inner.x - offset,
                    ..inner
                };
                if focused && input.select_all {
                    let (w, _) = self.text.measure(&value, n.style.font_size, f32::INFINITY);
                    self.rect(
                        Rect {
                            x: inner.x,
                            y: b.y + 8.0,
                            width: w,
                            height: b.height - 16.0,
                        },
                        t.accent.mix(t.surface, 0.7),
                        2.0,
                        text_clip,
                    );
                }
                self.label(
                    &value,
                    inner,
                    n.style.font_size,
                    if n.value.is_empty() { t.muted } else { fg },
                    text_clip,
                    false,
                    false,
                )?;
                if focused && !n.read_only {
                    self.rect(
                        Rect {
                            x: inner.x + caret_width,
                            y: b.y + 10.0,
                            width: 1.0,
                            height: b.height - 20.0,
                        },
                        t.accent,
                        0.0,
                        text_clip,
                    );
                }
            }
            Kind::TextArea => self.text_area(n, b, clip, input, t, fg)?,
            Kind::ProgressBar => {
                let progress = self
                    .channel(&n.key, "progress-fill")
                    .unwrap_or(n.progress as f32 / 100.0)
                    .clamp(0.0, 1.0);
                self.rect(b, t.raised, b.height * 0.5, clip);
                self.rect(
                    Rect {
                        width: b.width * progress,
                        ..b
                    },
                    t.accent,
                    b.height * 0.5,
                    clip,
                );
            }
            Kind::Accordion | Kind::Collapsible => {
                self.border(b, t.surface, t.border, t.radius, clip);
                if !n.content.is_empty() {
                    self.label(
                        &n.content,
                        Rect {
                            x: b.x + 12.0,
                            width: (b.width - 44.0).max(0.0),
                            ..b
                        },
                        n.style.font_size,
                        fg,
                        clip,
                        false,
                        false,
                    )?;
                    self.label(
                        if n.disclosure.unwrap_or(n.expanded.unwrap_or(false)) {
                            "−"
                        } else {
                            "+"
                        },
                        Rect {
                            x: b.x + b.width - 32.0,
                            width: 20.0,
                            ..b
                        },
                        18.0,
                        fg,
                        clip,
                        true,
                        false,
                    )?;
                }
            }
            Kind::Overlay => {
                let modal = matches!(
                    n.overlay_kind,
                    Some(
                        OverlayKind::Dialog
                            | OverlayKind::AlertDialog
                            | OverlayKind::Drawer
                            | OverlayKind::CommandPalette
                    )
                );
                if modal {
                    let dim = Color(0.0, 0.0, 0.0, 0.34);
                    self.rect(
                        Rect {
                            x: 0.0,
                            y: 0.0,
                            width: self.viewport.0,
                            height: self.viewport.1,
                        },
                        dim,
                        0.0,
                        Rect {
                            x: 0.0,
                            y: 0.0,
                            width: self.viewport.0,
                            height: self.viewport.1,
                        },
                    );
                }
                let surface = if n.overlay_kind == Some(OverlayKind::Toast) {
                    t.raised
                } else {
                    t.surface
                };
                self.border(b, surface, t.border, t.radius, clip);
                if n.overlay_kind == Some(OverlayKind::Toast) {
                    self.rect(
                        Rect {
                            x: b.x,
                            y: b.y,
                            width: 3.0,
                            height: b.height,
                        },
                        t.tone(&n.tone),
                        t.radius,
                        clip,
                    );
                }
                if !n.label.is_empty() && n.children.is_empty() {
                    self.label(
                        &n.label,
                        Rect { x: b.x + 16.0, ..b },
                        n.style.font_size,
                        fg,
                        clip,
                        false,
                        true,
                    )?;
                }
            }
            Kind::MenuItem | Kind::OptionItem => {
                let selected = n.checked || input.focused.as_deref() == Some(&n.key);
                if hovered || pressed || selected {
                    self.rect(b, t.raised, t.radius.min(6.0), clip);
                }
                if n.kind == Kind::OptionItem && n.checked {
                    self.label(
                        "✓",
                        Rect {
                            x: b.x + 8.0,
                            width: 18.0,
                            ..b
                        },
                        13.0,
                        t.accent,
                        clip,
                        true,
                        false,
                    )?;
                }
                self.label(
                    &n.content,
                    Rect {
                        x: b.x
                            + if n.kind == Kind::OptionItem {
                                32.0
                            } else {
                                12.0
                            },
                        width: (b.width - 44.0).max(0.0),
                        ..b
                    },
                    n.style.font_size,
                    fg,
                    clip,
                    false,
                    false,
                )?;
                if n.disabled {
                    self.label(
                        "Disabled",
                        Rect {
                            x: b.x + b.width - 74.0,
                            width: 62.0,
                            ..b
                        },
                        10.0,
                        t.muted,
                        clip,
                        true,
                        false,
                    )?;
                }
            }
            Kind::Pagination => {
                self.border(b, t.surface, t.border, t.radius, clip);
                if let (Some(page), Some(count)) = (n.page, n.page_count) {
                    let label = format!("Page {page} of {count}");
                    self.label(&label, b, n.style.font_size, fg, clip, true, false)?;
                }
            }
            Kind::Table => self.border(b, t.surface, t.border, t.radius, clip),
            Kind::TableRow => {
                if n.index % 2 == 1 {
                    self.rect(b, t.raised.mix(t.surface, 0.55), 0.0, clip);
                }
                self.rect(
                    Rect {
                        x: b.x,
                        y: b.y + b.height - 1.0,
                        width: b.width,
                        height: 1.0,
                    },
                    t.border,
                    0.0,
                    clip,
                );
            }
            Kind::TableHeader => {
                self.rect(b, t.raised, 0.0, clip);
                self.label(
                    &n.content,
                    Rect {
                        x: b.x + 10.0,
                        width: (b.width - 20.0).max(0.0),
                        ..b
                    },
                    n.style.font_size,
                    fg,
                    clip,
                    false,
                    false,
                )?;
                if n.change.is_some() {
                    self.label(
                        "↕",
                        Rect {
                            x: b.x + b.width - 22.0,
                            width: 14.0,
                            ..b
                        },
                        12.0,
                        t.muted,
                        clip,
                        true,
                        false,
                    )?;
                }
                self.rect(
                    Rect {
                        x: b.x,
                        y: b.y + b.height - 1.0,
                        width: b.width,
                        height: 1.0,
                    },
                    t.border,
                    0.0,
                    clip,
                );
            }
            Kind::TableCell => {
                self.label(
                    &n.content,
                    Rect {
                        x: b.x + 10.0,
                        width: (b.width - 20.0).max(0.0),
                        ..b
                    },
                    n.style.font_size,
                    fg,
                    clip,
                    false,
                    false,
                )?;
            }
            Kind::EffectSurface => {
                self.effect_surface(n, b, clip, input, t)?;
            }
            _ => {}
        }
        if n.kind == Kind::Button && n.effect == Some(Effect::Shimmer) && !disabled {
            let alpha = self.alpha;
            self.alpha *= 0.18;
            self.rect(
                Rect {
                    x: b.x + b.width * 0.55,
                    y: b.y + 2.0,
                    width: (b.width * 0.18).min(28.0),
                    height: (b.height - 4.0).max(0.0),
                },
                t.on_accent,
                t.radius,
                clip,
            );
            self.alpha = alpha;
        }
        Ok(())
    }
    fn effect_surface(
        &mut self,
        n: &Node,
        b: Rect,
        clip: Rect,
        input: &Input,
        t: &Theme,
    ) -> Result<(), Error> {
        let effect = n.effect.unwrap_or(Effect::AnimatedList);
        if matches!(
            effect,
            Effect::GridBackground | Effect::AuroraBackground | Effect::Marquee
        ) {
            self.rect(b, t.background, t.radius, clip);
        } else {
            self.rect(Rect { y: b.y + 2.0, ..b }, t.shadow, t.card_radius, clip);
            self.border(b, t.surface, t.border, t.card_radius, clip);
        }
        match effect {
            Effect::NumberTicker => {
                let animated = self
                    .channel(&n.key, "ticker-value")
                    .map(|value| value.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32);
                let text = animated
                    .or(n.number)
                    .map_or_else(|| n.content.clone(), |number| number.to_string());
                self.label(
                    &text,
                    b,
                    n.style.font_size.max(18.0),
                    t.text,
                    clip,
                    true,
                    false,
                )?;
            }
            Effect::TextReveal => {
                let progress = self.channel(&n.key, "text-reveal-progress").unwrap_or(1.0);
                self.label_aligned(
                    revealed_word_prefix(&n.content, progress),
                    b,
                    n.style.font_size,
                    t.text,
                    clip,
                    false,
                    true,
                    false,
                )?
            }
            Effect::SpotlightCard => {
                let center = if input.pointer.0 >= b.x
                    && input.pointer.0 <= b.x + b.width
                    && input.pointer.1 >= b.y
                    && input.pointer.1 <= b.y + b.height
                {
                    (input.pointer.0, input.pointer.1)
                } else {
                    (b.x + b.width * 0.5, b.y + b.height * 0.5)
                };
                let old_alpha = self.alpha;
                for step in (0..8).rev() {
                    let radius = (8 - step) as f32 * 18.0;
                    self.alpha = old_alpha * (0.025 + step as f32 * 0.006);
                    self.rect(
                        Rect {
                            x: center.0 - radius,
                            y: center.1 - radius,
                            width: radius * 2.0,
                            height: radius * 2.0,
                        },
                        t.accent,
                        radius,
                        clip.intersect(b),
                    );
                }
                self.alpha = old_alpha;
            }
            Effect::TiltCard | Effect::CardStack | Effect::Carousel | Effect::Dock => {
                let accent = if input.focused.as_deref() == Some(&n.key)
                    || input.hovered.as_deref() == Some(&n.key)
                {
                    t.accent
                } else {
                    t.border
                };
                self.border(b, t.surface, accent, t.card_radius, clip);
            }
            Effect::BorderBeam => {
                self.rect(
                    Rect {
                        x: b.x + 1.0,
                        y: b.y + 1.0,
                        width: (b.width * 0.28).max(1.0),
                        height: 2.0,
                    },
                    t.accent,
                    1.0,
                    clip,
                );
            }
            Effect::AuroraBackground => {
                self.rect(
                    Rect {
                        x: b.x,
                        y: b.y,
                        width: b.width,
                        height: b.height * 0.45,
                    },
                    t.accent.mix(t.background, 0.65),
                    t.radius,
                    clip,
                );
            }
            Effect::GridBackground => {
                let old_alpha = self.alpha;
                self.alpha *= 0.22;
                for column in 1..8 {
                    let x = b.x + b.width * column as f32 / 8.0;
                    self.rect(
                        Rect {
                            x,
                            y: b.y,
                            width: 1.0,
                            height: b.height,
                        },
                        t.border,
                        0.0,
                        clip.intersect(b),
                    );
                }
                for row in 1..6 {
                    let y = b.y + b.height * row as f32 / 6.0;
                    self.rect(
                        Rect {
                            x: b.x,
                            y,
                            width: b.width,
                            height: 1.0,
                        },
                        t.border,
                        0.0,
                        clip.intersect(b),
                    );
                }
                self.alpha = old_alpha;
            }
            Effect::Shimmer => {
                let old_alpha = self.alpha;
                self.alpha *= 0.18;
                self.rect(
                    Rect {
                        x: b.x + b.width * 0.55,
                        y: b.y + 2.0,
                        width: (b.width * 0.18).min(28.0),
                        height: (b.height - 4.0).max(0.0),
                    },
                    t.accent,
                    t.radius,
                    clip,
                );
                self.alpha = old_alpha;
            }
            Effect::AnimatedList
            | Effect::Marquee
            | Effect::RotatingText
            | Effect::ScrollReveal => {}
        }
        Ok(())
    }
    fn text_area(
        &mut self,
        n: &Node,
        b: Rect,
        clip: Rect,
        input: &Input,
        t: &Theme,
        fg: Color,
    ) -> Result<(), Error> {
        let focused = input.focused.as_deref() == Some(&n.key);
        self.border(
            b,
            if n.read_only { t.raised } else { t.surface },
            if focused { t.accent } else { t.border },
            t.radius,
            clip,
        );
        let inner = Rect {
            x: b.x + 12.0,
            y: b.y + 12.0,
            width: (b.width - 24.0).max(1.0),
            height: (b.height - 24.0).max(1.0),
        };
        let clip = clip.intersect(inner);
        let value = if n.value.is_empty() {
            &n.content
        } else {
            &n.value
        };
        let (glyphs, caret_layout) = self.text.glyphs_and_carets(
            value,
            n.style.font_size * self.dpi,
            inner.width * self.dpi,
        );
        let (row, caret_x) = caret_layout
            .caret(input.caret.min(n.value.chars().count()), input.caret_line)
            .unwrap_or((0, 0.0));
        let line = &caret_layout.lines[row];
        let caret = (caret_x / self.dpi, line.y / self.dpi);
        let line_height = line.height / self.dpi;
        let offset = if focused {
            (caret.1 + line_height - inner.height).max(0.0)
        } else {
            0.0
        };
        if focused && input.select_all && !n.value.is_empty() {
            self.rect(inner, t.accent.mix(t.surface, 0.76), 2.0, clip);
        }
        for g in glyphs {
            if g.width == 0 || g.height == 0 {
                continue;
            }
            let uv = self.atlas.glyph(self.text, self.queue, g.key)?;
            self.quad(
                Rect {
                    x: inner.x + g.x / self.dpi,
                    y: inner.y + g.y / self.dpi - offset,
                    width: g.width as f32 / self.dpi,
                    height: g.height as f32 / self.dpi,
                },
                if n.value.is_empty() { t.muted } else { fg },
                0.0,
                clip,
                uv,
                true,
            );
        }
        if focused && !n.read_only {
            self.rect(
                Rect {
                    x: inner.x + caret.0,
                    y: inner.y + caret.1 - offset,
                    width: 1.0,
                    height: n.style.font_size + 3.0,
                },
                t.accent,
                0.0,
                clip,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::revealed_word_prefix;
    use std::collections::BTreeMap;
    use ubi_ui_core::motion::{MotionState, MotionTarget};
    use ubi_ui_core::{Easing, Kind, Motion, Transition};

    #[test]
    fn text_reveal_exposes_words_and_preserves_internal_whitespace() {
        let text = "A  careful\nchange arrives";
        assert_eq!(revealed_word_prefix(text, 0.0), "");
        assert_eq!(revealed_word_prefix(text, 0.34), "A  careful");
        assert_eq!(revealed_word_prefix(text, 0.67), "A  careful\nchange");
        assert_eq!(revealed_word_prefix(text, 1.0), text);
    }

    #[test]
    fn text_reveal_motion_samples_change_the_painted_word_run() {
        let text = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty";
        let target = MotionTarget {
            key: "story".into(),
            kind: Kind::EffectSurface,
            channels: BTreeMap::from([
                ("opacity".into(), 1.0),
                ("text-reveal-progress".into(), 1.0),
                ("translate-y".into(), 0.0),
            ]),
            motion: Motion {
                duration_ms: 220,
                delay_ms: 0,
                easing: Easing::EaseOut,
            },
            enter: Transition::Rise,
            exit: Transition::None,
            snapshot: None,
        };
        let mut state = MotionState::default();
        state.commit(&[target], 0, false).unwrap();
        let sample = |state: &mut MotionState, now_ms| {
            let frame = state.sample(now_ms, false);
            revealed_word_prefix(text, frame.channels["story"]["text-reveal-progress"] as f32)
                .to_owned()
        };
        let initial = sample(&mut state, 0);
        let middle = sample(&mut state, 110);
        let complete = sample(&mut state, 440);
        assert!(initial.is_empty());
        assert!(
            !middle.is_empty() && middle != text,
            "mid-frame must reveal a bounded word run"
        );
        assert_eq!(complete, text);
    }
}
