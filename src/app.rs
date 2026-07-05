use std::f64::consts::PI;

use anyhow::{Context, Result};
use winit::event::WindowEvent;
use winit::keyboard::KeyCode;
use winit::{application::ApplicationHandler, window::WindowAttributes};

use crate::app::graphic::d3d11::test_sprite::TestSpriteRender;
use crate::app::graphic::d3d11::test_triangle::TestTriangleRender;
use crate::app::graphic::d3d11::D3D11;

mod key_state;
mod keyboard_input;
mod mouse_input;

mod graphic;
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

#[derive(Default)]
struct State {
    red: f32,
    blue: f32,
    triangle_render: Option<TestTriangleRender>,
    sprite: Option<TestSpriteRender>,
    rot: f64,
    auto_rot: bool,
}

impl State {
    fn new(gfx: &D3D11) -> Result<Self> {
        let tri_render = TestTriangleRender::new(&gfx.device)?;

        // Load texture from seth.png
        let img = image::open("seth.png")
            .context("failed to load seth.png")?
            .into_rgba8();
        let (tex_w, tex_h) = img.dimensions();

        let sprite = TestSpriteRender::new(
            &gfx.device,
            &img.into_raw(),
            tex_w,
            tex_h,
        )?;

        Ok(Self {
            red: 0.0,
            blue: 0.5,
            triangle_render: Some(tri_render),
            sprite: Some(sprite),
            auto_rot: true,
            ..Default::default()
        })
    }
}

impl App {
    fn on_init(&mut self) -> Result<()> {
        let gfx = self.gfx.as_ref().context("App not initialised")?;
        self.state = Some(State::new(gfx)?);
        Ok(())
    }
    fn on_frame(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = self.window.as_ref().unwrap();
        let gfx = self.gfx.as_ref().unwrap();
        let state = self.state.as_mut().unwrap();

        let delta_time = self.timer.pre_frame_and_get_delta_time();

        if self
            .keyboard_input
            .get_key_state(KeyCode::KeyW)
            .is_pressed()
        {
            state.red = 1.0_f32
        }
        if self
            .keyboard_input
            .get_key_state(KeyCode::KeyR)
            .is_pressed()
        {
            state.blue = 1.0_f32
        }

        if self
            .keyboard_input
            .get_key_state(KeyCode::ArrowLeft)
            .is_pressed()
        {
            state.rot -= 2.0 * PI * delta_time;
        }
        if self
            .keyboard_input
            .get_key_state(KeyCode::ArrowRight)
            .is_pressed()
        {
            state.rot += 2.0 * PI * delta_time;
        }
        if self
            .keyboard_input
            .get_key_state(KeyCode::KeyZ)
            .is_down_true_edge()
        {
            state.auto_rot = !state.auto_rot;
        }
        if state.auto_rot {
            state.rot += 0.5 * PI * delta_time;
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
            let w = self.window_size.0 as f32;
            let h = self.window_size.1 as f32;

            // ── Set states ─────────────────────────────────────────
            unsafe {
                gfx.imm_context.OMSetBlendState(&gfx.states.blend_alpha, None, 0xFFFFFFFF);
                gfx.imm_context.RSSetState(&gfx.states.rasterizer_solid_cull_none);
                gfx.imm_context.OMSetDepthStencilState(&gfx.states.depth_none, 0);
                gfx.imm_context.PSSetSamplers(0, Some(&[Some(gfx.states.sampler_linear_clamp.clone())]));
            }

            gfx.clear_screen(&[state.red, 0.1, state.blue, 1.0]);
            gfx.set_viewport(0.0, 0.0, w, h);

            // ── Draw triangle ─────────────────────────────────────
            if let Some(triangle) = state.triangle_render.as_ref() {
                triangle.draw(gfx);
            }

            // ── Draw sprite ───────────────────────────────────────
            if let Some(sprite) = state.sprite.as_ref() {
                let sw = sprite.tex_width as f32;
                let sh = sprite.tex_height as f32;

                // Orthographic projection (window coords: 0,0 = top-left)
                let mvp = glam::Mat4::orthographic_lh(0.0, w, h, 0.0, 0.0, 1.0);

                // Sprite transform: center on screen + rotate
                let angle = state.rot as f32;
                let spr = glam::Mat4::from_translation(glam::Vec3::new(w / 2.0, h / 2.0, 0.0))
                    * glam::Mat4::from_rotation_z(angle)
                    * glam::Mat4::from_scale(glam::Vec3::splat(0.5));

                sprite.draw(
                    gfx,
                    [sw / 2.0, sh / 2.0],      // origin = center of sprite
                    [sw, sh],                   // size = full texture size
                    [0.0, 0.0],                 // UV top-left
                    [sw, sh],                   // UV size = full texture
                    [1.0, 1.0, 1.0, 1.0],      // color = white
                    &mvp.transpose(),
                    &spr.transpose(),
                ).unwrap_or(());
            }
        }

        if state.red > 0.0 {
            state.red -= (1.0 * delta_time) as f32
        } else {
            state.red = 0.0
        }
        if state.blue > 0.5 {
            state.blue -= (0.5 * delta_time) as f32
        } else {
            state.blue = 0.5
        }

        gfx.present()
            .unwrap_or_else(|e| panic!("gfx::present: {:#}", e));
        self.timer.post_frame_fpsc();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(WindowAttributes::default().with_title("KrisuRJW"))
            .unwrap_or_else(|e| panic!("window::create: {:#}", e));
        self.window = Some(window);

        self.gfx = Some(
            D3D11::init_on_window(self.window.as_ref().unwrap())
                .unwrap_or_else(|e| panic!("gfx::init: {:#}", e)),
        );

        self.keyboard_input = keyboard_input::KeyboardInput::default();
        self.mouse_input = mouse_input::MouseInput::default();

        self.on_init()
            .unwrap_or_else(|e| panic!("state::init: {:#}", e));
    }
    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.on_frame(event_loop);
        self.keyboard_input.end_frame();
        self.mouse_input.end_frame();
    }
    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        self.keyboard_input.window_event(&event);
        self.mouse_input.window_event(&event);
        match event {
            WindowEvent::CloseRequested => {
                println!("[Event] Window close requested, exiting...");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.window_size = (size.width, size.height);
                self.gfx
                    .as_mut()
                    .unwrap()
                    .on_resize(size.width, size.height)
                    .unwrap_or_else(|e| panic!("gfx::resize: {:#}", e));
            }
            WindowEvent::Moved(pos) => {
                self.window_pos = (pos.x, pos.y);
            }
            _ => {}
        }
    }
    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.mouse_input.device_event(&event);
    }
}