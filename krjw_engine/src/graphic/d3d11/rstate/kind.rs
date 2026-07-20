//! `RStateBuilderKind` 枚举 + `RState::into_builder` 的实现。
//!
//! 此模块提供从已有 `RState` 安全提取构建器的能力，强制调用者显式处理两种模式。

use super::bit_layout::RState;
use super::builder::{RStateBuilder, Basic, Advanced};
use super::enums::*;

// ---------- RStateBuilderKind 枚举 ----------
/// 从现有 `RState` 提取构建器时的返回类型，强制 `match` 处理两种模式。
pub enum RStateBuilderKind {
    Basic(RStateBuilder<Basic>),
    Advanced(RStateBuilder<Advanced>),
}

// ---------- 为 RState 实现 into_builder ----------
impl RState {
    /// 将当前的 `RState` 转换为对应的构建器。
    ///
    /// # 示例
    /// ```
    /// let state = RState::default();
    /// match state.into_builder() {
    ///     RStateBuilderKind::Basic(b) => {
    ///         let new_state = b.blend(BlendMode::Additive).build();
    ///         // 使用 new_state
    ///     }
    ///     RStateBuilderKind::Advanced(b) => {
    ///         // Advanced 模式无法修改 Basic 字段，只能原样使用
    ///         let new_state = b.build();
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn into_builder(self) -> RStateBuilderKind {
        if self.is_basic() {
            RStateBuilderKind::Basic(RStateBuilder {
                bits: self.0,
                _state: std::marker::PhantomData,
            })
        } else {
            RStateBuilderKind::Advanced(RStateBuilder {
                bits: self.0,
                _state: std::marker::PhantomData,
            })
        }
    }

    // ---------- 便捷辅助方法 ----------
    /// 如果当前是 Basic 模式，则修改混合模式并返回新状态；
    /// 如果是 Advanced 模式，则原样返回自身（不丢失高级 ID）。
    #[inline]
    pub fn with_blend(self, mode: BlendMode) -> Self {
        match self.into_builder() {
            RStateBuilderKind::Basic(b) => b.blend(mode).build(),
            RStateBuilderKind::Advanced(_) => self,
        }
    }

    /// 如果当前是 Basic 模式，则修改采样器模式。
    #[inline]
    pub fn with_sampler(self, mode: SamplerMode) -> Self {
        match self.into_builder() {
            RStateBuilderKind::Basic(b) => b.sampler(mode).build(),
            RStateBuilderKind::Advanced(_) => self,
        }
    }

    /// 如果当前是 Basic 模式，则修改光栅化模式。
    #[inline]
    pub fn with_raster(self, mode: RasterMode) -> Self {
        match self.into_builder() {
            RStateBuilderKind::Basic(b) => b.raster(mode).build(),
            RStateBuilderKind::Advanced(_) => self,
        }
    }

    /// 如果当前是 Basic 模式，则修改深度测试。
    #[inline]
    pub fn with_depth_test(self, enable: bool) -> Self {
        match self.into_builder() {
            RStateBuilderKind::Basic(b) => b.depth_test(enable).build(),
            RStateBuilderKind::Advanced(_) => self,
        }
    }

    /// 如果当前是 Basic 模式，则修改深度写入。
    #[inline]
    pub fn with_depth_write(self, enable: bool) -> Self {
        match self.into_builder() {
            RStateBuilderKind::Basic(b) => b.depth_write(enable).build(),
            RStateBuilderKind::Advanced(_) => self,
        }
    }

    /// 如果当前是 Basic 模式，则修改模板模式。
    #[inline]
    pub fn with_stencil(self, mode: StencilMode) -> Self {
        match self.into_builder() {
            RStateBuilderKind::Basic(b) => b.stencil(mode).build(),
            RStateBuilderKind::Advanced(_) => self,
        }
    }
}