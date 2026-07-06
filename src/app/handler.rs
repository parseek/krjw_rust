use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::window::WindowAttributes;

use super::App;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(WindowAttributes::default().with_title("KrisuRJW"))
            .unwrap_or_else(|e| panic!("window::create: {:#}", e));
        self.window = Some(window);

        self.gfx = Some(
            super::graphic::d3d11::D3D11::init_on_window(self.window.as_ref().unwrap())
                .unwrap_or_else(|e| panic!("gfx::init: {:#}", e)),
        );

        self.keyboard_input = super::keyboard_input::KeyboardInput::default();
        self.mouse_input = super::mouse_input::MouseInput::default();

        self.on_init().unwrap_or_else(|e| {
            panic!("App::on_init failed. Info: {:#}", e);
        });
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