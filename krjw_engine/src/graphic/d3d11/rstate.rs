//! 渲染状态（RState）模块：32 位紧凑打包，支持 Basic/Advanced 双模式。
//!
//! Basic 模式使用位域直接编码（零开销），覆盖 90%+ 的 2D 精灵需求。
//! Advanced 模式使用剩余 31 位作为索引，支持自定义 Shader/额外纹理等复杂状态。

mod enums;
mod bit_layout;
mod builder;
mod kind;

pub use enums::*;
pub use bit_layout::RState;
pub use builder::RStateBuilder;
pub use kind::RStateBuilderKind;