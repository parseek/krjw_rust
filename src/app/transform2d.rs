use glam::Vec2;

/// A 2D transform: rotation → scale → translate (RST), applied in that order.
///
/// For a child entity: `world = parent * self` means first apply self's
/// local RST, then parent's. This is the convention used by `transform()`.
#[derive(Copy, Clone)]
pub struct Transform2D {
    pub pos: Vec2,
    pub scale: Vec2,
    pub rot: f32,
}

impl Transform2D {
    pub const IDENTITY: Self = Self {
        pos: Vec2::ZERO,
        scale: Vec2::ONE,
        rot: 0.0,
    };

    /// Compose with a parent transform: `result = parent * self`.
    ///
    /// The child is first rotated, scaled, translated in its own local space,
    /// then placed into the parent's space.
    ///
    /// Mathematically:
    ///   `result.pos  = parent.pos + rotate(self.pos, parent.rot) * parent.scale`
    ///   `result.scale = self.scale * parent.scale`
    ///   `result.rot   = self.rot + parent.rot`
    pub fn transform(&self, parent: &Transform2D) -> Self {
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

    /// Convenience: compose with raw components.
    pub fn transform_components(&self, pos: Vec2, scale: Vec2, rot: f32) -> Self {
        self.transform(&Self { pos, scale, rot })
    }

    /// Transform a point from this entity's local space to parent space.
    ///
    /// `world_point = pos + rotate(local_point * scale, rot)`
    pub fn transform_point(&self, local_point: Vec2) -> Vec2 {
        let (sin, cos) = self.rot.sin_cos();
        let scaled = local_point * self.scale;
        self.pos + Vec2::new(
            scaled.x * cos - scaled.y * sin,
            scaled.x * sin + scaled.y * cos,
        )
    }

    /// Inverse: transform a point from parent space back to local space.
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