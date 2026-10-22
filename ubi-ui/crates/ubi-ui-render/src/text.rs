use fontdue::layout::{CoordinateSystem, GlyphPosition, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use ubi_ui_core::Error;
use ubi_ui_input::{TextLayout, VisualLine};
use ubi_ui_layout::TextMeasure;

/// Shared fontdue shaping/line-layout for intrinsic size and GPU glyph placement.
/// Inter covers its bundled scripts; advanced shaping/fallback is not claimed.
pub struct Text {
    pub(crate) font: Font,
    layout: Layout,
    cache: HashMap<(String, u32, u32), (f32, f32)>,
}
impl Text {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            font: Font::from_bytes(
                include_bytes!("../../../assets/Inter.ttf") as &[u8],
                FontSettings::default(),
            )
            .map_err(|e| Error(format!("bundled font: {e}")))?,
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            cache: HashMap::new(),
        })
    }
    pub(crate) fn glyphs(
        &mut self,
        text: &str,
        size: f32,
        max_width: f32,
    ) -> (Vec<GlyphPosition>, f32) {
        self.layout_text(text, size, max_width);
        (self.layout.glyphs().clone(), self.layout.height())
    }
    fn layout_text(&mut self, text: &str, size: f32, max_width: f32) {
        self.layout.reset(&LayoutSettings {
            max_width: if max_width.is_finite() {
                Some(max_width.max(1.0))
            } else {
                None
            },
            line_height: 1.2,
            ..Default::default()
        });
        self.layout
            .append(&[&self.font], &TextStyle::new(text, size, 0));
    }
    /// Pixel-space caret geometry from the same fontdue pass used to paint.
    /// Supply the font size and content width at the render surface's DPI.
    pub fn caret_layout(&mut self, text: &str, size: f32, max_width: f32) -> TextLayout {
        self.layout_text(text, size, max_width);
        self.current_carets(text, size)
    }
    pub(crate) fn glyphs_and_carets(
        &mut self,
        text: &str,
        size: f32,
        max_width: f32,
    ) -> (Vec<GlyphPosition>, TextLayout) {
        self.layout_text(text, size, max_width);
        (
            self.layout.glyphs().clone(),
            self.current_carets(text, size),
        )
    }
    fn current_carets(&self, text: &str, size: f32) -> TextLayout {
        let offsets: Vec<_> = text.char_indices().map(|(byte, _)| byte).collect();
        let glyphs = self.layout.glyphs();
        let mut lines = Vec::new();
        if let Some(metrics) = self.layout.lines() {
            for line in metrics {
                let mut carets = Vec::new();
                for g in glyphs
                    .iter()
                    .take(line.glyph_end + 1)
                    .skip(line.glyph_start)
                {
                    let at = offsets.binary_search(&g.byte_offset).unwrap();
                    let metrics = self.font.metrics(g.parent, size);
                    let x = if g.parent == '\n' {
                        g.x
                    } else {
                        g.x - metrics.bounds.xmin.floor()
                    };
                    // fontdue retains newline glyphs at the end of their row.
                    // The next row starts after the newline, never before it.
                    if carets.is_empty() {
                        carets.push((at, x));
                    }
                    if g.parent != '\n' {
                        carets.push((at + 1, x + metrics.advance_width.ceil()));
                    }
                }
                lines.push(VisualLine {
                    carets,
                    y: line.baseline_y - line.max_ascent,
                    height: line.max_new_line_size * 1.2,
                });
            }
        }
        if lines.is_empty() || text.ends_with('\n') {
            let height = lines.last().map_or_else(
                || {
                    self.font
                        .horizontal_line_metrics(size)
                        .map_or(size * 1.2, |m| m.new_line_size.ceil() * 1.2)
                },
                |line| line.height,
            );
            let y = lines.last().map_or(0.0, |line| line.y + line.height);
            lines.push(VisualLine {
                carets: vec![(offsets.len(), 0.0)],
                y,
                height,
            });
        }
        TextLayout { lines }
    }
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }
}
impl TextMeasure for Text {
    fn measure(&mut self, text: &str, size: f32, max_width: f32) -> (f32, f32) {
        let key = (text.to_owned(), size.to_bits(), max_width.to_bits());
        if let Some(size) = self.cache.get(&key) {
            return *size;
        }
        let (glyphs, height) = self.glyphs(text, size, max_width);
        let width = glyphs
            .iter()
            .map(|g| g.x + g.width as f32)
            .fold(0.0, f32::max)
            .ceil();
        let size = (width, height.max(size * 1.2).ceil());
        if self.cache.len() >= 4096 {
            self.cache.clear();
        }
        self.cache.insert(key, size);
        size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use ubi_ui_core::View;
    use ubi_ui_input::{Input, Key};
    fn area(value: &str) -> View {
        View::decode(json!({"schemaVersion":1,"root":"area","nodes":[{
            "key":"area","kind":{"tag":"TextArea","values":[]},"children":[],
            "props":[{"tag":"Value","values":[value]},{"tag":"FontSize","values":[16]}]
        }]}))
        .unwrap()
    }
    #[test]
    fn textarea_home_uses_actual_soft_wraps() {
        let mut text = Text::new().unwrap();
        let value = "iiii iiiiiiii WWWWW ".repeat(8);
        let view = area(&value);
        let scene = ubi_ui_layout::Layout::default()
            .compute(&view, 69.0, 150.0, &mut text)
            .unwrap();
        let width = scene.boxes["area"].bounds.width - 24.0;
        for dpi in [1.0, 1.25, 2.0] {
            let glyphs = text.glyphs(&value, 16.0 * dpi, width * dpi).0;
            let line = &text.layout.lines().unwrap()[1];
            let start = value[..glyphs[line.glyph_start].byte_offset]
                .chars()
                .count();
            assert!(start > 0);
            let geometry = text.caret_layout(&value, 16.0 * dpi, width * dpi);
            assert_eq!(geometry.lines[1].carets[0].0, start);
            let mut input = Input::default();
            input.key(&view, Key::Tab, true, false);
            input.caret = start + 2;
            input.key_with_text_layout(&view, Key::Home, true, false, Some(&geometry));
            assert_eq!(input.caret, start);
            assert_eq!(input.caret_line, Some(1));
            input.key_with_text_layout(&view, Key::End, true, false, Some(&geometry));
            assert_eq!(input.caret, geometry.lines[1].carets.last().unwrap().0);
            // End retains the preceding row at a shared soft-wrap boundary.
            assert_eq!(geometry.caret(input.caret, input.caret_line).unwrap().0, 1);
            input.key_with_text_layout(&view, Key::Home, true, false, Some(&geometry));
            assert_eq!(input.caret, start);
            input.key_with_text_layout(&view, Key::Up, true, false, Some(&geometry));
            assert_eq!(input.caret, geometry.lines[0].carets[0].0);
            input.key_with_text_layout(&view, Key::Down, true, false, Some(&geometry));
            assert_eq!(input.caret, start);
        }
    }

    #[test]
    fn visual_carets_cover_unicode_empty_and_trailing_hard_lines() {
        let mut text = Text::new().unwrap();
        for (value, starts) in [
            ("", vec![0]),
            ("\n", vec![0, 1]),
            ("\n\n", vec![0, 1, 2]),
            ("a\n", vec![0, 2]),
            ("é🦀\n\n中", vec![0, 3, 4]),
        ] {
            let geometry = text.caret_layout(value, 16.0, 200.0);
            assert_eq!(
                geometry
                    .lines
                    .iter()
                    .map(|line| line.carets[0].0)
                    .collect::<Vec<_>>(),
                starts,
                "{value:?}"
            );
            for at in 0..=value.chars().count() {
                assert!(
                    geometry.caret(at, None).is_some(),
                    "missing scalar {at} in {value:?}"
                );
            }
            let view = area(value);
            let mut input = Input::default();
            input.key(&view, Key::Tab, true, false);
            input.key_with_text_layout(&view, Key::Home, true, false, Some(&geometry));
            assert_eq!(input.caret, *starts.last().unwrap());
            input.key_with_text_layout(&view, Key::End, true, false, Some(&geometry));
            assert_eq!(input.caret, value.chars().count());
        }
    }

    #[test]
    fn vertical_navigation_preserves_pixel_column_across_short_visual_rows() {
        let mut text = Text::new().unwrap();
        let value = "iiii WWWW\ni\niiii WWWW";
        let view = area(value);
        let geometry = text.caret_layout(value, 16.0, 200.0);
        let mut input = Input::default();
        input.key(&view, Key::Tab, true, false);
        input.caret = 6;
        let goal = geometry.caret(input.caret, None).unwrap().1;
        input.key_with_text_layout(&view, Key::Down, true, false, Some(&geometry));
        assert_eq!(input.caret, geometry.lines[1].carets.last().unwrap().0);
        input.key_with_text_layout(&view, Key::Down, true, false, Some(&geometry));
        assert_eq!(input.caret, 18);
        assert_eq!(
            geometry.caret(input.caret, input.caret_line).unwrap().1,
            goal
        );
        input.key_with_text_layout(&view, Key::Up, true, false, Some(&geometry));
        input.key_with_text_layout(&view, Key::Up, true, false, Some(&geometry));
        assert_eq!(input.caret, 6);
    }
}
