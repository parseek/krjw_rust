//! # app_shapes — Example app: colorful bouncing shapes with physics
//!
//! Showcases krjw_engine's ShapeBatch2D, Camera2D, EventDriver, and Timer.
//! 展示 krjw_engine 的 ShapeBatch2D、Camera2D、EventDriver 和 Timer。

use std::sync::mpsc::Receiver;

use anyhow::{Result, Context};
use glam::Vec2;

use krjw_engine::{
    self, AppMsg, Timer,
    camera2d::Camera2D,
    event_driver::EventDriver,
    graphic::d3d11::D3D11,
    graphic::d3d11::shape_batch_2d::ShapeBatch2D,
    graphic::d3d11::sprite_batch_2d::SpriteBatch2D,
    winit::keyboard::KeyCode,
    winit::window::Window,
};

/// Number of bouncing balls.
const BALL_COUNT: usize = 80;
/// Ball radius.
const BALL_RADIUS: f32 = 8.0;

/// A colorful bouncing ball.
#[derive(Clone)]
pub struct Ball {
    pos: Vec2,
    vel: Vec2,
    color: [f32; 4],
    radius: f32,
}

/// Engine resources created after window initialisation.
#[allow(dead_code)]
pub struct AppContext {
    pub window: Window,
    pub gfx: D3D11,
    pub batch: SpriteBatch2D,
    pub shape_batch: ShapeBatch2D,
    pub camera: Camera2D,
}

/// Application state.
pub struct App {
    pub ctx: Option<AppContext>,
    pub timer: Timer,
    pub balls: Vec<Ball>,
    pub trails: Vec<(Vec2, [f32; 4])>, // fading trail dots
    pub mouse_pos: Vec2,
    pub mouse_down: bool,
    pub attract_mode: bool,
}

impl App {
    pub fn new() -> Self {
        let mut balls = Vec::with_capacity(BALL_COUNT);
        let mut rng = SimpleRng::new(42);
        for i in 0..BALL_COUNT {
            let angle = i as f32 * 137.508_f32.to_radians(); // golden angle
            let radius = 200.0 + rng.next() * 300.0;
            balls.push(Ball {
                pos: Vec2::new(angle.cos() * radius, angle.sin() * radius),
                vel: Vec2::new(rng.next() * 400.0 - 200.0, rng.next() * 400.0 - 200.0),
                color: hsv_to_rgb(i as f32 * 0.618, 0.8, 1.0),
                radius: BALL_RADIUS + rng.next() * 8.0,
            });
        }
        Self {
            ctx: None,
            timer: Timer::default(),
            balls,
            trails: Vec::new(),
            mouse_pos: Vec2::ZERO,
            mouse_down: false,
            attract_mode: true,
        }
    }

    pub fn run(&mut self, window: Window, hwnd: isize, rx: Receiver<AppMsg>) -> Result<()> {
        let gfx = D3D11::init_on_hwnd(hwnd)
            .unwrap_or_else(|e| panic!("gfx::init: {:#}", e));
        let size = window.inner_size();

        let mut driver = EventDriver::new(rx);
        driver.set_initial_window_size(size.width, size.height);

        let batch = SpriteBatch2D::new(
            &gfx.device, 1024,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc,
        )?;
        let shape_batch = ShapeBatch2D::new(
            &gfx.device, 4096,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc,
        )?;
        let camera = Camera2D::new(Vec2::new(size.width as f32, size.height as f32));

        self.ctx = Some(AppContext { window, gfx, batch, shape_batch, camera });

        println!("app_shapes — colorful bouncing shapes demo");
        println!("  [Space] toggle attract/explode mode");
        println!("  [R]     reset balls");
        println!("  [WASD]  move camera");
        println!("  [QE]    rotate camera");
        println!("  Scroll  zoom");

        loop {
            let events = driver.poll_frame();
            if events.close_requested || events.disconnected { break; }

            driver.if_window_size_dirty( |w,h|{
                if let Some(ctx) = self.ctx.as_mut() {
                    ctx.gfx.on_resize(w, h)?;
                    ctx.camera.viewport_size = Vec2::new(w as f32, h as f32);
                }
                Ok(())
            })?;

            let dt = self.timer.pre_frame_and_get_delta_time() as f64;
            let dt32 = dt as f32;
            self.mouse_pos = driver.mouse().get_mouse_pos_vec2();
            self.mouse_down = driver.mouse().get_mouse_button_state(krjw_engine::mouse_input::MouseButton::Left).is_pressed();

            self.handle_input(&driver, dt32);
            self.update_balls(dt);
            self.render_frame(&driver)?;

            if let Some(ctx) = self.ctx.as_mut() {
                ctx.gfx.present().context("present failed")?;
            }
            self.timer.post_frame_fpsc(dt);
            driver.end_frame();
        }
        Ok(())
    }

    fn handle_input(&mut self, driver: &EventDriver, dt: f32) {
        let ctx = self.ctx.as_mut().unwrap();
        let camera = &mut ctx.camera;

        let move_speed = 600.0 * (1.0 / camera.zoom.x.max(0.01));

        // Camera controls
        let k = |code| driver.keyboard().get_key_state(code).is_pressed();
        if k(KeyCode::KeyQ) { camera.rotation -= 2.0 * dt; }
        if k(KeyCode::KeyE) { camera.rotation += 2.0 * dt; }
        if k(KeyCode::KeyA) { camera.walk_xplus(-move_speed * dt); }
        if k(KeyCode::KeyD) { camera.walk_xplus(move_speed * dt); }
        if k(KeyCode::KeyW) { camera.walk_yplus(-move_speed * dt); }
        if k(KeyCode::KeyS) { camera.walk_yplus(move_speed * dt); }

        // Zoom
        if let Some(pixel) = driver.mouse().get_pixel_wheel() {
            if pixel.1 > 0.0 { camera.zoom *= 1.05_f32.powf(dt as f32 * pixel.1.abs() as f32); }
            if pixel.1 < 0.0 { camera.zoom /= 1.05_f32.powf(dt as f32 * pixel.1.abs() as f32); }
        } else {
            let wheel = driver.mouse().get_mouse_wheel_delta();
            if wheel.1 > 0.0 { camera.zoom *= 2.0_f32.powf(dt as f32); }
            if wheel.1 < 0.0 { camera.zoom /= 2.0_f32.powf(dt as f32); }
        }
        camera.zoom = camera.zoom.clamp(Vec2::splat(0.1), Vec2::splat(10.0));

        camera.viewport_pos = Vec2::ZERO;
        let (w, h) = driver.window_size();
        camera.viewport_size = Vec2::new(w as f32, h as f32);
        camera.apply_viewport(&ctx.gfx);

        // Toggle attract / explode
        if driver.keyboard().get_key_state(KeyCode::Space).is_down_true_edge() {
            self.attract_mode = !self.attract_mode;
        }
        // Reset
        if driver.keyboard().get_key_state(KeyCode::KeyR).is_down_true_edge() {
            let mut rng = SimpleRng::new(42);
            for (i, ball) in self.balls.iter_mut().enumerate() {
                let angle = i as f32 * 137.508_f32.to_radians();
                let radius = 200.0 + rng.next() * 300.0;
                ball.pos = Vec2::new(angle.cos() * radius, angle.sin() * radius);
                ball.vel = Vec2::new(rng.next() * 400.0 - 200.0, rng.next() * 400.0 - 200.0);
            }
        }
    }

    fn update_balls(&mut self, dt: f64) {
        let camera = &self.ctx.as_ref().unwrap().camera;
        let dt = dt as f32;

        // Convert mouse to world space
        let world_mouse = camera.screen_to_world(self.mouse_pos);

        // Add trail: every ball leaves a fading dot
        if self.trails.len() > 2000 {
            self.trails.drain(0..100);
        }

        let mut force_center = Vec2::ZERO;
        if self.mouse_down {
            force_center = world_mouse;
        } else if self.attract_mode {
            force_center = Vec2::ZERO;
        }

        for ball in &mut self.balls {
            // Force toward center (or away in explode mode)
            let to_center = force_center - ball.pos;
            let dist = to_center.length().max(1.0);
            if self.mouse_down || self.attract_mode {
                let strength = if self.mouse_down { 800.0 } else { 200.0 };
                ball.vel += to_center / dist * strength * dt;
            } else {
                // Explode: push away from center
                ball.vel += to_center / dist * -300.0 * dt;
            }

            // Drag
            ball.vel *= (1.0 - 0.5 * dt).max(0.0);

            // Speed cap
            let speed = ball.vel.length();
            if speed > 800.0 {
                ball.vel = ball.vel / speed * 800.0;
            }

            ball.pos += ball.vel * dt;

            // Trail
            self.trails.push((ball.pos, ball.color));
        }

        // Fade trail opacity
        for trail in &mut self.trails {
            trail.1[3] *= 0.97;
        }
        self.trails.retain(|t| t.1[3] > 0.01);
    }

    fn render_frame(&mut self, _driver: &EventDriver) -> Result<()> {
        // Extract data from self while we still can
        let trails = self.trails.clone();
        let balls = self.balls.clone();
        let mouse_pos = self.mouse_pos;

        let (vp, world_mouse) = {
            let ctx = self.ctx.as_ref().unwrap();
            unsafe {
                ctx.gfx.imm_context.OMSetBlendState(&ctx.gfx.states.blend_alpha, None, 0xFFFFFFFF);
                ctx.gfx.imm_context.RSSetState(&ctx.gfx.states.rasterizer_solid_cull_none);
                ctx.gfx.imm_context.OMSetDepthStencilState(&ctx.gfx.states.depth_none, 0);
            }
            ctx.gfx.clear_screen(&[0.05, 0.05, 0.08, 1.0]);
            let vp = ctx.camera.vp_matrix().transpose();
            let world_mouse = ctx.camera.screen_to_world(mouse_pos);
            (vp, world_mouse)
        };

        let ctx = self.ctx.as_mut().unwrap();
        let sb = &mut ctx.shape_batch;
        let gfx = &ctx.gfx;
        let camera = &ctx.camera;

        // ── Draw grid ──
        sb.clear_batch();
        draw_grid(sb, camera);
        sb.set_mvp(gfx, &vp);
        sb.submit_and_draw(gfx)?;
        sb.clear_batch();

        // ── Draw trails ──
        for (pos, color) in &trails {
            sb.add_circle_no_uv(*pos, 1.5, *color, 8);
        }

        // ── Draw balls ──
        for ball in &balls {
            sb.add_circle_no_uv(ball.pos, ball.radius * 2.5, [ball.color[0], ball.color[1], ball.color[2], 0.15], 20);
            sb.add_circle_no_uv(ball.pos, ball.radius, ball.color, 20);
            sb.add_circle_no_uv(ball.pos, ball.radius + 1.0, [1.0, 1.0, 1.0, 0.3], 20);
        }

        sb.set_mvp(gfx, &vp);
        sb.submit_and_draw(gfx)?;
        sb.clear_batch();

        // ── Draw crosshair at mouse ──
        let cross_size = 15.0 * camera.zoom.x.max(camera.zoom.y);
        sb.add_square_line_no_uv(
            world_mouse + Vec2::new(-cross_size, 0.0),
            world_mouse + Vec2::new(cross_size, 0.0),
            2.0, [1.0, 1.0, 1.0, 0.5],
        );
        sb.add_square_line_no_uv(
            world_mouse + Vec2::new(0.0, -cross_size),
            world_mouse + Vec2::new(0.0, cross_size),
            2.0, [1.0, 1.0, 1.0, 0.5],
        );
        sb.set_mvp(gfx, &vp);
        sb.submit_and_draw(gfx)?;
        sb.clear_batch();

        Ok(())
    }
}

/// Draw a perspective grid.
fn draw_grid(sb: &mut ShapeBatch2D, camera: &Camera2D) {
    // let spacing = 100.0 * camera.zoom.x.max(camera.zoom.y).max(0.3).min(3.0);
    let spacing = 100.0;
    let hw = camera.viewport_size.x * 0.5 * camera.zoom.x;
    let hh = camera.viewport_size.y * 0.5 * camera.zoom.y;
    let half_side = (hw * hw + hh * hh).sqrt().max(spacing);
    let cx = camera.position.x;
    let cy = camera.position.y;

    let min_x = ((cx - half_side) / spacing).floor() * spacing;
    let max_x = ((cx + half_side) / spacing).ceil() * spacing;
    let min_y = ((cy - half_side) / spacing).floor() * spacing;
    let max_y = ((cy + half_side) / spacing).ceil() * spacing;

    let mut count = 0usize;
    let mut x = min_x;
    while x <= max_x && count < 200 {
        sb.add_square_line_no_uv(Vec2::new(x, min_y), Vec2::new(x, max_y), 1.0, [0.2, 0.2, 0.3, 0.5]);
        x += spacing;
        count += 1;
    }
    let mut y = min_y;
    while y <= max_y && count < 400 {
        sb.add_square_line_no_uv(Vec2::new(min_x, y), Vec2::new(max_x, y), 1.0, [0.2, 0.2, 0.3, 0.5]);
        y += spacing;
        count += 1;
    }
}

/// Convert HSV to RGBA.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 4] {
    let hi = (h * 6.0).floor() as i32 % 6;
    let f = h * 6.0 - (hi as f32);
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b, 1.0]
}

/// Minimal deterministic RNG for reproducible results.
struct SimpleRng(u64);

impl SimpleRng {
    fn new(seed: u64) -> Self { Self(seed) }
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
}