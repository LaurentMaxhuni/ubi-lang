use fontdue::layout::{CoordinateSystem, GlyphPosition, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;
use ubi_ui_core::Error;
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
        (self.layout.glyphs().clone(), self.layout.height())
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
