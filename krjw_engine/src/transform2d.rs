//! # Transform2D — 2D position / scale / rotation
//!
//! A composable 2D transform: rotation → scale → translation (RST), applied in that order.
//! 可组合的 2D 变换：旋转 → 缩放 → 平移（RST），按此顺序应用。
//!
//! For a child entity: `world = parent * self` means first apply self's local RST,
//! then parent's. This is the convention used by `transform()`.
//! 子实体的变换：`world = parent * self` 表示先应用自身的局部 RST，再应用父级。

use glam::Vec2;

/// A composable 2D transform: position, scale, rotation.
/// 可组合的 2D 变换：位置、缩放、旋转。
///
/// # Coordinate convention / 坐标约定
///
/// - Rotation is counter-clockwise in radians. / 逆时针旋转（弧度）
/// - Scale is applied after rotation. / 缩放在旋转之后应用
/// - Translation is applied last. / 平移最后应用
#[derive(Copy, Clone, Debug)]
pub struct Transform2D {
    /// World-space position. / 世界空间位置。
    pub pos: Vec2,
    /// Scale factors along local X and Y axes. / 沿局部 X/Y 轴的缩放因子。
    pub scale: Vec2,
    /// Rotation in radians (counter-clockwise). / 旋转角度（弧度，逆时针）。
    pub rot: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform2D {
    /// Identity transform: position (0,0), scale (1,1), rotation 0.
    /// 单位变换：位置 (0,0)，缩放 (1,1)，旋转 0。
    pub const IDENTITY: Self = Self {
        pos: Vec2::ZERO,
        scale: Vec2::ONE,
        rot: 0.0,
    };

    /// Builder: set position. / 构建器模式：设置位置。
    pub fn with_pos(mut self, pos: Vec2) -> Self {
        self.pos = pos;
        self
    }

    /// Builder: set scale. / 构建器模式：设置缩放。
    pub fn with_scale(mut self, scale: Vec2) -> Self {
        self.scale = scale;
        self
    }

    /// Builder: set rotation. / 构建器模式：设置旋转。
    pub fn with_rot(mut self, rot: f32) -> Self {
        self.rot = rot;
        self
    }

    /// Builder: translate by `pos`. / 构建器模式：位移。
    pub fn with_move_by(mut self, pos: Vec2) -> Self {
        self.pos += pos;
        self
    }

    /// Builder: translate by rotated `pos`. / 构建器模式：按旋转位移。
    pub fn with_walk_by(mut self, pos: Vec2) -> Self {
        let (sin, cos) = self.rot.sin_cos();
        self.pos += Vec2::new(sin, cos).rotate(pos);
        self
    }

    /// Builder: scale by `scale`. / 构建器模式：缩放。
    pub fn with_scale_by(mut self, scale: Vec2) -> Self {
        self.scale *= scale;
        self
    }

    /// Builder: rotate by `rot` radians. / 构建器模式：旋转（弧度）。
    pub fn with_rotate_by(mut self, rot: f32) -> Self {
        self.rot += rot;
        self
    }

    /// Compose with a parent transform: `result = parent * self`.
    /// 与父级变换组合：`result = parent * self`。
    ///
    /// The child is first rotated, scaled, translated in its own local space,
    /// then placed into the parent's space.
    /// 子级先在其局部空间旋转、缩放、平移，然后放入父级空间。
    ///
    /// Mathematically / 数学表达式:
    ///   `result.pos  = parent.pos + rotate(self.pos, parent.rot) * parent.scale`  
    ///   `result.scale = self.scale * parent.scale`  
    ///   `result.rot   = self.rot + parent.rot`
    pub fn with_transform(&self, parent: &Transform2D) -> Self {
        let (sin, cos) = parent.rot.sin_cos();
        let rotated = Vec2::new(
            self.pos.x * cos - self.pos.y * sin,
            self.pos.x * sin + self.pos.y * cos,
        ) * parent.scale;
        Self {
            pos: parent.pos + rotated,
            scale: self.scale * parent.scale,
            rot: self.rot + parent.rot,
        }
    }

    /// Convenience: compose with raw components. / 便捷方法：与原始组件组合。
    pub fn transform_components(&self, pos: Vec2, scale: Vec2, rot: f32) -> Self {
        self.with_transform(&Self { pos, scale, rot })
    }

    /// Transform a point from this entity's local space to parent space.
    /// 将点从实体的局部空间变换到父级空间。
    ///
    /// `world_point = pos + rotate(local_point * scale, rot)`
    pub fn transform_point(&self, local_point: Vec2) -> Vec2 {
        let (sin, cos) = self.rot.sin_cos();
        let scaled = local_point * self.scale;
        self.pos
            + Vec2::new(
                scaled.x * cos - scaled.y * sin,
                scaled.x * sin + scaled.y * cos,
            )
    }

    /// Inverse: transform a point from parent space back to local space.
    /// 反向变换：将点从父级空间变换回局部空间。
    ///
    /// `local_point = rotate(world_point - pos, -rot) / scale`
    pub fn inverse_transform_point(&self, world_point: Vec2) -> Vec2 {
        let (sin, cos) = (-self.rot).sin_cos();
        let translated = world_point - self.pos;
        Vec2::new(
            (translated.x * cos - translated.y * sin) / self.scale.x,
            (translated.x * sin + translated.y * cos) / self.scale.y,
        )
    }
}
