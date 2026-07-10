use std::collections::HashMap;
#[allow(unused_imports)]
use std::f64::consts::*;
use std::io::Cursor;

use anyhow::{Context, Result};
use glam::Vec2;
use winit::keyboard::KeyCode;
use winit::window::WindowAttributes;

use kira::{
    AudioManager, DefaultBackend,
    sound::static_sound::StaticSoundData,
};

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

pub struct AppContext {
    pub window: winit::window::Window,
    pub gfx: D3D11,
    pub audio_mgr: AudioManager,
    pub batch: SpriteBatch2D,
    pub batch_tex_srv: TextureInfo,
    pub shape_batch: ShapeBatch2D,
    pub tiles: Vec<Tile>,
    pub camera: Camera2D,
    pub hovered_tile: Option<usize>,
    pub grid_spacing: f32,
}

pub struct Tile {
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
pub struct App {
    pub window_pos: (i32, i32),
    pub window_size: (u32, u32),

    pub keyboard_input: keyboard_input::KeyboardInput,
    pub mouse_input: mouse_input::MouseInput,

    pub sounds: HashMap<String, StaticSoundData>,

    pub timer: timer::Timer,
    pub ctx: Option<AppContext>,
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

impl App {
    pub fn on_init(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) -> Result<()> {
        // ── Window & GPU ───────────────────────────────────────
        let window = event_loop
            .create_window(WindowAttributes::default().with_title("KrisuRJW"))
            .unwrap_or_else(|e| panic!("window::create: {:#}", e));
        let gfx = D3D11::init_on_window(&window)
            .unwrap_or_else(|e| panic!("gfx::init: {:#}", e));

        // ── Audio ──────────────────────────────────────────────
        let audio_mgr =
            AudioManager::<DefaultBackend>::new(Default::default())
                .context("AudioManager::new failed")?;

        macro_rules! insert_snd {
            ($name:expr, $dir:expr) => {
                self.sounds.insert($name.to_string(),
                    StaticSoundData::from_cursor(Cursor::new(include_bytes!($dir)))?);
            };
        }
        insert_snd!("snd_ominous_cancel", "../snd_ominous_cancel.wav");
        insert_snd!("snd_ominous", "../snd_ominous.wav");

        // ── Texture ────────────────────────────────────────────
        let img = image::load_from_memory(include_bytes!("../seth.png"))
            .context("failed to load seth.png")?;
        let tex_info = load_texture_from_dynamic_image(&gfx.device, &img)?;
        println!("Seth.png: {:?}", tex_info);
        let tw = tex_info.width as f32;
        let th = tex_info.height as f32;
        let cell_w = tw / 4.0;
        let cell_h = th / 6.0;

        // ── Batches ────────────────────────────────────────────
        let batch = SpriteBatch2D::new(
            &gfx.device, 2048,
            &gfx.states.vs_puc_m_2d,
            &gfx.states.ps_tex_rgba_2d,
            &gfx.states.input_layout_puc,
        )?;
        let shape_batch = ShapeBatch2D::new(
            &gfx.device, 4096,
            &gfx.states.vs_puc_m_2d,
            &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc,
        )?;

        // ── Tiles ──────────────────────────────────────────────
        let mut tiles = Vec::new();
        for i in 0..24 {
            let cx = (i % 4) as f32 * cell_w;
            let cy = (i / 4) as f32 * cell_h;
            let angle = i as f32 * 1.3;
            let col = (i % 4) as f32;
            let row = (i / 4) as f32;

            tiles.push(Tile {
                pos: Vec2::new((col - 1.5) * 150.0, (row - 2.5) * 150.0),
                vel: Vec2::new(
                    (i as f32 * 0.7).cos() * 200.0,
                    (i as f32 * 1.1).sin() * 200.0,
                ),
                rot: 0.0,
                rot_vel: ((i as f32 * 0.5).cos() * 2.0).abs(),
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

        // ── Camera ─────────────────────────────────────────────
        let ws = Vec2::new(self.window_size.0 as f32, self.window_size.1 as f32);
        let camera = Camera2D::new(ws);

        println!("赛博吸尘器 with Seth.png");
        println!("    ---- 🔪Aqua's idea");
        println!("操作方式：");
        println!("  - AD WS 移动相机");
        println!("  - Q / E 旋转相机");
        println!("  - 鼠标滚轮缩放相机");
        println!("  - 鼠标左键吸引图块");
        println!("  - X 键强力制动");

        self.ctx = Some(AppContext {
            window,
            gfx,
            audio_mgr,
            batch,
            batch_tex_srv: tex_info,
            shape_batch,
            tiles,
            camera,
            hovered_tile: None,
            grid_spacing: GRID_SPACING,
        });
        Ok(())
    }

    pub fn on_frame(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();
        let gfx = &ctx.gfx;
        let camera = &mut ctx.camera;

        let w = self.window_size.0 as f32;
        let h = self.window_size.1 as f32;
        let dt = self.timer.pre_frame_and_get_delta_time() as f32;
        let dt = if dt > 0.1 { 0.1 } else { dt };

        // ── Camera viewport ────────────────────────────────────
        camera.viewport_pos = Vec2::splat(0.0f32);
        camera.viewport_size = Vec2::new(w, h);

        let move_speed = 500.0;
        let rot_speed = 2.0;
        let zoom_speed: f32 = 25.0;

        if key_pressed!(self, KeyCode::KeyQ) { camera.rotation -= rot_speed * dt; }
        if key_pressed!(self, KeyCode::KeyE) { camera.rotation += rot_speed * dt; }

        if self.mouse_input.get_mouse_wheel_delta().1 > 0.0 {
            camera.zoom *= zoom_speed.powf(dt);
        }
        if self.mouse_input.get_mouse_wheel_delta().1 < 0.0 {
            camera.zoom /= zoom_speed.powf(dt);
        }

        let (sin_rot, cos_rot) = camera.rotation.sin_cos();
        let mut move_dir = Vec2::ZERO;
        if key_pressed!(self, KeyCode::KeyD) { move_dir += Vec2::new(cos_rot, sin_rot); }
        if key_pressed!(self, KeyCode::KeyA) { move_dir -= Vec2::new(cos_rot, sin_rot); }
        if key_pressed!(self, KeyCode::KeyW) { move_dir -= Vec2::new(-sin_rot, cos_rot); }
        if key_pressed!(self, KeyCode::KeyS) { move_dir += Vec2::new(-sin_rot, cos_rot); }
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

        let audio_mgr = &mut ctx.audio_mgr;

        if key_state!(self, KeyCode::KeyX).is_down_true_edge() {
            if let Some(snd) = self.sounds.get("snd_ominous_cancel") {
                audio_mgr.play(snd.clone().volume(0.0)).unwrap();
            }
        }
        if self
            .mouse_input
            .get_mouse_button_state(MouseButton::Left)
            .is_down_edge()
        {
            if let Some(snd) = self.sounds.get("snd_ominous") {
                audio_mgr.play(snd.clone().volume(0.0)).unwrap();
            }
        }

        // ── Tile physics + hover ───────────────────────────────
        ctx.hovered_tile = None;
        for (idx, tile) in ctx.tiles.iter_mut().enumerate().rev() {
            tile.pos += tile.vel * dt;
            tile.rot += tile.rot_vel * dt;

            if lmb_pressed {
                let d = world_mouse - tile.pos;
                let len = d.length();
                let a = len.sqrt();
                let a = d / len * a;
                tile.vel += a;
            }

            let f: f32 = if key_pressed!(self, KeyCode::KeyX) { 10.0 } else { 0.1 };
            let len_sqr = tile.vel.length_squared();
            if len_sqr > 25.0 {
                tile.vel += -tile.vel * f / len_sqr.sqrt();
            }

            let inst = ColliderInstance {
                shape: &tile.collider,
                xform: Transform2D {
                    pos: tile.pos,
                    scale: Vec2::splat(tile.scale),
                    rot: tile.rot,
                },
            };
            if inst.contains_point(world_mouse) && ctx.hovered_tile.is_none() {
                ctx.hovered_tile = Some(idx);
            }
        }

        ctx.window.set_title(
            format!("KrisuRJW - FPS: {:.2} dTime: {:.05}", self.timer.get_fps(), dt).as_str(),
        );

        // ── Render ─────────────────────────────────────────────
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

            // Grid
            let sb = &mut ctx.shape_batch;
            sb.clear_batch();
            build_grid(sb, camera, ctx.grid_spacing, GRID_COLOR);
            sb.set_mvp(gfx, &vp_transposed);
            sb.submit_and_draw(gfx).context("grid submit_and_draw failed")?;
            sb.clear_batch();

            // Tiles
            let batch = &mut ctx.batch;
            batch.clear_batch();
            batch.set_texture(
                ctx.batch_tex_srv.srv.clone(),
                ctx.batch_tex_srv.width,
                ctx.batch_tex_srv.height,
            );
            for tile in &ctx.tiles {
                batch
                    .add(
                        tile.pos + Vec2::new(5.0, 5.0),
                        Vec2::splat(tile.scale), tile.rot,
                        &tile.sprite_rect, [0.0, 0.0, 0.0, 0.2],
                    )
                    .unwrap_or_else(|_| ());
                batch
                    .add(tile.pos, Vec2::splat(tile.scale), tile.rot,
                         &tile.sprite_rect, tile.color,
                    )
                    .unwrap_or_else(|_| ());
            }
            batch.set_mvp(gfx, &vp_transposed);
            batch.submit_and_draw(gfx).context("batch.submit_and_draw failed")?;
            batch.clear_batch();

            // Collider outlines + cursor circle
            let sb = &mut ctx.shape_batch;
            sb.clear_batch();
            for (idx, tile) in ctx.tiles.iter().enumerate() {
                let inst = ColliderInstance {
                    shape: &tile.collider,
                    xform: Transform2D {
                        pos: tile.pos,
                        scale: Vec2::splat(tile.scale),
                        rot: tile.rot,
                    },
                };
                let color = if Some(idx) == ctx.hovered_tile {
                    [1.0, 0.8, 0.0, 0.8]
                } else {
                    [0.0, 1.0, 0.0, 0.3]
                };
                draw_collider_outline(sb, &inst, color);
            }
            if lmb_pressed {
                sb.add_circle_no_uv(world_mouse, 30.0, [1.0, 1.0, 1.0, 0.3], 24);
            }
            sb.set_mvp(gfx, &vp_transposed);
            sb.submit_and_draw(gfx).context("shape_batch submit_and_draw failed")?;
            sb.clear_batch();
        }

        gfx.present().context("gfx::present failed")?;
        self.timer.post_frame_fpsc();
        Ok(())
    }
}


fn build_grid(sb: &mut ShapeBatch2D, camera: &Camera2D, spacing: f32, color: [f32; 4]) {
    let hw = camera.viewport_size.x * 0.5 * camera.zoom.x;
    let hh = camera.viewport_size.y * 0.5 * camera.zoom.y;
    let half_side = (hw * hw + hh * hh).sqrt();
    let cx = camera.position.x;
    let cy = camera.position.y;

    let min_x = ((cx - half_side) / spacing).floor() * spacing;
    let max_x = ((cx + half_side) / spacing).ceil() * spacing;
    let min_y = ((cy - half_side) / spacing).floor() * spacing;
    let max_y = ((cy + half_side) / spacing).ceil() * spacing;

    let max_lines = 500;
    if ((max_x - min_x) / spacing) as usize > max_lines
        || ((max_y - min_y) / spacing) as usize > max_lines
    {
        return;
    }

    let shadow = Vec2::new(5.0, 5.0);

    let mut x = min_x;
    while x <= max_x {
        for (off, col) in [(&shadow, [0.0, 0.0, 0.0, 0.2]), (&Vec2::ZERO, color)] {
            sb.add_square_line_no_uv(
                Vec2::new(x, min_y) + *off,
                Vec2::new(x, max_y) + *off,
                10.0, col,
            );
        }
        x += spacing;
    }

    let mut y = min_y;
    while y <= max_y {
        for (off, col) in [(&shadow, [0.0, 0.0, 0.0, 0.2]), (&Vec2::ZERO, color)] {
            sb.add_square_line_no_uv(
                Vec2::new(min_x, y) + *off,
                Vec2::new(max_x, y) + *off,
                10.0, col,
            );
        }
        y += spacing;
    }
}

fn draw_collider_outline(sb: &mut ShapeBatch2D, inst: &ColliderInstance, color: [f32; 4]) {
    match inst.shape {
        Collider::Rect { half_size } | Collider::AABB { half_size } => {
            let h = if matches!(inst.shape, Collider::AABB { .. }) {
                *half_size * inst.xform.scale
            } else {
                *half_size
            };
            let local = [
                Vec2::new(-h.x, -h.y),
                Vec2::new(h.x, -h.y),
                Vec2::new(h.x, h.y),
                Vec2::new(-h.x, h.y),
            ];
            let mut world = [Vec2::ZERO; 4];
            for (i, lc) in local.iter().enumerate() {
                world[i] = inst.xform.transform_point(*lc);
            }
            for i in 0..4 {
                sb.add_square_line_no_uv(world[i], world[(i + 1) % 4], 3.0, color);
            }
        }
        Collider::Circle { radius } => {
            let r = radius * inst.xform.scale.x.max(inst.xform.scale.y);
            sb.add_circle_no_uv(inst.xform.pos, r, color, 24);
        }
    }
}