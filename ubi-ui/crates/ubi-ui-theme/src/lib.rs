//! Central semantic design tokens, shared by every native component.
/// Linear shader input is obtained from these sRGB channel values at rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub f32, pub f32, pub f32, pub f32);
impl Color {
    pub const fn rgb(hex: u32) -> Self {
        Self(
            ((hex >> 16) & 255) as f32 / 255.0,
            ((hex >> 8) & 255) as f32 / 255.0,
            (hex & 255) as f32 / 255.0,
            1.0,
        )
    }
    pub fn array(self) -> [f32; 4] {
        [self.0, self.1, self.2, self.3]
    }
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        Self(
            self.0 + (other.0 - self.0) * t,
            self.1 + (other.1 - self.1) * t,
            self.2 + (other.2 - self.2) * t,
            self.3 + (other.3 - self.3) * t,
        )
    }
}
/// Clone and override these public semantic tokens to define a custom theme.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub background: Color,
    pub surface: Color,
    pub raised: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
    pub control_height: f32,
    pub radius: f32,
    pub card_radius: f32,
    pub focus_width: f32,
    pub spacing: [f32; 7],
    pub font_size: f32,
    pub shadow: Color,
}
impl Theme {
    pub fn light() -> Self {
        Self {
            dark: false,
            background: Color::rgb(0xf4f5f7),
            surface: Color::rgb(0xffffff),
            raised: Color::rgb(0xeef0f4),
            text: Color::rgb(0x192334),
            muted: Color::rgb(0x58657a),
            border: Color::rgb(0xcdd3df),
            accent: Color::rgb(0x3158d4),
            on_accent: Color::rgb(0xffffff),
            danger: Color::rgb(0xb63142),
            success: Color::rgb(0x23734d),
            warning: Color::rgb(0x92621e),
            control_height: 40.0,
            radius: 8.0,
            card_radius: 12.0,
            focus_width: 2.0,
            spacing: [0.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0],
            font_size: 14.0,
            shadow: Color(0.05, 0.08, 0.15, 0.06),
        }
    }
    pub fn dark() -> Self {
        Self {
            dark: true,
            background: Color::rgb(0x10141c),
            surface: Color::rgb(0x191f2b),
            raised: Color::rgb(0x252e3d),
            text: Color::rgb(0xe9edf5),
            muted: Color::rgb(0xa6b2c7),
            border: Color::rgb(0x3b465a),
            accent: Color::rgb(0x7897ff),
            on_accent: Color::rgb(0x101b3c),
            danger: Color::rgb(0xff8b9b),
            success: Color::rgb(0x77c9a0),
            warning: Color::rgb(0xe3bd7f),
            ..Self::light()
        }
    }
    /// Resolve a subtree override; System inherits the selected OS theme.
    pub fn inherit(&self, mode: Option<&str>, system_dark: bool) -> Self {
        match mode {
            Some("Light") => Self::light(),
            Some("Dark") => Self::dark(),
            Some("System") if system_dark => Self::dark(),
            Some("System") => Self::light(),
            _ => self.clone(),
        }
    }
    pub fn tone(&self, tone: &str) -> Color {
        match tone {
            "Primary" => self.accent,
            "Success" => self.success,
            "Warning" => self.warning,
            "Danger" => self.danger,
            _ => self.muted,
        }
    }
}
impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subtree_inherits_custom_tokens_and_switches_modes() {
        let mut theme = Theme::light();
        theme.radius = 6.0;
        assert_eq!(theme.inherit(None, false).radius, 6.0);
        assert!(theme.inherit(Some("Dark"), false).dark);
        assert!(theme.inherit(Some("System"), true).dark);
        assert_eq!(Color::rgb(0xffffff).mix(Color::rgb(0), 0.5).0, 0.5);
    }
}
