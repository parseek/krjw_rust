//! # krjw_engine — Direct3D 11 2D sprite engine
//!
//! A reusable 2D sprite engine built on Direct3D 11, `winit`, `glam`, and `kira`.
//! 基于 Direct3D 11 的可复用 2D 精灵引擎。

pub mod engine_handler;
pub mod event_driver;
pub mod graphic;
pub mod key_state;
pub mod keyboard_input;
pub mod mouse_input;
pub mod msg;

#[allow(unused)]
pub mod sprite2d;
#[allow(unused)]
pub mod transform2d;
#[allow(unused)]
pub mod camera2d;
#[allow(unused)]
pub mod collider;
#[allow(unused)]
pub mod atlas_text;
#[allow(unused)]
pub mod timer;

// ── Re-exports ─────────────────────────────────────────────

pub use engine_handler::EngineHandler;
pub use event_driver::{EventDriver, FrameEvents};
pub use key_state::{KeyState, KEY_STATE_DOWN_EDGE, KEY_STATE_DOWN_TRUE_EDGE, KEY_STATE_PRESSING, KEY_STATE_RELEASED, KEY_STATE_UP_EDGE, KEY_STATE_UP_TRUE_EDGE};
pub use keyboard_input::KeyboardInput;
pub use mouse_input::{MouseInput, MouseButton};
pub use msg::AppMsg;
pub use sprite2d::{Sprite2D, Sprite2DBuffer, Sprite2DObject, HaveID};
pub use transform2d::Transform2D;
pub use camera2d::Camera2D;
pub use collider::{Collider, ColliderInstance};
pub use atlas_text::AtlasText;
pub use timer::Timer;

pub use graphic::d3d11::D3D11;
pub use graphic::d3d11::d3d11_utils::TextureInfo;
pub use graphic::d3d11::sprite_batch_2d::{SpriteBatch2D, self};
pub use graphic::d3d11::shape_batch_2d::ShapeBatch2D;

// ── TextureInfoArced (originally from app.rs) ──────────────

use std::sync::Arc;

/// An `Arc<TextureInfo>` wrapper that implements `HaveID` using the pointer address.
/// `Arc<TextureInfo>` 的包装器，用指针地址实现 `HaveID`。
///
/// This allows `Sprite2DBuffer` to detect when the active texture changes.
/// 这使得 `Sprite2DBuffer` 能够检测当前纹理的切换。
#[derive(Debug, Clone)]
pub struct TextureInfoArced(pub Arc<TextureInfo>);

impl HaveID for TextureInfoArced {
    fn get_id(&self) -> u64 {
        self.0.as_ref() as *const _ as u64
    }
}

impl sprite_batch_2d::Pipeline for TextureInfoArced {
    fn apply_to_batch(&self, batch: &mut SpriteBatch2D) {
        batch.set_texture(self.0.srv.clone(), self.0.width, self.0.height);
    }
}