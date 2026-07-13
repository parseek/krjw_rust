mod app;

use krjw_engine::EngineHandler;
use winit::event_loop::ControlFlow;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new(|window, hwnd, rx| {
        let mut app = app::App::new();
        app.run(window, hwnd, rx)
    });
    event_loop.run_app(&mut handler).unwrap();
}