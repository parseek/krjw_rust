//! 类型状态（Typestate）构建器，编译时保证 Basic/Advanced 模式互斥。

use core::marker::PhantomData;
use super::bit_layout::RState;
use super::enums::*;

// ---------- 内部标记类型 ----------
pub mod sealed {
    pub trait BuilderState {}
    pub struct Basic;
    pub struct Advanced;
    impl BuilderState for Basic {}
    impl BuilderState for Advanced {}
}
pub use sealed::{BuilderState, Basic, Advanced};

// ---------- 构建器核心结构 ----------
/// 状态构建器，泛型参数 `S` 标记当前所处的模式。
///
/// - `S = Basic`：拥有全部设置器，可转换为 Advanced。
/// - `S = Advanced`：仅包含终端操作，无 Basic 设置器。
#[derive(Copy, Clone, Debug)]
pub struct RStateBuilder<S: BuilderState> {
    pub(crate) bits: u32,
    pub(super) _state: PhantomData<S>,
}

// ---------- Basic 模式：拥有全部设置器 ----------
impl RStateBuilder<Basic> {
    #[inline]
    pub fn new() -> Self {
        Self {
            bits: 0,
            _state: PhantomData,
        }
    }

    #[inline]
    pub fn blend(mut self, mode: BlendMode) -> Self {
        self.bits = (self.bits & !(0xF << 1)) | ((mode as u32) << 1);
        self
    }

    #[inline]
    pub fn sampler(mut self, mode: SamplerMode) -> Self {
        self.bits = (self.bits & !(0xF << 5)) | ((mode as u32) << 5);
        self
    }

    #[inline]
    pub fn raster(mut self, mode: RasterMode) -> Self {
        self.bits = (self.bits & !(0xF << 9)) | ((mode as u32) << 9);
        self
    }

    #[inline]
    pub fn depth_test(mut self, enable: bool) -> Self {
        if enable {
            self.bits |= 1 << 13;
        } else {
            self.bits &= !(1 << 13);
        }
        self
    }

    #[inline]
    pub fn depth_write(mut self, enable: bool) -> Self {
        if enable {
            self.bits |= 1 << 14;
        } else {
            self.bits &= !(1 << 14);
        }
        self
    }

    #[inline]
    pub fn stencil(mut self, mode: StencilMode) -> Self {
        self.bits = (self.bits & !(0x3 << 15)) | ((mode as u32) << 15);
        self
    }

    /// 切换到 Advanced 模式，返回不同类型的构建器。
    ///
    /// 此操作会**清除**之前设置的所有 Basic 字段，因为 Advanced 模式
    /// 使用剩余的 31 位存放高级 ID。
    #[inline]
    pub fn advanced(self, id: u32) -> RStateBuilder<Advanced> {
        RStateBuilder {
            bits: (id << 1) | 1,
            _state: PhantomData,
        }
    }

    /// 完成构建，产生 Basic 模式的 `RState`。
    #[inline]
    pub fn build(self) -> RState {
        RState(self.bits)
    }
}

impl Default for RStateBuilder<Basic> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// ---------- Advanced 模式：仅终端操作，无设置器 ----------
impl RStateBuilder<Advanced> {
    /// 完成构建，产生 Advanced 模式的 `RState`。
    #[inline]
    pub fn build(self) -> RState {
        RState(self.bits)
    }

    /// 修改 Advanced ID（保留 Advanced 模式）。
    #[inline]
    pub fn set_id(mut self, new_id: u32) -> Self {
        self.bits = (new_id << 1) | 1;
        self
    }
}