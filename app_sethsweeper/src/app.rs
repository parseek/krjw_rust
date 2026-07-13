//! # Application layer — `App`
//!
//! Ties together engine, game state, physics, and rendering.
//! 整合引擎、游戏状态、物理和渲染。

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use anyhow::{Context, Result};
use glam::Vec2;
use winit::keyboard::KeyCode;

use kira::{AudioManager, DefaultBackend, sound::static_sound::StaticSoundData};

use krjw_engine::{
    self, AppMsg, TextureInfoArced, Timer,
    atlas_text::AtlasText,
    camera2d::Camera2D,
    collider::{Collider, ColliderInstance},
    event_driver::EventDriver,
    graphic::d3d11::D3D11,
    graphic::d3d11::d3d11_utils::*,
    graphic::d3d11::shape_batch_2d::ShapeBatch2D,
    graphic::d3d11::sprite_batch_2d::SpriteBatch2D,
    mouse_input::MouseButton,
    sprite2d::{Sprite2D, Sprite2DBuffer, Sprite2DObject},
    transform2d::Transform2D,
};

/// Macro: check if a key is currently pressed.
macro_rules! key_pressed {
    ($driver:expr, $key:expr) => {
        $driver.keyboard().get_key_state($key).is_pressed()
    };
}

/// Macro: get the full key state.
macro_rules! key_state {
    ($driver:expr, $key:expr) => {
        $driver.keyboard().get_key_state($key)
    };
}

/// Spacing between grid lines in world units.
const GRID_SPACING: f32 = 100.0;
/// Grid line colour (RGBA).
const GRID_COLOR: [f32; 4] = [0.15, 0.15, 0.15, 1.0];

/// Runtime application context — holds engine resources via EngineContext.
/// Also contains game-specific state: tiles, hover, custom_text, etc.
pub struct App {
    // ── Engine-level resources ──
    pub ctx: Option<AppContext>,

    // ── Game state ──
    /// Loaded sound data keyed by name.
    pub sounds: HashMap<String, StaticSoundData>,
    /// Frame timer (FPS, delta time).
    pub timer: Timer,
}

/// Holds all engine resources and game-specific data that used to be in AppContext.
/// This is the merge of the original AppContext and the game-related fields.
pub struct AppContext {
    // ── Engine resources ──
    pub window: winit::window::Window,
    pub gfx: D3D11,
    pub audio_mgr: AudioManager,
    pub batch: SpriteBatch2D,
    pub textures: HashMap<String, Arc<TextureInfo>>,
    pub shape_batch: ShapeBatch2D,
    pub camera: Camera2D,
    pub atlas_text: AtlasText,
    pub text_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    pub sprite_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,

    // ── Game state ──
    pub tiles: Vec<Tile>,
    pub hovered_tile: Option<usize>,
    pub grid_spacing: f32,
    pub font_name: String,
    pub custom_text: String,
}

/// A single bouncing sprite tile.
pub struct Tile {
    pos: Vec2,
    vel: Vec2,
    rot: f32,
    rot_vel: f32,
    scale: f32,
    sprite_rect: Sprite2D,
    collider: Collider,
    color: [f32; 4],
}

impl Default for App {
    fn default() -> Self {
        Self {
            ctx: None,
            sounds: HashMap::new(),
            timer: Timer::default(),
        }
    }
}

// ──────────────────────────────────────────────
//  App — thread entry point & main loop
// ──────────────────────────────────────────────
impl App {
    /// Entry point for the App thread.
    /// Initialises everything and runs the frame loop.
    pub fn run(
        &mut self,
        window: winit::window::Window,
        hwnd: isize,
        rx: Receiver<AppMsg>,
    ) -> Result<()> {
        use windows::Win32::Foundation::HWND;
        let gfx = D3D11::init_on_hwnd(HWND(hwnd as *mut _))
            .unwrap_or_else(|e| panic!("gfx::init: {:#}", e));

        let size = window.inner_size();

        // ── Event driver ──
        let mut driver = EventDriver::new(rx);
        driver.set_initial_window_size(size.width, size.height);

        // ── Audio ──
        let audio_mgr = self.init_audio()?;

        // ── Batches ──
        let (batch, shape_batch) = Self::init_batches(&gfx)?;

        // ── Texture ──
        let textures = Self::init_textures(&gfx)?;

        // ── Tiles ──
        let tiles = Self::init_tiles(&textures);

        // ── Camera ──
        let camera = Self::init_camera(driver.window_size());

        self.startup_info();

        // ── Dynamic text atlas ──
        let (font_name, atlas_text, text_buf, sprite_buf) = Self::init_text_system(&gfx)?;

        let custom_text = std::env::var("RJW_TEXT")
            .unwrap_or("😂😂😊😂❤🌹😆😖🥪🥗🥞🥟🥩🍚🍤\n🛬✈🚊🚈🚝🚹🟧🟨🟩🟦🟪🟫⬛⬜🔹".to_string());

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
            custom_text,
            atlas_text,
            text_buf,
            sprite_buf,
        });

        // ── Main loop ──
        println!("[AppThread] entering main loop");
        self.main_loop(&mut driver)?;
        println!("[AppThread] main loop ended");

        Ok(())
    }

    /// Main loop — uses EventDriver for message processing and input state.
    fn main_loop(&mut self, driver: &mut EventDriver) -> Result<()> {
        loop {
            let events = driver.poll_frame();
            if events.close_requested || events.disconnected {
                break;
            }

            if driver.window_size_dirty() {
                if let Some(ctx) = self.ctx.as_mut() {
                    let (w, h) = driver.window_size();
                    ctx.gfx
                        .on_resize(w, h)
                        .unwrap_or_else(|e| panic!("gfx::resize: {:#}", e));
                }
                driver.clear_window_size_dirty();
            }

            let dt = self.delta_time();
            self.handle_camera_input(dt, driver);
            self.handle_sound_effects(driver);
            self.update_tiles(dt, driver);
            self.update_window_title(dt);
            self.render_frame(dt, driver)?;

            let ctx = self.ctx.as_mut().unwrap();
            ctx.gfx.present().context("gfx::present failed")?;
            self.timer.post_frame_fpsc(dt as f64);

            driver.end_frame();
        }

        Ok(())
    }

    fn startup_info(&self) {
        println!("赛博吸尘器 with Seth.png");
        println!("    ---- 🔪Aqua's idea");
        println!("操作方式：");
        println!("  - AD WS 移动相机");
        println!("  - Q / E 旋转相机");
        println!("  - 鼠标滚轮缩放相机");
        println!("  - 鼠标左键吸引图块");
        println!("  - X 键强力制动");
    }

    fn init_audio(&mut self) -> Result<AudioManager> {
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

        AudioManager::<DefaultBackend>::new(Default::default()).context("AudioManager::new failed")
    }

    fn init_batches(gfx: &D3D11) -> Result<(SpriteBatch2D, ShapeBatch2D)> {
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
        Ok((batch, shape_batch))
    }

    fn init_textures(gfx: &D3D11) -> Result<HashMap<String, Arc<TextureInfo>>> {
        let mut textures = HashMap::new();
        macro_rules! insert_tex {
            ($name:expr, $dir:expr) => {
                let img = image::load_from_memory(include_bytes!($dir))
                    .context(concat!("loading ", $name, " (", $dir, ") failed"))?;
                let tex_info = load_texture_from_dynamic_image(&gfx.device, &img)?;
                let tex_info = Arc::new(tex_info);
                textures.insert($name.to_string(), tex_info);
            };
        }
        insert_tex!("seth", "../seth.png");
        insert_tex!("seth2", "../seth2.png");

        Ok(textures)
    }

    fn init_tiles(textures: &HashMap<String, Arc<TextureInfo>>) -> Vec<Tile> {
        const TEX_NAME: &str = "seth";
        let w_count = 12;
        let h_count = 9;
        let texture = textures.get(TEX_NAME).unwrap().as_ref();
        let cell_w = texture.width as f32 / w_count as f32;
        let cell_h = texture.height as f32 / h_count as f32;

        let mut tiles = Vec::new();
        for i in 0..w_count * h_count {
            let cx = (i % w_count) as f32 * cell_w;
            let cy = (i / w_count) as f32 * cell_h;
            let angle = i as f32 * 1.3;
            let col = (i % w_count) as f32;
            let row = (i / w_count) as f32;

            tiles.push(Tile {
                pos: Vec2::new((col - 1.5) * 150.0, (row - 2.5) * 150.0),
                vel: Vec2::new(
                    (i as f32 * 0.7).cos() * 200.0,
                    (i as f32 * 1.1).sin() * 200.0,
                ),
                rot: 0.0,
                rot_vel: ((i as f32 * 0.5).cos() * 2.0).abs(),
                scale: 0.5 + (i % 3) as f32 * 0.08,
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
        tiles
    }

    fn init_camera(window_size: (u32, u32)) -> Camera2D {
        let ws = Vec2::new(window_size.0 as f32, window_size.1 as f32);
        Camera2D::new(ws)
    }

    fn init_text_system(
        gfx: &D3D11,
    ) -> Result<(
        String,
        AtlasText,
        Sprite2DBuffer<TextureInfoArced, Transform2D>,
        Sprite2DBuffer<TextureInfoArced, Transform2D>,
    )> {
        let font_name = std::env::var("RJW_FONTNAME").unwrap_or_else(|_| "SimHei".to_string());
        let atlas_text = AtlasText::new(&gfx.device, -20.0, 12000.0)?;
        let text_buf = Sprite2DBuffer::default();
        let sprite_buf = Sprite2DBuffer::default();
        Ok((font_name, atlas_text, text_buf, sprite_buf))
    }
}

// ──────────────────────────────────────────────
//  on_frame sub-functions (called every frame)
// ──────────────────────────────────────────────
impl App {
    /// Compute clamped delta time for this frame.
    fn delta_time(&mut self) -> f64 {
        let dt = self.timer.pre_frame_and_get_delta_time() as f64;
        if dt > 0.2 {
            eprintln!("dt too long: {}", dt);
            0.2
        } else {
            dt
        }
    }

    /// Handle camera movement.
    fn handle_camera_input(&mut self, dt: f64, driver: &EventDriver) {
        let ctx = self.ctx.as_mut().unwrap();
        let camera = &mut ctx.camera;

        let move_speed = 500.0;
        let rot_speed = 2.0;
        let zoom_speed: f32 = 25.0;

        if key_pressed!(driver, KeyCode::KeyQ) {
            camera.rotation -= rot_speed * dt as f32;
        }
        if key_pressed!(driver, KeyCode::KeyE) {
            camera.rotation += rot_speed * dt as f32;
        }

        if let Some(pixel) = driver.mouse().get_pixel_wheel() {
            if pixel.1 > 0.0 {
                camera.zoom *= (zoom_speed * 0.02).powf(dt as f32 * pixel.1 as f32);
            }
            if pixel.1 < 0.0 {
                camera.zoom /= (zoom_speed * 0.02).powf(dt as f32 * (-pixel.1) as f32);
            }
        } else {
            if driver.mouse().get_mouse_wheel_delta().1 > 0.0 {
                camera.zoom *= zoom_speed.powf(dt as f32);
            }
            if driver.mouse().get_mouse_wheel_delta().1 < 0.0 {
                camera.zoom /= zoom_speed.powf(dt as f32);
            }
        }

        let (sin_rot, cos_rot) = camera.rotation.sin_cos();
        let mut move_dir = Vec2::ZERO;
        if key_pressed!(driver, KeyCode::KeyD) {
            move_dir += Vec2::new(cos_rot, sin_rot);
        }
        if key_pressed!(driver, KeyCode::KeyA) {
            move_dir -= Vec2::new(cos_rot, sin_rot);
        }
        if key_pressed!(driver, KeyCode::KeyW) {
            move_dir -= Vec2::new(-sin_rot, cos_rot);
        }
        if key_pressed!(driver, KeyCode::KeyS) {
            move_dir += Vec2::new(-sin_rot, cos_rot);
        }
        if move_dir.length_squared() > 0.0 {
            camera.position += move_dir.normalize() * move_speed * dt as f32;
        }

        camera.viewport_pos = Vec2::splat(0.0f32);
        let (w, h) = driver.window_size();
        camera.viewport_size = Vec2::new(w as f32, h as f32);

        camera.apply_viewport(&ctx.gfx);
    }

    /// Play sound effects based on input events.
    fn handle_sound_effects(&mut self, driver: &EventDriver) {
        let ctx = self.ctx.as_mut().unwrap();
        let audio_mgr = &mut ctx.audio_mgr;

        if key_state!(driver, KeyCode::KeyX).is_down_true_edge() {
            if let Some(snd) = self.sounds.get("snd_ominous_cancel") {
                audio_mgr.play(snd.clone().volume(0.0)).unwrap();
            }
        }
        if driver
            .mouse()
            .get_mouse_button_state(MouseButton::Left)
            .is_down_edge()
        {
            if let Some(snd) = self.sounds.get("snd_ominous") {
                audio_mgr.play(snd.clone().volume(0.0)).unwrap();
            }
        }
    }

    /// Update tile physics.
    fn update_tiles(&mut self, dt: f64, driver: &EventDriver) {
        let ctx = self.ctx.as_mut().unwrap();
        let camera = &mut ctx.camera;

        let mouse_screen = driver.mouse().get_mouse_pos_vec2();
        let world_mouse = camera.screen_to_world(mouse_screen);
        let lmb_pressed = driver
            .mouse()
            .get_mouse_button_state(MouseButton::Left)
            .is_pressed();

        ctx.hovered_tile = None;
        for (idx, tile) in ctx.tiles.iter_mut().enumerate().rev() {
            tile.pos += tile.vel * dt as f32;
            tile.rot += tile.rot_vel * dt as f32;

            if lmb_pressed {
                let d = world_mouse - tile.pos;
                let len = d.length();
                let a = len.sqrt();
                let a = d / len * a;
                tile.vel += a;
            }

            let f: f32 = if key_pressed!(driver, KeyCode::KeyX) {
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
    }

    /// Update window title with FPS and delta time.
    fn update_window_title(&mut self, dt: f64) {
        let ctx = self.ctx.as_mut().unwrap();
        ctx.window.set_title(
            format!(
                "KrisuRJW - FPS: {:.2} dTime: {:.05}",
                self.timer.get_fps(),
                dt
            )
            .as_str(),
        );
    }

    /// Render demo sprites (background logo with shadow + push_buffered).
    fn render_demo_sprites(&mut self) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();
        let gfx = &ctx.gfx;
        let camera = &ctx.camera;
        let seth_tex = ctx.textures.get("seth").unwrap();
        let seth2_tex = ctx.textures.get("seth2").unwrap();
        let buf = &mut ctx.sprite_buf;
        let batch = &mut ctx.batch;
        let vp_transposed = camera.vp_matrix().transpose();

        let shadow_offset = Vec2::splat(25.0);
        let shadow_color: [f32; 4] = [0.0, 0.0, 0.0, 0.5];

        buf.clear();

        let mut push_sprite = |obj: &Sprite2DObject<TextureInfoArced, Transform2D>| {
            let mut shadow = obj.clone();
            shadow.transform = shadow.transform.move_by(shadow_offset);
            shadow.color = shadow_color;
            buf.push(&shadow);
            buf.push(obj);
        };

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
        push_sprite(&obj);

        let base = Sprite2DObject {
            spr: Sprite2D {
                origin_px: seth_tex.size_vec2f() * 0.5,
                size_px: seth_tex.size_vec2f(),
                uv_tl_px: Vec2::ZERO,
                uv_size_px: seth_tex.size_vec2f(),
            },
            transform: Transform2D::default(),
            pipeline: TextureInfoArced(seth_tex.clone()),
            color: [1.0; 4],
            layer: 0.0,
        };

        for offset in [
            Vec2::new(0.0, -1000.0),
            Vec2::new(0.0, 1000.0),
            Vec2::new(1000.0, 0.0),
            Vec2::new(-1000.0, 0.0),
        ] {
            let obj = Sprite2DObject {
                transform: Transform2D::default().with_pos(offset),
                pipeline: base.pipeline.clone(),
                ..base
            };
            push_sprite(&obj);
        }

        batch.push_buffered(gfx, &vp_transposed, buf, |xform| {
            (xform.pos, xform.scale, xform.rot)
        });
        Ok(())
    }

    /// Render the perspective grid.
    fn render_grid(&mut self) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();
        let gfx = &ctx.gfx;
        let camera = &ctx.camera;
        let vp_transposed = camera.vp_matrix().transpose();

        let sb = &mut ctx.shape_batch;
        sb.clear_batch();
        build_grid(sb, camera, ctx.grid_spacing, GRID_COLOR);
        sb.set_mvp(gfx, &vp_transposed);
        sb.submit_and_draw(gfx)
            .context("grid submit_and_draw failed")?;
        sb.clear_batch();
        Ok(())
    }

    /// Render tile sprites (shadow + colour) using push_buffered.
    fn render_tiles(&mut self) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();
        let gfx = &ctx.gfx;
        let camera = &ctx.camera;
        let seth_tex = ctx.textures.get("seth").unwrap();
        let batch = &mut ctx.batch;
        let buf = &mut ctx.sprite_buf;
        let vp_transposed = camera.vp_matrix().transpose();

        buf.clear();
        let seth_pipeline = TextureInfoArced(seth_tex.clone());
        for tile in &ctx.tiles {
            // Shadow
            buf.push(&Sprite2DObject {
                transform: Transform2D {
                    pos: tile.pos + Vec2::new(5.0, 5.0),
                    scale: Vec2::splat(tile.scale),
                    rot: tile.rot,
                },
                color: [0.0, 0.0, 0.0, 0.2],
                spr: tile.sprite_rect,
                pipeline: seth_pipeline.clone(),
                layer: 0.0,
            });
            // Main sprite
            buf.push(&Sprite2DObject {
                transform: Transform2D {
                    pos: tile.pos,
                    scale: Vec2::splat(tile.scale),
                    rot: tile.rot,
                },
                color: tile.color,
                spr: tile.sprite_rect,
                pipeline: seth_pipeline.clone(),
                layer: 0.0,
            });
        }

        batch.push_buffered(gfx, &vp_transposed, buf, |xform| {
            (xform.pos, xform.scale, xform.rot)
        });
        Ok(())
    }

    /// Render collider outlines and mouse cursor circle.
    fn render_colliders(&mut self, driver: &EventDriver) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();
        let gfx = &ctx.gfx;
        let camera = &ctx.camera;
        let vp_transposed = camera.vp_matrix().transpose();

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
        if driver
            .mouse()
            .get_mouse_button_state(MouseButton::Left)
            .is_pressed()
        {
            let mouse_screen = driver.mouse().get_mouse_pos_vec2();
            let world_mouse = camera.screen_to_world(mouse_screen);
            sb.add_circle_no_uv(world_mouse, 30.0, [1.0, 1.0, 1.0, 0.3], 24);
        }
        sb.set_mvp(gfx, &vp_transposed);
        sb.submit_and_draw(gfx)
            .context("shape_batch submit_and_draw failed")?;
        sb.clear_batch();
        Ok(())
    }

    /// Render HUD text overlay (screen-space) using push_buffered.
    fn render_hud(&mut self, dt: f64, driver: &EventDriver) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();
        let gfx = &ctx.gfx;

        unsafe {
            gfx.imm_context
                .OMSetBlendState(&gfx.states.blend_alpha, None, 0xFFFFFFFF);
            gfx.imm_context
                .RSSetState(&gfx.states.rasterizer_solid_cull_none);
            gfx.imm_context
                .OMSetDepthStencilState(&gfx.states.depth_none, 0);
        }

        use cosmic_text::{Attrs, Family, Metrics, Shaping};

        let (w, h) = driver.window_size();
        let w = w as f32;
        let h = h as f32;

        let hud_mvp = glam::Mat4::orthographic_lh(0.0, w, h, 0.0, 0.0, 1.0);
        let hud_vp = hud_mvp.transpose();

        let text_to_display = format!(
            "FPS: {:.2} | Delta: {:.05}ms\nHello, Rust! 🦀\nKrisuRJW - Atlas Renderer　渲染文字到２Ｄ精灵✔✔✔\n　ゆっくりしていってね (❁´◡`❁)\n{}",
            self.timer.get_fps(),
            dt,
            ctx.custom_text,
        );

        ctx.text_buf.clear();
        let layout = ctx.atlas_text.layout_text(
            &text_to_display,
            Metrics::new(24.0, 32.0),
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

        let batch = &mut ctx.batch;
        batch.push_buffered(gfx, &hud_vp, &mut ctx.text_buf, |xform| {
            (xform.pos, xform.scale, xform.rot)
        });

        Ok(())
    }

    /// Render a full frame.
    fn render_frame(&mut self, dt: f64, driver: &EventDriver) -> Result<()> {
        let (w, h) = driver.window_size();
        if w == 0 || h == 0 {
            return Ok(());
        }

        {
            let ctx = self.ctx.as_ref().unwrap();
            let gfx = &ctx.gfx;
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
        }

        self.render_demo_sprites()?;
        self.render_grid()?;
        self.render_tiles()?;
        self.render_colliders(driver)?;
        self.render_hud(dt, driver)?;

        Ok(())
    }
}

// ──────────────────────────────────────────────
//  Utility functions (only used by App)
// ──────────────────────────────────────────────

/// Build a perspective grid of lines visible within the camera frustum.
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

/// Draw the outline of a collider shape.
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
