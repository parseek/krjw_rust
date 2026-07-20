//! 构成 Basic 模式子状态的枚举定义
//! 使用 `num_enum` 提供安全的整数转换

use num_enum::{IntoPrimitive, TryFromPrimitive};

/// 混合模式（4 位，共 16 种）
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
pub enum BlendMode {
    Normal = 0,          // SrcAlpha, InvSrcAlpha (默认)
    Additive = 1,        // One, One
    Multiply = 2,        // Zero, SrcColor (正片叠底)
    Premultiplied = 3,   // One, InvSrcAlpha (预乘阿尔法)
    Subtract = 4,        // SrcAlpha, InvSrcAlpha, BlendOp = SUBTRACT (减法)
    ReverseSubtract = 5, // SrcAlpha, InvSrcAlpha, BlendOp = REV_SUBTRACT
    Min = 6,             // One, One, BlendOp = MIN
    Max = 7,             // One, One, BlendOp = MAX
    Opaque = 8,          // One, Zero (完全不透明)
    Invert = 9,          // InvDstColor, Zero (颜色反相)
    // 10-15 预留自定义
}

/// 采样器模式（4 位，共 16 种）
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
pub enum SamplerMode {
    PointClamp = 0,   // 点采样 + Clamp (默认)
    PointWrap = 1,    // 点采样 + Wrap
    LinearClamp = 2,  // 线性采样 + Clamp
    LinearWrap = 3,   // 线性采样 + Wrap
    AnisoClamp = 4,   // 各向异性 + Clamp
    AnisoWrap = 5,    // 各向异性 + Wrap
    // 6-15 预留
}

/// 光栅化模式（4 位，共 16 种）
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
pub enum RasterMode {
    CullNone = 0,   // 不剔除（双面渲染，2D 默认）
    CullCW = 1,     // 剔除正面（顺时针）
    CullCCW = 2,    // 剔除背面（逆时针）
    Wireframe = 3,  // 线框模式（调试用）
    // 4-15 预留
}

/// 模板模式（2 位，共 4 种）
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
pub enum StencilMode {
    Disabled = 0,  // 完全禁用模板
    Write = 1,     // 写入模式：将当前像素的模板值强制写为 1（画遮罩）
    Read = 2,      // 读取模式：只画模板值为 1 的像素（应用遮罩）
    Invert = 3,    // 反相模式：将当前像素位置的模板值按位取反（XOR 交叠特效）
}