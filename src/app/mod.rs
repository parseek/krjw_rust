use winit::event::WindowEvent;
use winit::{application::ApplicationHandler, window::WindowAttributes};

mod key_state;
mod keyboard_input;
mod mouse_input;

#[derive(Default)]
pub struct App {
    frame_counter: u64,

    window: Option<winit::window::Window>,
    keyboard_input: keyboard_input::KeyboardInput,
    mouse_input: mouse_input::MouseInput,
}

impl App {
    fn on_frame(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = self.window.as_ref().unwrap();
        window.set_title(
            format!(
                "KrisuRJW - Mouse Position: {:?} Mouse Delta: {:?} Wheel Delta: {:?} In Window: {}",
                self.mouse_input.get_mouse_position(),
                self.mouse_input.get_mouse_delta(),
                self.mouse_input.get_mouse_wheel_delta(),
                self.mouse_input.is_in_window()
            )
            .as_str(),
        );

        // Because of nothing to draw, just wait for some milliseconds.
        std::thread::sleep(std::time::Duration::from_millis(10));

        // End of frame, update the keyboard input state.
        self.keyboard_input.end_frame();
        self.mouse_input.end_frame();

        self.frame_counter += 1;
        window.request_redraw();
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
            WindowEvent::RedrawRequested => {
                self.on_frame(event_loop);
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
