use std::f64::consts::PI;

use image::GenericImageView;
use anyhow::{Context, Result};
use winit::keyboard::KeyCode;

use graphic::d3d11::sprite_batch_2d::{Sprite, SpriteBatch2D};
use graphic::d3d11::test_sprite::TestSpriteRender;
use graphic::d3d11::test_triangle::TestTriangleRender;
use graphic::d3d11::D3D11;


mod key_state;
mod keyboard_input;
mod mouse_input;

use mouse_input::MouseButton;

mod graphic;
mod handler;
mod timer;

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
    pos: [f32; 2],
    vel: [f32; 2],
    rot: f32,
    rot_vel: f32,
    scale: f32,
    sprite_rect: Sprite,
    color: [f32; 4],
}

#[derive(Default)]
struct State {
    red: f32,
    green: f32,
    blue: f32,
    triangle_render: Option<TestTriangleRender>,
    sprite: Option<TestSpriteRender>,
    rot: f64,
    auto_rot: bool,

    // SpriteBatch2D test
    batch: Option<SpriteBatch2D>,
    tiles: Vec<Tile>,
    batch_tex_srv: Option<graphic::d3d11::d3d11_utils::TextureInfo>,
}

impl State {
    fn new(gfx: &D3D11) -> Result<Self> {
        let tri_render = TestTriangleRender::new(&gfx.device)?;

        // Load texture from seth.png
        let img = image::load_from_memory(include_bytes!("../seth.png"))
        // let img = image::load_from_memory(include_bytes!("../../yssy.jpg"))
            .context("failed to load seth.png")?;
        let (tex_w, tex_h) = img.dimensions();

        let rgba = img.to_rgba8();
        let sprite = TestSpriteRender::new(&gfx.device, &rgba.into_raw(), tex_w, tex_h)?;

        // ── SpriteBatch2D test ──────────────────────────────────
        let tex_info = graphic::d3d11::d3d11_utils::load_texture_from_dynamic_image(
            &gfx.device,
            &img,
        )?;
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

            tiles.push(Tile {
                pos: [
                    100.0 + i as f32 * 60.0,
                    100.0 + (i % 7) as f32 * 80.0,
                ],
                vel: [
                    (i as f32 * 0.7).cos() * 200.0,
                    (i as f32 * 1.1).sin() * 200.0,
                ],
                rot: 0.0,
                rot_vel: (i as f32 * 0.5).cos() * 2.0,
                scale: 0.2 + (i % 3) as f32 * 0.08,
                sprite_rect: Sprite {
                    origin_px: [cell_w / 2.0, cell_h / 2.0],
                    size_px: [cell_w, cell_h],
                    uv_tl_px: [cx, cy],
                    uv_size_px: [cell_w, cell_h],
                },
                color: [
                    0.5 + (angle).sin() * 0.5,
                    0.5 + (angle + 2.0).sin() * 0.5,
                    0.5 + (angle + 4.0).sin() * 0.5,
                    1.0,
                ],
            });
        }

        Ok(Self {
            red: 0.0,
            green: 0.1,
            blue: 0.5,
            triangle_render: Some(tri_render),
            sprite: Some(sprite),
            auto_rot: true,
            batch: Some(batch),
            tiles,
            batch_tex_srv: Some(tex_info),
            ..Default::default()
        })
    }
}

macro_rules! key_pressed {
    ($self:expr, $key:expr) => {
        $self
            .keyboard_input
            .get_key_state($key)
            .is_pressed()
    };
}

macro_rules! key_state {
    ($self:expr, $key:expr) => {
        $self
            .keyboard_input
            .get_key_state($key)
    };
}

impl App {
    fn on_init(&mut self) -> Result<()> {
        let gfx = self.gfx.as_ref().context("App not initialized")?;
        self.state = Some(State::new(gfx).context("State::new failed")?);
        // println!("赛博吸尘器 with Seth.png");
        // println!("    ---- 🔪Aqua's idea");
        println!("赛博吸尘器 with Y水SY");
        println!("操作方式：R、J、W、Z、←、→");
        println!("  - X 键制动");
        println!("  - 鼠标左键吸引");
        Ok(())
    }
    fn on_frame(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = self.window.as_ref().unwrap();
        let gfx = self.gfx.as_ref().unwrap();
        let state = self.state.as_mut().unwrap();

        let w = self.window_size.0 as f32;
        let h = self.window_size.1 as f32;
        let (cur_x, cur_y) = self.mouse_input.get_mouse_position();

        let delta_time = self.timer.pre_frame_and_get_delta_time();
        let delta_time = if delta_time > 1.0 { 1.0 } else { delta_time };

        if key_pressed!(self, KeyCode::KeyR) {
            state.red = 1.0_f32
        }
        if key_pressed!(self, KeyCode::KeyJ) {
            state.blue = 1.0_f32
        }
        if key_pressed!(self, KeyCode::KeyW) {
            state.green = 1.0_f32
        }

        if key_pressed!(self, KeyCode::ArrowLeft) {
            state.rot -= 1.0 * PI * delta_time;
        }
        if key_pressed!(self, KeyCode::ArrowRight) {
            state.rot += 1.0 * PI * delta_time;
        }
        if key_state!(self, KeyCode::KeyZ).is_down_true_edge() {
            state.auto_rot = !state.auto_rot;
        }
        if state.auto_rot {
            state.rot += (if key_pressed!(self, KeyCode::KeyX) { 0.01 } else { 0.2 }) * PI * delta_time;
        }

        let lmb_pressed = self.mouse_input.get_mouse_button_state(MouseButton::Left).is_pressed();

        // ── SpriteBatch2D test: flying tiles ────────────────
        let dt = delta_time as f32;
        for tile in &mut state.tiles {
            tile.pos[0] += tile.vel[0] * dt;
            tile.pos[1] += tile.vel[1] * dt;
            tile.rot += tile.rot_vel * dt;

            if lmb_pressed {
                let distance_to_cursor = glam::Vec2::new(cur_x as f32 - tile.pos[0], cur_y as f32 - tile.pos[1]);
                let a = distance_to_cursor * 0.1;
                tile.vel[0] += a.x;
                tile.vel[1] += a.y;
            }

            let f : f32 = if key_pressed!(self, KeyCode::KeyX) { 10.0 } else { 0.1 };
            let len_sqr = tile.vel[0]*tile.vel[0] + tile.vel[1]*tile.vel[1];
            if (len_sqr) > 25.0 {
                let a = -glam::Vec2::from_array(tile.vel) * f / len_sqr.sqrt();
                tile.vel[0] += a.x;
                tile.vel[1] += a.y;
            }

            // Bounce off edges
            let half_size = 50.0; // approximate half tile size
            if tile.pos[0] < half_size { tile.vel[0] = tile.vel[0].abs(); }
            if tile.pos[0] > w - half_size { tile.vel[0] = -tile.vel[0].abs(); }
            if tile.pos[1] < half_size { tile.vel[1] = tile.vel[1].abs(); }
            if tile.pos[1] > h - half_size { tile.vel[1] = -tile.vel[1].abs(); }
        }

        window.set_title(
            format!(
                "KrisuRJW - FPS: {:.2} dTime: {:.05}",
                self.timer.get_fps(),
                delta_time
            )
            .as_str(),
        );

        if self.window_size.0 > 0 && self.window_size.1 > 0 {

            // ── Set states ─────────────────────────────────────────
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

            gfx.clear_screen(&[state.red, state.green, state.blue, 1.0]);
            gfx.set_viewport(0.0, 0.0, w, h);

            // ── Draw triangle ─────────────────────────────────────
            if let Some(triangle) = state.triangle_render.as_ref() {
                triangle.draw(gfx);
            }

            // ── Draw test sprite ───────────────────────────────────────
            if let Some(sprite) = state.sprite.as_ref() {
                let sw = sprite.tex_width as f32;
                let sh = sprite.tex_height as f32;

                // Orthographic projection (window coords: 0,0 = top-left)
                let mvp = glam::Mat4::orthographic_rh(0.0, w, h, 0.0, 0.0, 1.0);

                // Sprite transform: center on screen + rotate
                let angle = state.rot as f32;
                let spr = glam::Mat4::from_translation(glam::Vec3::new(w / 2.0, h / 2.0, 0.0))
                    * glam::Mat4::from_rotation_z(angle)
                    * glam::Mat4::from_scale(glam::Vec3::splat(0.5));

                sprite
                    .draw(
                        gfx,
                        [sw / 2.0, sh / 2.0], // origin = center of sprite
                        [sw, sh],             // size = full texture size
                        [0.0, 0.0],           // UV top-left
                        [sw, sh],             // UV size = full texture
                        [1.0, 1.0, 1.0, 1.0], // color = white
                        &mvp.transpose(),
                        &spr.transpose(),
                    )
                    .unwrap_or(());
            }

            if let Some(batch) = state.batch.as_mut() {
                if let Some(tex) = state.batch_tex_srv.as_ref() {
                    batch.clear_batch();
                    batch.set_texture(tex.srv.clone(), tex.width, tex.height);
                    if lmb_pressed {
                        batch.add([cur_x as f32, cur_y as f32], [1.0; 2], 0.0, &state.tiles[0].sprite_rect, [1.0, 1.0, 1.0, 0.2])
                            .unwrap_or_else(|e| panic!("batch.add failed: {:#}", e));
                    }
                    for tile in &state.tiles {
                        batch
                            .add(
                                tile.pos,
                                [tile.scale; 2],
                                tile.rot,
                                &tile.sprite_rect,
                                tile.color,
                            )
                            .unwrap_or_else(|e| panic!("batch.add failed: {:#}", e));
                    }
                    let mvp = glam::Mat4::orthographic_rh(0.0, w, h, 0.0, 0.0, 1.0);
                    batch.set_mvp(gfx, &mvp.transpose());
                    batch.submit_and_draw(gfx).unwrap_or_else(|e| {
                        panic!("batch.submit_and_draw failed: {:#}", e)
                    });
                    batch.clear_batch();
                }
            }
        }

        if state.red > 0.0 {
            state.red -= (1.0 * delta_time) as f32
        } else {
            state.red = 0.0
        }
        if state.blue > 0.2 {
            state.blue -= (0.8 * delta_time) as f32
        } else {
            state.blue = 0.2
        }
        if state.green > 0.1 {
            state.green -= (0.9 * delta_time) as f32
        } else {
            state.green = 0.1
        }

        gfx.present()
            .unwrap_or_else(|e| panic!("gfx::present: {:#}", e));
        self.timer.post_frame_fpsc();
    }
}