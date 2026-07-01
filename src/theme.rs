use tiny_skia::Color;

#[derive(Clone, Copy)]
pub struct Base16 {
    pub base00: Color,
    pub base01: Color,
    pub base02: Color,
    pub base03: Color,
    pub base04: Color,
    pub base05: Color,
    pub base06: Color,
    pub base07: Color,
    pub base08: Color,
    pub base09: Color,
    pub base0a: Color,
    pub base0b: Color,
    pub base0c: Color,
    pub base0d: Color,
    pub base0e: Color,
    pub base0f: Color,
}

#[derive(Clone)]
pub struct Theme {
    pub raw: Base16,

    pub surface: Color,
    pub surface_raised: Color,
    pub surface_sunken: Color,

    pub text: Color,
    pub text_muted: Color,
    pub text_strong: Color,

    pub border: Color,
    pub border_focus: Color,
    pub primary: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
}

impl Theme {
    // TODO: dark and light theming
    pub fn from_base16(b: Base16) -> Self {
        Self {
            raw: b,

            surface: b.base00,
            surface_raised: b.base01,
            surface_sunken: b.base02,

            text: b.base05,
            text_muted: b.base04,
            text_strong: b.base07,

            border: b.base02,
            border_focus: b.base0d,

            primary: b.base0d,
            success: b.base0b,
            warning: b.base0a,
            danger: b.base08,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor {
    value: u32,
    is_from_rgba: bool,
}

impl HexColor {
    /// Accepts hex as 0xRRGGBBAA
    pub fn from_rgb(value: u32) -> Self {
        Self {
            value,
            is_from_rgba: false,
        }
    }

    pub fn from_rgba(value: u32) -> Self {
        Self {
            value,
            is_from_rgba: true,
        }
    }

    pub fn red(&self) -> u8 {
        if self.is_from_rgba {
            ((self.value >> 24) & 0xFF) as u8
        } else {
            (self.value >> 16) as u8
        }
    }

    pub fn green(&self) -> u8 {
        if self.is_from_rgba {
            ((self.value >> 16) & 0xFF) as u8
        } else {
            (self.value >> 8) as u8
        }
    }

    pub fn blue(&self) -> u8 {
        if self.is_from_rgba {
            ((self.value >> 8) & 0xFF) as u8
        } else {
            (self.value >> 8) as u8
        }
    }

    pub fn alpha(&self) -> u8 {
        if self.is_from_rgba {
            (self.value & 0xFF) as u8
        } else {
            255
        }
    }

    pub fn rgba(&self) -> (u8, u8, u8, u8) {
        (self.red(), self.green(), self.blue(), self.alpha())
    }
}

impl From<HexColor> for Color {
    fn from(c: HexColor) -> Self {
        // println!(
        //     "Hex {} to rgba r={} g={} b={} a={}",
        //     c.value,
        //     c.red(),
        //     c.green(),
        //     c.blue(),
        //     c.alpha()
        // );
        //
        Self::from_rgba8(c.red(), c.green(), c.blue(), c.alpha())
    }
}

pub fn hex(val: u32) -> Color {
    HexColor::from_rgb(val).into()
}

pub fn hex_alpha(val: u32) -> Color {
    HexColor::from_rgba(val).into()
}
