mod app;

use krjw_engine::EngineHandler;
use krjw_engine::winit::dpi::LogicalSize;
use krjw_engine::winit::event_loop::{EventLoop, ControlFlow};
use krjw_engine::winit::window::WindowAttributes;
use krjw_engine::winit;

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new( WindowAttributes::default()
                    .with_title("KrisuRJW - Shapes")
                    .with_inner_size(winit::dpi::Size::Logical(LogicalSize {
                        width: 960.0,
                        height: 600.0,
                    }))
                    .with_transparent(true),|window, hwnd, rx| {
        let mut app = app::App::new();
        app.run(window, hwnd, rx)
    });
    event_loop.run_app(&mut handler).unwrap();
}