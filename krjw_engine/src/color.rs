use kira::info;

use crate::krjw_vecf;

fn default_alpha() -> f32 { 1.0 }

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ColorRGBA {
    r: f32,
    g: f32,
    b: f32,
    #[serde(default="default_alpha")]
    a: f32,
}

impl ColorRGBA {
    #[inline]
    pub fn into_tuple(self) -> (f32, f32, f32, f32) {
        (self.r, self.g, self.b, self.a)
    }
    #[inline]
    pub fn from_tuple(val: &(f32, f32, f32, f32)) -> Self {
        Self { r: val.0, g: val.1, b: val.2, a: val.3 }
    }

    #[inline]
    pub fn into_list(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
    #[inline]
    pub fn from_list(val: &[f32]) -> ColorRGBA {
        ColorRGBA { r: val[0], g: val[1], b: val[2], a: val.get(3).copied().unwrap_or(1.0)}
    }
}

use glam::{Vec3, Vec4};

impl From<Vec4> for ColorRGBA {
    fn from(v: Vec4) -> Self {
        Self {
            r: v.x,
            g: v.y,
            b: v.z,
            a: v.w,
        }
    }
}

impl From<ColorRGBA> for Vec4 {
    fn from(c: ColorRGBA) -> Self {
        Self::new(c.r, c.g, c.b, c.a)
    }
}

impl From<Vec3> for ColorRGBA {
    fn from(v: Vec3) -> Self {
        Self {
            r: v.x,
            g: v.y,
            b: v.z,
            a: 1.0, // 默认不透明
        }
    }
}

impl From<ColorRGBA> for Vec3 {
    fn from(c: ColorRGBA) -> Self {
        Self::new(c.r, c.g, c.b) // 丢弃 alpha
    }
}

impl ColorRGBA {
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
}

impl Default for ColorRGBA {
    fn default() -> Self {
        Self::BLACK
    }
}

impl ColorRGBA {
    const INV_U8MAX: f32 = 1. / 255.;

    #[inline]
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        let rgba = krjw_vecf!(r, g, b, a);
        let rgba = rgba * Self::INV_U8MAX;
        rgba.into()
    }
    #[inline]
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }
}