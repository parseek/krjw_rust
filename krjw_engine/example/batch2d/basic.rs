use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Ok, Result};
use glam::Vec2;
use krjw_engine::graphic::d3d11::rstate::{BlendMode, SamplerMode};
use krjw_engine::{Camera2D, ColorRGBA, EventDriver, Sprite2D, Timer, Transform2D, engine_handler::{AppMsgReceiver, run_app}, graphic, krjw_vecf};
use winit::window::{Window, WindowAttributes};

use serde::Deserialize;

use graphic::d3d11::{ResourcePool, RState, batch2d::{Batch2D, SortKey, TextureId}, ResourceManager, D3D11};

#[derive(Default, Debug, Clone, Deserialize)]
struct SpritePart {
    pub tex: String,
    pub lt: Vec2,
    pub wh: Vec2,
    pub or: Vec2,
}

const PRELOAD_TEXTURES: &[(&str, &[u8])] = &[
    ("rjw", include_bytes!("krisurjw.png")),
    ("ralsei", include_bytes!("ralsei.png")),
    ("rjw2", include_bytes!("rjw2.png")),
];

struct App {
    _window: Window,
    event: EventDriver,
    gfx: D3D11,
    timer: Timer,
    rs_mgr: Arc<Mutex<ResourceManager>>,
    sp_parts: ResourcePool<SpritePart>,
    batch: Batch2D,
    ctx: Ctx,
}

/// 屏幕上的 [`Chara`]
/// 
/// `sp_parts` 中定义的 id（如 spr.toml 里的 y2、x1 等）
/// 
/// 但是会旋转
struct Chara {
    pub spr_id: u32,
    pub pos: Vec2,
    pub rot: f32,
    pub rv: f32,
}

impl Chara {
    pub fn step(&mut self, _dt_f64: f64, dt_f32: f32) {
        self.rot += self.rv * dt_f32
    }
    pub fn transform(&self) -> Transform2D {
        Transform2D::default().with_pos(self.pos).with_rot(self.rot)
    }
}

struct Ctx {
    pub camera: Camera2D,
    pub charas: Vec<Chara>,
}

impl Default for Ctx {
    fn default() -> Self {
        Self {
            camera: Camera2D::default(),
            charas: Vec::new(),
        }
    }
}

impl App {
    pub fn new(window: Window, hwnd: isize, rx: AppMsgReceiver) -> Result<Self> {
        let gfx = D3D11::init_on_hwnd(hwnd)?;
        let event = EventDriver::new(rx, &window);
        let rs_mgr = ResourceManager::new(gfx.device.clone());
        let rs_mgr = Arc::new(Mutex::new(rs_mgr));
        let batch = Batch2D::new(gfx.device.clone(), rs_mgr.clone())?;

        Ok(Self {
            _window: window,
            event,
            gfx,
            sp_parts: ResourcePool::new(),
            batch,
            rs_mgr,
            timer: Timer::default(),
            ctx: Ctx::default(),
        })
    }
}

impl App {
    pub fn init(&mut self) -> Result<()> {
        let rs_mgr = &mut self.rs_mgr.lock().unwrap();
        PRELOAD_TEXTURES.iter().for_each(|&(name, buffer)| {
            let img = image::load_from_memory(buffer).expect(&format!("Loading image {} failed", name));
            let tex = graphic::d3d11::d3d11_utils::load_texture_from_dynamic_image(&self.gfx.device, &img).expect(&format!("Loading texture {} failed", name));
            let tex = Arc::new(tex);
            rs_mgr.textures.insert(name, tex);
            println!("Texture {} loaded.", name);
        });

        // ZeroCopy 读取
        let spr_def: &'static str = include_str!("spr.toml");
        #[derive(Deserialize)]
        struct SpritePartFile {
            #[serde(alias = "p")]
            parts: HashMap<String, SpritePart>,
        }
        let spr_file: SpritePartFile = toml::from_str(spr_def).context("Reading spr.toml failed")?;
        let spr_def = spr_file.parts;

        // 将 HashMap 中的每个 key-value 对插入 ResourcePool
        let mut spr_ids: Vec<u32> = Vec::new();
        for (name, part) in spr_def.into_iter() {
            let id = self.sp_parts.insert(&name, part);
            spr_ids.push(id);
        }

        #[cfg(debug_assertions)]
        eprintln!("sp_parts: {} entries", self.sp_parts.names().count());

        // 初始化 charas
        for (i, &part_id) in spr_ids.iter().enumerate() {
            let part = self.sp_parts.get(part_id).unwrap();
            println!("  Chara {}: id={}, tex={} ({:?}x{:?})", i, part_id, part.tex, part.wh, part.wh);
            self.ctx.charas.push(Chara {
                spr_id: part_id,
                pos: krjw_vecf!((i as f32 - spr_ids.len() as f32 * 0.5 + 0.5) * 200.0, 0.0),
                rot: 0.0,
                rv: 0.3 + (i as f32 * 0.5).cos() * 0.5,
            });
        }

        println!("Initialized {} charas.", self.ctx.charas.len());

        Ok(())
    }

    pub fn step_event(&mut self, dt_f64: f64, dt_f32: f32) -> Result<()> {
        for chara in &mut self.ctx.charas {
            chara.step(dt_f64, dt_f32);
        }
        Ok(())
    }

    pub fn render(&mut self, _dt_f64: f64, _dt_f32: f32) -> Result<()> {
        self.gfx.clear_screen(&[0., 0.2, 0.5, 1.]);

        unsafe {
            self.gfx.imm_context.OMSetRenderTargets(Some(&[Some(self.gfx.rtv().clone())]), Some(self.gfx.dsv()));
        }

        let wh_u32 = self.event.window_size();
        let wh = krjw_vecf!(wh_u32.0, wh_u32.1);

        self.gfx.set_viewport(0.0, 0.0, wh.x, wh.y);

        // 构建相机
        self.ctx.camera.viewport_size = wh;
        let mvp = self.ctx.camera.vp_matrix().transpose();

        let rs_mgr = self.rs_mgr.lock().unwrap();

        let rstate = RState::new_basic_builder().blend(BlendMode::Normal).sampler(SamplerMode::PointClamp).build();

        // 添加每个 chara 的精灵
        for chara in &self.ctx.charas {
            let part = self.sp_parts.get(chara.spr_id).unwrap();
            let (tex_id, tex_info) = rs_mgr.get_texture_or_white(&part.tex);

            let sprite = Sprite2D::from_uv(part.lt, part.wh).with_origin(part.or);

            let key = SortKey::new(0, TextureId(tex_id), rstate);
            let color = ColorRGBA::WHITE;

            self.batch.add_sprite(&sprite, &chara.transform(), color, key, 0.0, tex_info.size_inv);
        }

        drop(rs_mgr);

        // 提交绘制
        self.batch.set_mvp(&self.gfx.imm_context, &mvp);
        self.batch.draw_flush(&self.gfx.imm_context).context("Batch2D draw_flush failed")?;

        self.gfx.present().context("[D3D11 GFX] Presenting failed")?;

        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        self.init().context("Initializing failed")?;
        loop {
            if self.event.poll_frame().to_quit() {
                break;
            }

            self.event.if_window_size_dirty(|width, height| {
                self.gfx.on_resize(width, height).context("Resizing the D3D11 graphic engine failed.")
            })?;

            let dt_f64 = self.timer.pre_frame_and_get_delta_time();
            let dt_f32 = dt_f64 as f32;

            self.step_event(dt_f64, dt_f32).context("Step processing failed")?;
            self.render(dt_f64, dt_f32).context("Rendering failed")?;

            self.timer.post_frame_fpsc(dt_f64);
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    run_app(WindowAttributes::default()
        .with_title("Test: Batch2D Basic")
        .with_inner_size(winit::dpi::LogicalSize {width: 960.0, height: 600.0 })
        , |window, hwnd, rx| {
        let mut app = App::new(window, hwnd, rx).context("Creating the app instance failed")?;
        app.run().context("Error occurred when the app is running")
    }).context("App failed")
}