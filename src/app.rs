//! # Application layer — `App` & `AppContext`
//!
//! Ties together windowing, GPU, audio, input, physics, and rendering.
//! 整合窗口管理、GPU、音频、输入、物理和渲染。
//!
//! ## Lifecycle / 生命周期
//!
//! 1. `App::default()` — create with default state / 创建默认状态
//! 2. `App::on_init()` — create window, init GPU + audio + textures / 创建窗口，初始化 GPU + 音频 + 纹理
//! 3. `App::on_frame()` — input → physics → render → present / 输入 → 物理 → 渲染 → 提交
//!
//! ## Key types / 关键类型
//!
//! - [`TextureInfoArced`] — thread-safe texture reference implementing `HaveID` / 实现 `HaveID` 的线程安全纹理引用
//! - [`Tile`] — a bouncing sprite tile with physics / 带物理的弹跳精灵图块
//! - [`AppContext`] — all non-default-constructible resources / 所有不可默认构造的资源

use std::collections::HashMap;
#[allow(unused_imports)]
use std::f64::consts::*;
use std::io::Cursor;
use std::sync::Arc;

use anyhow::{Context, Result};
use glam::Vec2;
use winit::dpi::LogicalSize;
use winit::dpi::Size::Logical;
use winit::keyboard::KeyCode;
use winit::window::WindowAttributes;

use kira::{AudioManager, DefaultBackend, sound::static_sound::StaticSoundData};

use graphic::d3d11::D3D11;
use graphic::d3d11::d3d11_utils::*;
use graphic::d3d11::shape_batch_2d::ShapeBatch2D;
use graphic::d3d11::sprite_batch_2d::SpriteBatch2D;

#[allow(unused)]
mod atlas_text;
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
mod sprite2d;
#[allow(unused)]
mod transform2d;

use camera2d::Camera2D;
use collider::{Collider, ColliderInstance};
use mouse_input::MouseButton;
use sprite2d::{Sprite2D, Sprite2DBuffer, Sprite2DObject};
use transform2d::Transform2D;
mod graphic;
mod handler;
mod timer;

/// Spacing between grid lines in world units. / 网格线间距（世界单位）。
const GRID_SPACING: f32 = 100.0;
/// Grid line colour (RGBA). / 网格线颜色（RGBA）。
const GRID_COLOR: [f32; 4] = [0.15, 0.15, 0.15, 1.0];

/// An `Arc<TextureInfo>` wrapper that implements `HaveID` using the pointer address.
/// `Arc<TextureInfo>` 的包装器，用指针地址实现 `HaveID`。
///
/// This allows `Sprite2DBuffer` to detect when the active texture changes.
/// 这使得 `Sprite2DBuffer` 能够检测当前纹理的切换。
#[derive(Debug, Clone)]
pub struct TextureInfoArced(pub Arc<TextureInfo>);

impl sprite2d::HaveID for TextureInfoArced {
    /// Returns the memory address of the inner `TextureInfo` as a unique ID.
    /// 用内部 `TextureInfo` 的内存地址作为唯一 ID。
    fn get_id(&self) -> u64 {
        self.0.as_ref() as *const _ as u64
    }
}

/// Runtime application context — created after window initialisation.
/// 运行时应用上下文——窗口初始化后创建。
///
/// Contains all GPU/audio/texture resources and game state.
/// 包含所有 GPU/音频/纹理资源和游戏状态。
pub struct AppContext {
    /// The winit window. / winit 窗口。
    pub window: winit::window::Window,
    /// Direct3D 11 device & context wrapper. / Direct3D 11 设备和上下文封装。
    pub gfx: D3D11,
    /// Kira audio manager. / Kira 音频管理器。
    pub audio_mgr: AudioManager,
    /// Primary sprite batch for rendering. / 主精灵渲染批处理。
    pub batch: SpriteBatch2D,
    /// Loaded textures keyed by name. / 按名称索引的已加载纹理。
    pub textures: HashMap<String, Arc<TextureInfo>>,
    /// 2D shape batch for lines and circles. / 2D 形状批处理（线条和圆形）。
    pub shape_batch: ShapeBatch2D,
    /// Bouncing tiles with physics. / 带物理的弹跳图块。
    pub tiles: Vec<Tile>,
    /// 2D camera (position, zoom, rotation). / 2D 相机（位置、缩放、旋转）。
    pub camera: Camera2D,
    /// Index of the tile under the cursor, if any. / 光标下方的图块索引。
    pub hovered_tile: Option<usize>,
    /// Grid spacing override (from const by default). / 网格间距（默认使用常量）。
    pub grid_spacing: f32,
    /// Font name for HUD text, from RJW_FONTNAME env or "SimHei". / HUD 字体名称。
    pub font_name: String,
    /// Dynamic text atlas for HUD text rendering. / 用于 HUD 文字渲染的动态文字图集。
    pub atlas_text: atlas_text::AtlasText,
    /// Sprite buffer for text glyphs from the atlas. / 来自图集的文字字形精灵缓冲区。
    pub text_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    /// Pipeline-sorted sprite buffer for batch rendering.
    /// 用于批渲染的流水线排序精灵缓冲区。
    pub sprite_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
}

/// A single bouncing sprite tile.
/// 单个弹跳精灵图块。
///
/// Each tile has position, velocity, rotation, angular velocity, scale,
/// a sprite rectangle, a collider shape, and a colour.
/// 每个图块包含位置、速度、旋转、角速度、缩放、精灵矩形、碰撞体形状和颜色。
pub struct Tile {
    /// World-space position. / 世界空间位置。
    pos: Vec2,
    /// Velocity in world-units/sec. / 速度（世界单位/秒）。
    vel: Vec2,
    /// Current rotation in radians. / 当前旋转（弧度）。
    rot: f32,
    /// Angular velocity in rad/sec. / 角速度（弧度/秒）。
    rot_vel: f32,
    /// Scale factor. / 缩放因子。
    scale: f32,
    /// Sprite geometry (UV rect). / 精灵几何（UV 矩形）。
    sprite_rect: Sprite2D,
    /// Collider shape for hit-testing. / 用于碰撞检测的碰撞体形状。
    collider: Collider,
    /// RGBA colour. / RGBA 颜色。
    color: [f32; 4],
}

/// Top-level application state.
/// 顶层应用状态。
///
/// Fields are public for macro access (`key_pressed!`, `key_state!`).
/// 字段为 pub 以便宏访问（`key_pressed!`、`key_state!`）。
#[derive(Default)]
pub struct App {
    /// Window position in screen coordinates. / 窗口在屏幕坐标中的位置。
    pub window_pos: (i32, i32),
    /// Window size in physical pixels. / 窗口大小（物理像素）。
    pub window_size: (u32, u32),
    /// Keyboard input state. / 键盘输入状态。
    pub keyboard_input: keyboard_input::KeyboardInput,
    /// Mouse input state. / 鼠标输入状态。
    pub mouse_input: mouse_input::MouseInput,
    /// Loaded sound data keyed by name. / 按名称索引的已加载音效数据。
    pub sounds: HashMap<String, StaticSoundData>,
    /// Frame timer (FPS, delta time). / 帧计时器（FPS、帧间隔）。
    pub timer: timer::Timer,
    /// Runtime context — `None` before `on_init`. / 运行时上下文——`on_init` 前为 `None`。
    pub ctx: Option<AppContext>,
}

/// Check if a key is currently pressed. / 检查按键是否处于按下状态。
#[allow(unused)]
macro_rules! key_pressed {
    ($self:expr, $key:expr) => {
        $self.keyboard_input.get_key_state($key).is_pressed()
    };
}

/// Get the full key state. / 获取完整按键状态。
#[allow(unused)]
macro_rules! key_state {
    ($self:expr, $key:expr) => {
        $self.keyboard_input.get_key_state($key)
    };
}

impl App {
    /// Called once after the event loop starts.
    /// 事件循环启动后调用一次。
    ///
    /// Creates the window, initialises GPU/audio, loads textures and sounds,
    /// sets up the camera, tiles, and HUD text.
    /// 创建窗口、初始化 GPU/音频、加载纹理和音效、设置相机、图块和 HUD 文字。
    pub fn on_init(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) -> Result<()> {
        // ── Window & GPU ───────────────────────────────────────
        // Create the application window and initialise Direct3D 11.
        // 创建应用窗口并初始化 Direct3D 11。
        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("KrisuRJW")
                    .with_inner_size(Logical(LogicalSize {
                        width: 960.0,
                        height: 600.0,
                    })),
            )
            .unwrap_or_else(|e| panic!("window::create: {:#}", e));
        let gfx = D3D11::init_on_window(&window).unwrap_or_else(|e| panic!("gfx::init: {:#}", e));

        // ── Audio ──────────────────────────────────────────────
        // Initialise Kira audio manager with the default backend.
        // 使用默认后端初始化 Kira 音频管理器。
        let audio_mgr = AudioManager::<DefaultBackend>::new(Default::default())
            .context("AudioManager::new failed")?;

        macro_rules! insert_snd {
            ($name:expr, $dir:expr) => {
                self.sounds.insert(
                    $name.to_string(),
                    StaticSoundData::from_cursor(Cursor::new(include_bytes!($dir)))?,
                );
            };
        }
        insert_snd!("snd_ominous_cancel", "../snd_ominous_cancel.wav");
        insert_snd!("snd_ominous", "../snd_ominous.wav");

        // ── Texture ────────────────────────────────────────────
        // Load sprite sheet textures seth.png and seth2.png.
        // 加载精灵表纹理 seth.png 和 seth2.png。
        let mut textures = HashMap::new();
        let img = image::load_from_memory(include_bytes!("../seth.png"))
            .context("failed to load seth.png")?;
        let tex_info = load_texture_from_dynamic_image(&gfx.device, &img)?;
        println!("Seth.png: {:?}", tex_info);

        let tw = tex_info.width as f32;
        let th = tex_info.height as f32;
        let cell_w = tw / 4.0;
        let cell_h = th / 6.0;

        let tex_info = Arc::new(tex_info);
        textures.insert("seth".to_string(), tex_info);

        let img = image::load_from_memory(include_bytes!("../seth2.png"))
            .context("failed to load seth2.png")?;
        let tex_info = load_texture_from_dynamic_image(&gfx.device, &img)?;
        let tex_info = Arc::new(tex_info);
        textures.insert("seth2".to_string(), tex_info);

        // ── Batches ────────────────────────────────────────────
        // Create sprite and shape batches with pre-allocated vertex buffers.
        // 创建精灵批处理和形状批处理，预分配顶点缓冲区。
        let batch = SpriteBatch2D::new(
            &gfx.device,
            2048,
            &gfx.states.vs_puc_m_2d,
            &gfx.states.ps_tex_rgba_2d,
            &gfx.states.input_layout_puc,
        )?;
        let shape_batch = ShapeBatch2D::new(
            &gfx.device,
            4096,
            &gfx.states.vs_puc_m_2d,
            &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc,
        )?;

        // ── Tiles ──────────────────────────────────────────────
        // Create 24 animated tiles arranged in a grid with random-ish velocities.
        // 创建 24 个动画图块，排列成网格，速度随机。
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
                sprite_rect: Sprite2D {
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

        // ── Dynamic text atlas ─────────────────────────────────
        // Use the atlas-based text renderer which caches glyphs and
        // supports dynamic text changes each frame.
        // 使用基于图集的文字渲染器，缓存字形并支持每帧动态更新文字。
        let font_name = std::env::var("RJW_FONTNAME").unwrap_or_else(|_| "SimHei".to_string());
        let atlas_text = atlas_text::AtlasText::new(&gfx.device, -20.0, 12000.0)?;
        let text_buf = Sprite2DBuffer::default();
        let sprite_buf = Sprite2DBuffer::default();

        self.ctx = Some(AppContext {
            window,
            gfx,
            audio_mgr,
            batch,
            textures,
            shape_batch,
            tiles,
            camera,
            hovered_tile: None,
            grid_spacing: GRID_SPACING,
            font_name,
            atlas_text,
            text_buf,
            sprite_buf,
        });
        Ok(())
    }

    /// Called every frame by the winit event loop.
    /// 每帧由 winit 事件循环调用。
    ///
    /// Steps / 步骤:
    /// 1. Compute delta time / 计算帧间隔
    /// 2. Handle camera movement / 处理相机移动
    /// 3. Handle tile physics & hover / 处理图块物理和悬停
    /// 4. Render grid, sprites, colliders, HUD / 渲染网格、精灵、碰撞体、HUD
    /// 5. Present to screen / 提交到屏幕
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

        if key_pressed!(self, KeyCode::KeyQ) {
            camera.rotation -= rot_speed * dt;
        }
        if key_pressed!(self, KeyCode::KeyE) {
            camera.rotation += rot_speed * dt;
        }

        if self.mouse_input.get_mouse_wheel_delta().1 > 0.0 {
            camera.zoom *= zoom_speed.powf(dt);
        }
        if self.mouse_input.get_mouse_wheel_delta().1 < 0.0 {
            camera.zoom /= zoom_speed.powf(dt);
        }

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
        // Update tile positions, apply mouse attraction, air drag,
        // and detect which tile is under the cursor.
        // 更新图块位置、施加鼠标吸引力、空气阻力，检测光标下方的图块。
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

            let f: f32 = if key_pressed!(self, KeyCode::KeyX) {
                10.0
            } else {
                0.1
            };
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
            format!(
                "KrisuRJW - FPS: {:.2} dTime: {:.05}",
                self.timer.get_fps(),
                dt
            )
            .as_str(),
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
            // Textures
            let seth_tex = ctx.textures.get("seth").unwrap();
            let seth2_tex = ctx.textures.get("seth2").unwrap();

            // Batch
            let batch = &mut ctx.batch;

            // Buffer
            let buf = &mut ctx.sprite_buf;

            gfx.clear_screen(&[0.8, 0.8, 0.8, 1.0]);
            let vp_transposed = camera.vp_matrix().transpose();

            // Behind the Grid — demo of `for_each_sorted` with pipeline switching
            // 网格层之后 — 演示 `for_each_sorted` 的流水线切换
            buf.clear();
            let shadow_offset = Vec2::splat(25.0);
            let shadow_color: [f32; 4] = [0.0, 0.0, 0.0, 0.5];

            let obj = Sprite2DObject {
                spr: Sprite2D {
                    origin_px: seth2_tex.size_vec2f() * 0.5,
                    size_px: seth2_tex.size_vec2f(),
                    uv_tl_px: Vec2::ZERO,
                    uv_size_px: seth2_tex.size_vec2f(),
                },
                transform: Transform2D::default(),
                pipeline: TextureInfoArced(seth2_tex.clone()),
                color: [1.0; 4],
                layer: 0.0,
            };

            let obj = Sprite2DObject {
                transform: obj.transform.move_by(shadow_offset),
                color: shadow_color,
                ..obj
            };
            buf.push(&obj);
            let obj = Sprite2DObject {
                transform: obj.transform.move_by(-shadow_offset),
                color: [1.0; 4],
                ..obj
            };
            buf.push(&obj);

            let obj = Sprite2DObject {
                spr: Sprite2D {
                    origin_px: seth_tex.size_vec2f() * 0.5,
                    size_px: seth_tex.size_vec2f(),
                    uv_tl_px: Vec2::ZERO,
                    uv_size_px: seth_tex.size_vec2f(),
                },
                transform: Transform2D::default().with_pos(Vec2 { x: 0.0, y: -1000.0 }),
                pipeline: TextureInfoArced(seth_tex.clone()),
                color: [1.0; 4],
                layer: 0.0,
            };

            let obj = Sprite2DObject {
                transform: obj.transform.move_by(shadow_offset),
                color: shadow_color,
                ..obj
            };
            buf.push(&obj);
            let obj = Sprite2DObject {
                transform: obj.transform.move_by(-shadow_offset),
                color: [1.0; 4],
                ..obj
            };
            buf.push(&obj);

            let obj = Sprite2DObject {
                transform: Transform2D::default().with_pos(Vec2 { x: 0.0, y: 1000.0 }),
                ..obj
            };

            let obj = Sprite2DObject {
                transform: obj.transform.move_by(shadow_offset),
                color: shadow_color,
                ..obj
            };
            buf.push(&obj);
            let obj = Sprite2DObject {
                transform: obj.transform.move_by(-shadow_offset),
                color: [1.0; 4],
                ..obj
            };
            buf.push(&obj);

            let obj = Sprite2DObject {
                transform: Transform2D::default().with_pos(Vec2 { x: 1000.0, y: 0.0 }),
                ..obj
            };

            let obj = Sprite2DObject {
                transform: obj.transform.move_by(shadow_offset),
                color: shadow_color,
                ..obj
            };
            buf.push(&obj);
            let obj = Sprite2DObject {
                transform: obj.transform.move_by(-shadow_offset),
                color: [1.0; 4],
                ..obj
            };
            buf.push(&obj);

            let obj = Sprite2DObject {
                transform: Transform2D::default().with_pos(Vec2 { x: -1000.0, y: 0.0 }),
                ..obj
            };

            let obj = Sprite2DObject {
                transform: obj.transform.move_by(shadow_offset),
                color: shadow_color,
                ..obj
            };
            buf.push(&obj);
            let obj = Sprite2DObject {
                transform: obj.transform.move_by(-shadow_offset),
                color: [1.0; 4],
                ..obj
            };
            buf.push(&obj);

            batch.clear_batch();
            batch.set_mvp(gfx, &vp_transposed);
            batch.set_texture(seth2_tex.srv.clone(), seth2_tex.width, seth2_tex.height);
            buf.for_each_sorted(
                batch,
                |batch, pp| {
                    batch.submit_and_draw(gfx).unwrap_or_else(|_| return);
                    batch.clear_batch();
                    batch.set_texture(pp.0.srv.clone(), pp.0.width, pp.0.height);
                },
                |batch, spr| {
                    batch
                        .add(
                            spr.transform.pos,
                            spr.transform.scale,
                            spr.transform.rot,
                            &spr.spr,
                            spr.color,
                        )
                        .unwrap_or_else(|_| return);
                },
            );
            batch.submit_and_draw(gfx)?;

            // Grid
            let sb = &mut ctx.shape_batch;
            sb.clear_batch();
            build_grid(sb, camera, ctx.grid_spacing, GRID_COLOR);
            sb.set_mvp(gfx, &vp_transposed);
            sb.submit_and_draw(gfx)
                .context("grid submit_and_draw failed")?;
            sb.clear_batch();

            // Tiles
            batch.clear_batch();
            batch.set_texture(seth_tex.srv.clone(), seth_tex.width, seth_tex.height);
            for tile in &ctx.tiles {
                batch
                    .add(
                        tile.pos + Vec2::new(5.0, 5.0),
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
            sb.submit_and_draw(gfx)
                .context("shape_batch submit_and_draw failed")?;
            sb.clear_batch();

            // ── Text overlay (screen-space HUD) ────────────────
            unsafe {
                gfx.imm_context
                    .OMSetBlendState(&gfx.states.blend_alpha, None, 0xFFFFFFFF);
                gfx.imm_context
                    .RSSetState(&gfx.states.rasterizer_solid_cull_none);
                gfx.imm_context
                    .OMSetDepthStencilState(&gfx.states.depth_none, 0);
            }

            // Build HUD text using the dynamic atlas.
            // 使用动态图集构建 HUD 文字。
            use cosmic_text::{Attrs, Metrics, Shaping};

            let hud_mvp = glam::Mat4::orthographic_lh(0.0, w, h, 0.0, 0.0, 1.0);
            let hud_vp = hud_mvp.transpose();

            let text_to_display = format!(
                "FPS: {:.2} | Delta: {:.05}ms\nHello, Rust! 🦀\nKrisuRJW - Atlas Renderer　渲染文字到２Ｄ精灵✔✔✔\n　ゆっくりしていってね (❁´◡`❁)",
                self.timer.get_fps(),
                dt,
            );

            // Clear previous text and render new glyphs into text_buf
            ctx.text_buf.clear();
            // Lay out text once, render twice (shadow + primary).
            use cosmic_text::Family;

            let layout = ctx.atlas_text.layout_text(
                &text_to_display,
                Metrics::new(24.0, 28.0),
                Attrs::new().family(Family::Name(&ctx.font_name)),
                Shaping::Advanced,
                &gfx.device,
            )?;

            // Shadow
            ctx.atlas_text.render_layout_simple(
                &layout,
                Vec2::new(10.0, 6.0),
                [0.0, 0.0, 0.0, 0.75],
                0.0,
                &mut ctx.text_buf,
            );

            // Primary text
            ctx.atlas_text.render_layout_simple(
                &layout,
                Vec2::new(8.0, 4.0),
                [1.0, 1.0, 1.0, 1.0],
                0.0,
                &mut ctx.text_buf,
            );

            // Upload atlas dirty pages to GPU before rendering
            ctx.atlas_text.upload(gfx)?;

            // Render the text sprites in sorted order
            batch.clear_batch();
            batch.set_mvp(gfx, &hud_vp);
            // Use the first atlas page texture as initial texture
            if ctx.atlas_text.page_count() > 0 {
                let ti = ctx.atlas_text.texture_info(0);
                batch.set_texture(ti.srv.clone(), ti.width, ti.height);
            }
            ctx.text_buf.for_each_sorted(
                batch,
                |batch, pp| {
                    batch.submit_and_draw(gfx).unwrap_or_else(|_| {});
                    batch.clear_batch();
                    batch.set_texture(pp.0.srv.clone(), pp.0.width, pp.0.height);
                },
                |batch, spr| {
                    batch
                        .add(
                            spr.transform.pos,
                            spr.transform.scale,
                            spr.transform.rot,
                            &spr.spr,
                            spr.color,
                        )
                        .unwrap_or_else(|_| {});
                },
            );
            batch
                .submit_and_draw(gfx)
                .context("text_buf submit_and_draw failed")?;
            batch.clear_batch();
        }

        gfx.present().context("gfx::present failed")?;
        self.timer.post_frame_fpsc();
        Ok(())
    }
}

/// Build a perspective grid of lines visible within the camera frustum.
/// 构建相机视锥内可见的透视线网格。
///
/// Draws vertical and horizontal lines at `spacing` intervals, clamped to `max_lines`.
/// 以 `spacing` 间隔绘制水平和垂直线，上限为 `max_lines`。
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
                10.0,
                col,
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
                10.0,
                col,
            );
        }
        y += spacing;
    }
}

/// Draw the outline of a collider shape (rect/AABB/circle).
/// 绘制碰撞体形状的轮廓（矩形/AABB/圆形）。
///
/// Transforms local-space vertices into world space using the instance transform.
/// 使用实例变换将局部空间顶点变换到世界空间。
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
