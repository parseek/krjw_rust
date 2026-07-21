//! 32 位渲染状态（RState）的位布局定义。
//!
//! 所有字段的偏移量和位数均以 `const` 常量声明，便于扩展。
//! 编译期静态断言确保无重叠且总位数 ≤ 32。

use super::enums::*;

// ============================================================================
// 1. 位布局常量
// ============================================================================

pub const MODE_SHIFT: u32 = 0;
pub const MODE_BITS: u32 = 1;
pub const MODE_MASK: u32 = ((1 << MODE_BITS) - 1) << MODE_SHIFT;

pub const BLEND_SHIFT: u32 = 1;
pub const BLEND_BITS: u32 = 4;
pub const BLEND_MASK: u32 = ((1 << BLEND_BITS) - 1) << BLEND_SHIFT;

pub const SAMPLER_SHIFT: u32 = 5;
pub const SAMPLER_BITS: u32 = 4;
pub const SAMPLER_MASK: u32 = ((1 << SAMPLER_BITS) - 1) << SAMPLER_SHIFT;

pub const RASTER_SHIFT: u32 = 9;
pub const RASTER_BITS: u32 = 4;
pub const RASTER_MASK: u32 = ((1 << RASTER_BITS) - 1) << RASTER_SHIFT;

pub const DEPTH_TEST_SHIFT: u32 = 13;
pub const DEPTH_TEST_BITS: u32 = 1;
pub const DEPTH_TEST_MASK: u32 = ((1 << DEPTH_TEST_BITS) - 1) << DEPTH_TEST_SHIFT;

pub const DEPTH_WRITE_SHIFT: u32 = 14;
pub const DEPTH_WRITE_BITS: u32 = 1;
pub const DEPTH_WRITE_MASK: u32 = ((1 << DEPTH_WRITE_BITS) - 1) << DEPTH_WRITE_SHIFT;

pub const STENCIL_SHIFT: u32 = 15;
pub const STENCIL_BITS: u32 = 2;
pub const STENCIL_MASK: u32 = ((1 << STENCIL_BITS) - 1) << STENCIL_SHIFT;

// ---- 保留位（索引 17..31） ----
// 未来扩展示例：
// pub const ALPHA_TO_COVERAGE_SHIFT: u32 = 17;
// pub const ALPHA_TO_COVERAGE_BITS: u32 = 1;
// pub const ALPHA_TO_COVERAGE_MASK: u32 = ...

// ============================================================================
// 2. 编译期布局检查
// ============================================================================

/// 所有字段的 (shift, bits) 列表，用于自动化检查。
pub const ALL_FIELDS: &[(u32, u32)] = &[
    (MODE_SHIFT, MODE_BITS),
    (BLEND_SHIFT, BLEND_BITS),
    (SAMPLER_SHIFT, SAMPLER_BITS),
    (RASTER_SHIFT, RASTER_BITS),
    (DEPTH_TEST_SHIFT, DEPTH_TEST_BITS),
    (DEPTH_WRITE_SHIFT, DEPTH_WRITE_BITS),
    (STENCIL_SHIFT, STENCIL_BITS),
];

/// 编译期检查：无重叠且总位数 ≤ 32。
#[allow(dead_code)]
const _LAYOUT_CHECK: () = {
    let mut total_bits = 0;
    let mut i = 0;
    while i < ALL_FIELDS.len() {
        total_bits += ALL_FIELDS[i].1;
        i += 1;
    }
    assert!(total_bits <= 32, "Total bits exceed 32");

    let mut i = 0;
    while i < ALL_FIELDS.len() {
        let (shift_i, bits_i) = ALL_FIELDS[i];
        let mask_i = ((1 << bits_i) - 1) << shift_i;
        let mut j = i + 1;
        while j < ALL_FIELDS.len() {
            let (shift_j, bits_j) = ALL_FIELDS[j];
            let mask_j = ((1 << bits_j) - 1) << shift_j;
            let mask_and = mask_i & mask_j;
            assert!(
                mask_and == 0
            );
            j += 1;
        }
        i += 1;
    }
};

// ============================================================================
// 3. RState 结构体及方法
// ============================================================================

/// 32 位紧凑打包的渲染状态。
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RState(pub u32);

impl RState {
    pub const DEFAULT: Self = RState(0);

    #[inline]
    pub const fn new_advanced(id: u32) -> Self {
        RState((id << 1) | 1)
    }

    #[inline]
    pub const fn is_basic(&self) -> bool {
        (self.0 & MODE_MASK) == 0
    }

    #[inline]
    pub const fn is_advanced(&self) -> bool {
        (self.0 & MODE_MASK) != 0
    }

    #[inline]
    pub const fn advanced_id(&self) -> u32 {
        self.0 >> 1
    }

    // ---------- Basic 字段提取 ----------
    #[inline]
    pub const fn blend_idx(&self) -> u8 {
        ((self.0 & BLEND_MASK) >> BLEND_SHIFT) as u8
    }

    #[inline]
    pub const fn sampler_idx(&self) -> u8 {
        ((self.0 & SAMPLER_MASK) >> SAMPLER_SHIFT) as u8
    }

    #[inline]
    pub const fn raster_idx(&self) -> u8 {
        ((self.0 & RASTER_MASK) >> RASTER_SHIFT) as u8
    }

    #[inline]
    pub const fn depth_test(&self) -> bool {
        (self.0 & DEPTH_TEST_MASK) != 0
    }

    #[inline]
    pub const fn depth_write(&self) -> bool {
        (self.0 & DEPTH_WRITE_MASK) != 0
    }

    #[inline]
    pub const fn stencil_idx(&self) -> u8 {
        ((self.0 & STENCIL_MASK) >> STENCIL_SHIFT) as u8
    }

    // ---------- 枚举转换（使用 num_enum 的 TryFrom） ----------
    #[inline]
    pub fn blend_mode(&self) -> Option<BlendMode> {
        if self.is_advanced() { None }
        else { BlendMode::try_from(self.blend_idx()).ok() }
    }

    #[inline]
    pub fn sampler_mode(&self) -> Option<SamplerMode> {
        if self.is_advanced() { None }
        else { SamplerMode::try_from(self.sampler_idx()).ok() }
    }

    #[inline]
    pub fn raster_mode(&self) -> Option<RasterMode> {
        if self.is_advanced() { None }
        else { RasterMode::try_from(self.raster_idx()).ok() }
    }

    #[inline]
    pub fn stencil_mode(&self) -> Option<StencilMode> {
        if self.is_advanced() { None }
        else { StencilMode::try_from(self.stencil_idx()).ok() }
    }
}

impl Default for RState {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}