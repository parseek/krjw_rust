#[macro_export]
macro_rules! krjw_vecf {
    ($x: expr, $y: expr) => {
        glam::Vec2::new($x as f32, $y as f32)
    };
    ($x: expr, $y: expr, ) => {
        glam::Vec2::new($x as f32, $y as f32)
    };
    ($x: expr, $y: expr, $z: expr) => {
        glam::Vec3::new($x as f32, $y as f32, $z as f32)
    };
    ($x: expr, $y: expr, $z: expr, ) => {
        glam::Vec3::new($x as f32, $y as f32, $z as f32)
    };
    ($x: expr, $y: expr, $z: expr, $w: expr) => {
        glam::Vec4::new($x as f32, $y as f32, $z as f32, $w as f32)
    };
    ($x: expr, $y: expr, $z: expr, $w: expr, ) => {
        glam::Vec4::new($x as f32, $y as f32, $z as f32, $w as f32)
    };
}

#[macro_export]
macro_rules! krjw_vecf_splat {
    ($x: expr, 2) => {
        glam::Vec2::splat($x as f32)
    };
    ($x: expr, 3) => {
        glam::Vec3::splat($x as f32)
    };
    ($x: expr, 4) => {
        glam::Vec4::splat($x as f32)
    };
}

pub use super::krjw_vecf as vecf;
pub use super::krjw_vecf_splat as vecf_splat;