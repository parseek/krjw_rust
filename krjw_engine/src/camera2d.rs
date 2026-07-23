use glam::{Mat4, Vec2, Vec3, Vec4};

/// A 2D orthographic camera.
///
/// Handles the View-Projection matrix and coordinates conversion between
/// screen space (pixels) and world space.
#[derive(Debug, Clone, Copy)]
pub struct Camera2D {
    /// Camera position in world space.
    pub position: Vec2,
    /// Camera rotation (radians).
    pub rotation: f32,
    /// Camera zoom (Vec2 for non-uniform scaling).
    pub zoom: Vec2,
    /// Top-left corner of the viewport in window pixels.
    pub viewport_pos: Vec2,
    /// Size of the viewport in pixels.
    pub viewport_size: Vec2,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self { position: Vec2::ZERO, rotation: 0.0, zoom: Vec2::ONE, viewport_pos: Vec2::ZERO, viewport_size: Vec2::ZERO }
    }
}

#[allow(unused)]
impl Camera2D {
    pub fn move_by(&mut self, position: Vec2) {
        self.position += position;
    }
    pub fn walk_xy(&mut self, xy: Vec2) {
        let (sin, cos) = self.rotation.sin_cos();
        self.position += Vec2::new(xy.x * cos - xy.y * sin, xy.x * sin + xy.y * cos);
    }
    pub fn walk_xplus(&mut self, xplus: f32) {
        let (sin, cos) = self.rotation.sin_cos();
        self.position += Vec2::new(cos, sin ) * xplus;
    }
    pub fn walk_yplus(&mut self, yplus: f32) {
        let (sin, cos) = self.rotation.sin_cos();
        self.position += Vec2::new(-sin, cos ) * yplus;
    }
}

#[allow(unused)]
impl Camera2D {
    /// Create a camera that covers the entire window.
    pub fn new(window_size_px: Vec2) -> Self {
        Self {
            position: Vec2::ZERO,
            rotation: 0.0,
            zoom: Vec2::ONE,
            viewport_pos: Vec2::ZERO,
            viewport_size: window_size_px,
        }
    }

    /// --- Matrix helpers ---

    /// Full View-Projection matrix: P × V.
    ///
    /// Note: Returns the matrix in a form ready to be used with
    /// `batch.set_mvp(gfx, &vp.transpose())`.
    pub fn vp_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// View matrix: inverse of camera transform.
    pub fn view_matrix(&self) -> Mat4 {
        let t = Mat4::from_translation(Vec3::new(-self.position.x, -self.position.y, 0.0));
        let r = Mat4::from_rotation_z(-self.rotation);
        let s = Mat4::from_scale(Vec3::new(1.0 / self.zoom.x, 1.0 / self.zoom.y, 1.0));
        s * r * t
    }

    /// Orthographic projection matrix (built from viewport size).
    pub fn projection_matrix(&self) -> Mat4 {
        let half_w = self.viewport_size.x * 0.5;
        let half_h = self.viewport_size.y * 0.5;
        Mat4::orthographic_rh(-half_w, half_w, half_h, -half_h, 0.0, 1.0)
    }

    /// --- Viewport state application ---

    /// Set the GPU viewport to match this camera's viewport rect.
    pub fn apply_viewport(&self, gfx: &crate::graphic::d3d11::D3D11) {
        gfx.set_viewport(
            self.viewport_pos.x,
            self.viewport_pos.y,
            self.viewport_size.x,
            self.viewport_size.y,
        );
    }

    /// --- Coordinate conversion ---

    /// Convert a window pixel coordinate to world space.
    ///
    /// Screen Y goes top→bottom (0 at top of window), world Y goes
    /// bottom→top (-half_h at bottom, +half_h at top per orthographic_rh).
    /// This method flips Y accordingly.
    pub fn screen_to_world(&self, screen_px: Vec2) -> Vec2 {
        // 1. Window pixel → viewport-local pixel
        let local_px = screen_px - self.viewport_pos;
        // 2. Viewport-local pixel → NDC [-1, 1], flipping Y (screen Y↓ vs NDC Y↑)
        let ndc = Vec2::new(
            (local_px.x / self.viewport_size.x) * 2.0 - 1.0,
            1.0 - (local_px.y / self.viewport_size.y) * 2.0, // Y flip
        );
        // 3. NDC → world via VP⁻¹
        let vp_inv = self.vp_matrix().inverse();
        let clip = Vec4::new(ndc.x, ndc.y, 0.0, 1.0);
        let world = vp_inv * clip;
        Vec2::new(world.x / world.w, world.y / world.w)
    }

    /// Convert a world coordinate to window pixel coordinate.
    ///
    /// Undoes the Y flip done in screen_to_world.
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        let clip = self.vp_matrix() * Vec4::new(world_pos.x, world_pos.y, 0.0, 1.0);
        let ndc = Vec2::new(clip.x / clip.w, clip.y / clip.w);
        let local_px = Vec2::new(
            (ndc.x + 1.0) * 0.5 * self.viewport_size.x,
            (1.0 - ndc.y) * 0.5 * self.viewport_size.y, // Y flip back
        );
        local_px + self.viewport_pos
    }
}
