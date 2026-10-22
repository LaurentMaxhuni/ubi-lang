use super::{Frame, Text, Vertex};
use fontdue::layout::GlyphRasterConfig;
use std::collections::HashMap;
use ubi_ui_core::{Error, Kind, Node, Rect, View};
use ubi_ui_input::Input;
use ubi_ui_layout::{Scene, TextMeasure};
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
    pub fn reset(&mut self, queue:&wgpu::Queue) {
        self.glyphs.clear(); self.x=1; self.y=1; self.row=0;
        queue.write_texture(self.texture.as_image_copy(), &vec![0;(ATLAS*ATLAS) as usize], wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(ATLAS),rows_per_image:Some(ATLAS)}, wgpu::Extent3d{width:ATLAS,height:ATLAS,depth_or_array_layers:1});
    }
    pub fn nearly_full(&self)->bool { self.y+self.row+256>=ATLAS }
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
    };
    let mut themes = HashMap::new();
    themes.insert(
        view.root.clone(),
        theme.inherit(view.nodes[&view.root].theme.as_deref(), system_dark),
    );
    let clear = themes[&view.root].background;
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
        let b = &scene.boxes[key];
        if b.bounds.intersect(b.clip).width <= 0.0 || b.bounds.intersect(b.clip).height <= 0.0 {
            continue;
        }
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
}
impl Painter<'_> {
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
            self.vertices.push(Vertex {
                position: [
                    world[0] / self.viewport.0 * 2.0 - 1.0,
                    1.0 - world[1] / self.viewport.1 * 2.0,
                ],
                world,
                local: [x * b.width, y * b.height],
                size: [b.width, b.height],
                uv: [uv[0] + x * (uv[2] - uv[0]), uv[1] + y * (uv[3] - uv[1])],
                color: color.array(),
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
        let y = b.y + ((b.height - height / self.dpi) * 0.5).max(0.0);
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
            Kind::Button | Kind::IconButton => {
                let (fill, ink) = match n.variant.as_str() {
                    "Primary" => (t.accent, t.on_accent),
                    "Destructive" => (t.danger, t.on_accent),
                    "Ghost" => (if hovered { t.raised } else { t.surface }, fg),
                    _ => (t.surface, fg),
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
            }
            Kind::Text | Kind::Heading => {
                self.label(&n.content, b, n.style.font_size, fg, clip, false, true)?
            }
            Kind::Badge => {
                let tone = t.tone(&n.tone);
                self.rect(b, t.surface.mix(tone, 0.12), b.height * 0.5, clip);
                self.label(&n.content, b, 12.0, tone, clip, true, false)?;
            }
            Kind::Separator => self.rect(b, t.border, 0.0, clip),
            Kind::Checkbox | Kind::Switch => {
                let control = Rect {
                    x: b.x,
                    y: b.y + (b.height - 22.0) * 0.5,
                    width: if n.kind == Kind::Switch { 38.0 } else { 22.0 },
                    height: 22.0,
                };
                let fill = if n.checked {
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
                    if n.checked { fill } else { t.border },
                    if n.kind == Kind::Switch { 11.0 } else { 5.0 },
                    clip,
                );
                if n.kind == Kind::Switch {
                    self.rect(
                        Rect {
                            x: control.x + if n.checked { 19.0 } else { 3.0 },
                            y: control.y + 3.0,
                            width: 16.0,
                            height: 16.0,
                        },
                        if n.checked { t.on_accent } else { t.muted },
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
            Kind::TextInput | Kind::PasswordInput => {
                self.border(
                    b,
                    t.surface,
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
                if focused {
                    let prefix: String = value.chars().take(input.caret).collect();
                    let (w, _) = self.text.measure(&prefix, n.style.font_size, f32::INFINITY);
                    self.rect(
                        Rect {
                            x: inner.x + w,
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
            Kind::ProgressBar => {
                self.rect(b, t.raised, b.height * 0.5, clip);
                self.rect(
                    Rect {
                        width: b.width * n.progress as f32 / 100.0,
                        ..b
                    },
                    t.accent,
                    b.height * 0.5,
                    clip,
                );
            }
            _ => {}
        }
        Ok(())
    }
}
