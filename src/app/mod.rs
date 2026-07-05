use std::time;
use winit::event::WindowEvent;
use winit::keyboard::KeyCode;
use winit::{application::ApplicationHandler, window::WindowAttributes};

use crate::app::graphic::d3d11::triangle::TriangleRender;
use crate::app::graphic::d3d11::{self, D3D11};

mod key_state;
mod keyboard_input;
mod mouse_input;

mod timer;
mod graphic;

pub struct App {
    window: Option<winit::window::Window>,

    /// Outer position
    window_pos: (i32, i32),

    /// Inner size
    window_size: (u32, u32),
    
    keyboard_input: keyboard_input::KeyboardInput,
    mouse_input: mouse_input::MouseInput,

    graphic_mod: graphic::d3d11::D3D11,

    frame_counter: u64,

    // Used to calculate the delta time.
    frame_stamp: time::Instant,

    u: User,
}

impl Default for App {
    fn default() -> Self {
        Self {
            frame_stamp: time::Instant::now(),
            window_pos: Default::default(),
            window_size: Default::default(),
            frame_counter: 0,
            window: None,
            keyboard_input: Default::default(),
            mouse_input: Default::default(),
            graphic_mod: Default::default(),
            u: Default::default(),
        }
    }
}

#[derive(Default)]
struct User {
    red: f32,
    blue: f32,
    triangle_render : Option<d3d11::triangle::TriangleRender>,
}

impl App {
    fn on_init(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.u.triangle_render = Some(TriangleRender::new(self.graphic_mod.device.as_ref().unwrap()));
    }
    fn on_frame(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        // Get Refs
        let window = self.window.as_ref().unwrap();
        let imm_context = self.graphic_mod.imm_context.as_ref().unwrap();
        let rtv = self.graphic_mod.render_target_view.as_ref().unwrap();
        let this = &mut self.u;

        // Get Delta Time
        let instant_now = time::Instant::now();
        let delta_time: f64 = (instant_now - self.frame_stamp).as_secs_f64();
        self.frame_stamp = instant_now;

        // Process
        if self
            .keyboard_input
            .get_key_state(KeyCode::KeyW)
            .is_down_edge()
        {
            this.red = 1.0_f32
        }
        if self
            .keyboard_input
            .get_key_state(KeyCode::KeyR)
            .is_down_edge()
        {
            this.blue = 1.0_f32
        }

        // Window
        window.set_title(format!("KrisuRJW - dTime: {}", delta_time).as_str());

        // Render
        self.graphic_mod
            .clear_screen(&[this.red, 0.1, this.blue, 1.0]);

        self.graphic_mod.set_viewport(0.0, 0.0, self.window_size.0 as f32, self.window_size.1 as f32);

        this.triangle_render.as_ref().unwrap().draw(imm_context, rtv);

        // Post-Render
        if this.red > 0.0 {
            this.red -= (1.0 * delta_time) as f32
        } else {
            this.red = 0.0
        }
        if this.blue > 0.5 {
            this.blue -= (0.5 * delta_time) as f32
        } else {
            this.blue = 0.5
        }

        // Present
        self.graphic_mod
            .present()
            .unwrap_or_else(|e| panic!("Failed to Present. Info: {}", e));
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.frame_counter = 0;

        let window = event_loop
            .create_window(WindowAttributes::default().with_title("KrisuRJW"))
            .unwrap_or_else(|e| panic!("Failed to create window: {}", e));
        self.window = Some(window);
        self.keyboard_input = keyboard_input::KeyboardInput::default();
        self.mouse_input = mouse_input::MouseInput::default();

        self.graphic_mod = D3D11::init_on_window(self.window.as_ref().unwrap());

        self.on_init(event_loop);
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
                self.graphic_mod.on_resize(size.width, size.height);
            }
            WindowEvent::Moved(pos) =>
            {
                self.window_pos = (pos.x, pos.y);
            }
            WindowEvent::RedrawRequested => {
                self.on_frame(event_loop);

                // End of frame, update the keyboard input state.
                self.keyboard_input.end_frame();
                self.mouse_input.end_frame();

                self.frame_counter += 1;
                self.window.as_ref().unwrap().request_redraw();
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
