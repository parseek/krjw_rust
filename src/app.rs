use anyhow::{Context, Result};
use winit::event::WindowEvent;
use winit::keyboard::KeyCode;
use winit::{application::ApplicationHandler, window::WindowAttributes};

use crate::app::graphic::d3d11::test_triangle::TriangleRender;
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

struct State {
    red: f32,
    blue: f32,
    triangle_render: TriangleRender,
}

impl State {
    fn new(gfx: &D3D11) -> Result<Self> {
        let tri_render = TriangleRender::new(&gfx.device)?;
        Ok(Self {
            red: 0.0,
            blue: 0.5,
            triangle_render: tri_render,
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
            .is_down_edge()
        {
            state.red = 1.0_f32
        }
        if self
            .keyboard_input
            .get_key_state(KeyCode::KeyR)
            .is_down_edge()
        {
            state.blue = 1.0_f32
        }

        window.set_title(
            format!(
                "KrisuRJW - FPS: {:.2} dTime: {}",
                self.timer.get_fps(),
                delta_time
            )
            .as_str(),
        );

        if self.window_size.0 > 0 && self.window_size.1 > 0 {
            gfx.clear_screen(&[state.red, 0.1, state.blue, 1.0]);
            gfx.set_viewport(0.0, 0.0, self.window_size.0 as f32, self.window_size.1 as f32);
            state.triangle_render.draw(&gfx.imm_context, gfx.rtv());
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