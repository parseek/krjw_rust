#[allow(unused_imports)]
use std::f64::consts::*;

use anyhow::{Context, Result};
use glam::Vec2;
use winit::keyboard::KeyCode;

use graphic::d3d11::D3D11;
use graphic::d3d11::d3d11_utils::*;
use graphic::d3d11::shape_batch_2d::ShapeBatch2D;
use graphic::d3d11::sprite_batch_2d::{Sprite, SpriteBatch2D};

#[allow(unused)]
mod camera2d;
#[allow(unused)]
mod collider;
#[allow(unused)]
mod key_state;
#[allow(unused)]
mod keyboard_input;
#[allow(unused)]
mod mouse_input;
#[allow(unused)]
mod transform2d;

use camera2d::Camera2D;
use collider::{Collider, ColliderInstance};
use mouse_input::MouseButton;
use transform2d::Transform2D;

mod graphic;
mod handler;
mod timer;

const GRID_SPACING: f32 = 100.0;
const GRID_COLOR: [f32; 4] = [0.15, 0.15, 0.15, 1.0];

#[derive(Default)]
pub struct App {
    window: Option<winit::window::Window>,
    window_pos: (i32, i32),
    window_size: (u32, u32),

    keyboard_input: keyboard_input::KeyboardInput,
    mouse_input: mouse_input::MouseInput,

    gfx: Option<D3D11>,

    timer: timer::Timer,
    state: Option<State>,
}

struct Tile {
    pos: Vec2,
    vel: Vec2,
    rot: f32,
    rot_vel: f32,
    scale: f32,
    sprite_rect: Sprite,
    collider: Collider,
    color: [f32; 4],
}

#[derive(Default)]
struct State {
    // SpriteBatch2D
    batch: Option<SpriteBatch2D>,
    tiles: Vec<Tile>,
    batch_tex_srv: Option<TextureInfo>,

    // ShapeBatch2D (grid + cursor circle + collider outlines)
    shape_batch: Option<ShapeBatch2D>,

    // Camera
    camera: Option<Camera2D>,

    // Hover state
    hovered_tile: Option<usize>,

}

impl State {
    fn new(gfx: &D3D11, window_size: Vec2) -> Result<Self> {
        // Load texture from seth.png
        let img = image::load_from_memory(include_bytes!("../seth.png"))
            .context("failed to load seth.png")?;

        // ── SpriteBatch2D test ──────────────────────────────────
        let tex_info = load_texture_from_dynamic_image(&gfx.device, &img)?;
        println!("Seth.png: {:?}", tex_info);
        let tw = tex_info.width as f32;
        let th = tex_info.height as f32;

        let batch = SpriteBatch2D::new(&gfx.device, 2048)?;

        let mut tiles = Vec::new();
        let num_tiles = 24;
        let cols = 4;
        let cell_w = tw / cols as f32;
        let cell_h = th / (num_tiles / cols) as f32;

        for i in 0..num_tiles {
            let cx = (i % cols) as f32 * cell_w;
            let cy = (i / cols) as f32 * cell_h;
            let angle = i as f32 * 1.3;

            let col = (i % cols) as f32;
            let row = (i / cols) as f32;

            tiles.push(Tile {
                pos: Vec2::new((col - 1.5) * 150.0, (row - 2.5) * 150.0),
                vel: Vec2::new(
                    (i as f32 * 0.7).cos() * 200.0,
                    (i as f32 * 1.1).sin() * 200.0,
                ),
                rot: 0.0,
                rot_vel: (i as f32 * 0.5).cos() * 2.0,
                scale: 0.2 + (i % 3) as f32 * 0.08,
                sprite_rect: Sprite {
                    origin_px: Vec2::new(cell_w / 2.0, cell_h / 2.0),
                    size_px: Vec2::new(cell_w, cell_h),
                    uv_tl_px: Vec2::new(cx, cy),
                    uv_size_px: Vec2::new(cell_w, cell_h),
                },
                collider: Collider::Rect {
                    half_size: Vec2::new(cell_w, cell_h) * 0.5,
                },
                color: [
                    0.5 + (angle).sin() * 0.5,
                    0.5 + (angle + 2.0).sin() * 0.5,
                    0.5 + (angle + 4.0).sin() * 0.5,
                    1.0,
                ],
            });
        }

        // ShapeBatch2D used for grid, cursor circle, and collider outlines.
        let shape_batch = ShapeBatch2D::new(&gfx.device, 4096)?;

        Ok(Self {
            batch: Some(batch),
            tiles,
            batch_tex_srv: Some(tex_info),
            shape_batch: Some(shape_batch),
            camera: Some(Camera2D::new(window_size)),
            hovered_tile: None,
        })
    }
}

#[allow(unused)]
macro_rules! key_pressed {
    ($self:expr, $key:expr) => {
        $self.keyboard_input.get_key_state($key).is_pressed()
    };
}

#[allow(unused)]
macro_rules! key_state {
    ($self:expr, $key:expr) => {
        $self.keyboard_input.get_key_state($key)
    };
}

/// Build grid lines within the culling square and store them in `sb`.
fn build_grid(sb: &mut ShapeBatch2D, camera: &Camera2D, grid_spacing: f32, grid_color: [f32; 4]) {
    let hw = camera.viewport_size.x * 0.5 * camera.zoom.x;
    let hh = camera.viewport_size.y * 0.5 * camera.zoom.y;
    let half_side = (hw * hw + hh * hh).sqrt();

    let cx = camera.position.x;
    let cy = camera.position.y;

    let min_x = ((cx - half_side) / grid_spacing).floor() * grid_spacing;
    let max_x = ((cx + half_side) / grid_spacing).ceil() * grid_spacing;
    let min_y = ((cy - half_side) / grid_spacing).floor() * grid_spacing;
    let max_y = ((cy + half_side) / grid_spacing).ceil() * grid_spacing;

    // Clamp to a sane max to avoid blowing up the batch
    let max_lines = 500;
    let num_x = ((max_x - min_x) / grid_spacing) as usize;
    let num_y = ((max_y - min_y) / grid_spacing) as usize;
    if num_x > max_lines || num_y > max_lines {
        return;
    }

    // Vertical lines
    let mut x = min_x;
    let shadow_offset = Vec2 { x: 5.0, y: 5.0};
    while x <= max_x {
        let from = Vec2::new(x, min_y);
        let to = Vec2::new(x, max_y);
        sb.add_square_line(
            from + shadow_offset,
            to + shadow_offset,
            10.0,
            [0.0, 0.0, 0.0, 0.2],
        );
        sb.add_square_line(
            from,
            to,
            10.0,
            grid_color,
        );
        x += grid_spacing;
    }

    // Horizontal lines
    let mut y = min_y;
    while y <= max_y {
        let from  = Vec2::new(min_x, y);
        let to = Vec2::new(max_x, y);
        sb.add_square_line(
            from + shadow_offset,
            to + shadow_offset,
            10.0,
            [0.0, 0.0, 0.0, 0.2],
        );
        sb.add_square_line(
            from,
            to,
            10.0,
            grid_color,
        );
        y += grid_spacing;
    }
}

/// Draw a collider outline using the ShapeBatch2D.
fn draw_collider_outline(sb: &mut ShapeBatch2D, inst: &ColliderInstance, color: [f32; 4]) {
    match inst.shape {
        Collider::Rect { half_size } | Collider::AABB { half_size } => {
            let h = if matches!(inst.shape, Collider::AABB { .. }) {
                *half_size * inst.xform.scale
            } else {
                *half_size
            };
            // Four corners in local space
            let local = [
                Vec2::new(-h.x, -h.y),
                Vec2::new(h.x, -h.y),
                Vec2::new(h.x, h.y),
                Vec2::new(-h.x, h.y),
            ];
            // Transform to world space
            let mut world = [Vec2::ZERO; 4];
            for (i, lc) in local.iter().enumerate() {
                world[i] = inst.xform.transform_point(*lc);
            }
            // Four line segments
            for i in 0..4 {
                let from = world[i];
                let to = world[(i + 1) % 4];
                sb.add_square_line(from, to, 3.0, color);
            }
        }
        Collider::Circle { radius } => {
            let r = radius * inst.xform.scale.x.max(inst.xform.scale.y);
            sb.add_circle(inst.xform.pos, r, color, 24);
        }
    }
}

impl App {
    fn on_init(&mut self) -> Result<()> {
        let gfx = self.gfx.as_ref().context("App not initialized")?;
        let ws = Vec2::new(self.window_size.0 as f32, self.window_size.1 as f32);
        self.state = Some(State::new(gfx, ws).context("State::new failed")?);
        println!("赛博吸尘器 with Seth.png");
        println!("    ---- 🔪Aqua's idea");
        println!("操作方式：");
        println!("  - AD WS 移动相机");
        println!("  - Q / E 旋转相机");
        println!("  - 鼠标滚轮缩放相机");
        println!("  - 鼠标左键吸引图块");
        println!("  - X 键强力制动");
        Ok(())
    }
    fn on_frame(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) -> Result<()> {
        let window = self.window.as_ref().unwrap();
        let gfx = self.gfx.as_ref().unwrap();
        let state = self.state.as_mut().unwrap();

        let w = self.window_size.0 as f32;
        let h = self.window_size.1 as f32;
        let dt = self.timer.pre_frame_and_get_delta_time() as f32;
        let dt = if dt > 0.1 { 0.1 } else { dt };

        // ── Camera controls ───────────────────────────────────
        let camera = state.camera.as_mut().unwrap();
        camera.viewport_pos = Vec2::splat(0.0f32);
        camera.viewport_size = Vec2::new(w, h);

        let move_speed = 500.0;
        let rot_speed = 2.0;
        let zoom_speed: f32 = 25.0;

        // Rotation
        if key_pressed!(self, KeyCode::KeyQ) {
            camera.rotation -= rot_speed * dt;
        }
        if key_pressed!(self, KeyCode::KeyE) {
            camera.rotation += rot_speed * dt;
        }

        // Zoom (exponential, frame-rate independent)
        if self.mouse_input.get_mouse_wheel_delta().1 > 0.0 {
            camera.zoom *= zoom_speed.powf(dt);
        }
        if self.mouse_input.get_mouse_wheel_delta().1 < 0.0 {
            camera.zoom /= zoom_speed.powf(dt);
        }

        // Movement (relative to camera rotation)
        let (sin_rot, cos_rot) = camera.rotation.sin_cos();
        let mut move_dir = Vec2::ZERO;
        if key_pressed!(self, KeyCode::KeyD) {
            move_dir += Vec2::new(cos_rot, sin_rot);
        }
        if key_pressed!(self, KeyCode::KeyA) {
            move_dir -= Vec2::new(cos_rot, sin_rot);
        }
        if key_pressed!(self, KeyCode::KeyW) {
            move_dir -= Vec2::new(-sin_rot, cos_rot);
        }
        if key_pressed!(self, KeyCode::KeyS) {
            move_dir += Vec2::new(-sin_rot, cos_rot);
        }
        if move_dir.length_squared() > 0.0 {
            camera.position += move_dir.normalize() * move_speed * dt;
        }

        camera.apply_viewport(gfx);

        // Mouse
        let mouse_screen = self.mouse_input.get_mouse_pos_vec2();
        let world_mouse = camera.screen_to_world(mouse_screen);
        let lmb_pressed = self
            .mouse_input
            .get_mouse_button_state(MouseButton::Left)
            .is_pressed();

        // ── Tile physics + hover detection ─────────────────────
        state.hovered_tile = None;
        for (idx, tile) in state.tiles.iter_mut().enumerate().rev() {
            tile.pos += tile.vel * dt;
            tile.rot += tile.rot_vel * dt;

            if lmb_pressed {
                let distance_to_cursor = world_mouse - tile.pos;
                let x = distance_to_cursor.length();
                let a = x.sqrt();
                let a = distance_to_cursor / x * a;
                tile.vel += a;
            }

            let f: f32 = if key_pressed!(self, KeyCode::KeyX) {
                10.0
            } else {
                0.1
            };
            let len_sqr = tile.vel.length_squared();
            if len_sqr > 25.0 {
                let a = -tile.vel * f / len_sqr.sqrt();
                tile.vel += a;
            }

            // Hover detection
            let tile_inst = ColliderInstance {
                shape: &tile.collider,
                xform: Transform2D {
                    pos: tile.pos,
                    scale: Vec2::splat(tile.scale),
                    rot: tile.rot,
                },
            };
            if tile_inst.contains_point(world_mouse) && state.hovered_tile.is_none() {
                state.hovered_tile = Some(idx);
            }
        }

        window.set_title(
            format!(
                "KrisuRJW - FPS: {:.2} dTime: {:.05}",
                self.timer.get_fps(),
                dt
            )
            .as_str(),
        );

        // -------------------------
        // Render Stage
        // -------------------------

        if self.window_size.0 > 0 && self.window_size.1 > 0 {
            unsafe {
                gfx.imm_context
                    .OMSetBlendState(&gfx.states.blend_alpha, None, 0xFFFFFFFF);
                gfx.imm_context
                    .RSSetState(&gfx.states.rasterizer_solid_cull_none);
                gfx.imm_context
                    .OMSetDepthStencilState(&gfx.states.depth_none, 0);
                gfx.imm_context
                    .PSSetSamplers(0, Some(&[Some(gfx.states.sampler_linear_clamp.clone())]));
            }

            gfx.clear_screen(&[0.8, 0.8, 0.8, 1.0]);

            let vp_transposed = camera.vp_matrix().transpose();

            // ── Grid background ──────────────────────────────────
            if let Some(sb) = state.shape_batch.as_mut() {
                sb.clear_batch();
                build_grid(sb, camera, GRID_SPACING, GRID_COLOR);
                sb.set_mvp(gfx, &vp_transposed);
                sb.submit_and_draw(gfx)
                    .context("grid submit_and_draw failed")?;
                sb.clear_batch();
            }

            // ── Tiles (SpriteBatch2D) ────────────────────────────
            if let Some(batch) = state.batch.as_mut() {
                if let Some(tex) = state.batch_tex_srv.as_ref() {
                    batch.clear_batch();
                    batch.set_texture(tex.srv.clone(), tex.width, tex.height);

                    for tile in &state.tiles {
                        batch
                            .add(
                                tile.pos + Vec2 { x: 5.0, y: 5.0 },
                                Vec2::splat(tile.scale),
                                tile.rot,
                                &tile.sprite_rect,
                                [0.0, 0.0, 0.0, 0.2],
                            )
                            .unwrap_or_else(|_| ());
                        batch
                            .add(
                                tile.pos,
                                Vec2::splat(tile.scale),
                                tile.rot,
                                &tile.sprite_rect,
                                tile.color,
                            )
                            .unwrap_or_else(|_| ());
                    }

                    batch.set_mvp(gfx, &vp_transposed);
                    batch
                        .submit_and_draw(gfx)
                        .context("batch.submit_and_draw failed")?;
                    batch.clear_batch();
                }
            }

            // ── Collider outlines + cursor circle ─────────────────
            if let Some(sb) = state.shape_batch.as_mut() {
                sb.clear_batch();

                // Draw all tile colliders
                for (idx, tile) in state.tiles.iter().enumerate() {
                    let inst = ColliderInstance {
                        shape: &tile.collider,
                        xform: Transform2D {
                            pos: tile.pos,
                            scale: Vec2::splat(tile.scale),
                            rot: tile.rot,
                        },
                    };
                    let color = if Some(idx) == state.hovered_tile {
                        [1.0, 0.8, 0.0, 0.8] // yellow highlight
                    } else {
                        [0.0, 1.0, 0.0, 0.3] // green dim
                    };
                    draw_collider_outline(sb, &inst, color);
                }

                // Cursor circle
                if lmb_pressed {
                    sb.add_circle(world_mouse, 30.0, [1.0, 1.0, 1.0, 0.3], 24);
                }

                sb.set_mvp(gfx, &vp_transposed);
                sb.submit_and_draw(gfx)
                    .context("shape_batch submit_and_draw failed")?;
                sb.clear_batch();
            }
        }

        gfx.present().context("gfx::present failed")?;
        self.timer.post_frame_fpsc();
        Ok(())
    }
}