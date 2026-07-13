//! # app_sethsweeper — Application using krjw_engine
//!
//! Based on the original RS260701 application.
//! 基于原 RS260701 的应用。

mod app;

use krjw_engine::EngineHandler;
use winit::event_loop::ControlFlow;

fn main() {
    println!("RS260701 by KrisuRJW");

    // Create a winit event loop that polls continuously.
    let event_loop = winit::event_loop::EventLoop::new()
        .unwrap_or_else(|e| panic!("Failed to create event loop: {}", e));
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new(|window, hwnd, rx| {
        let mut app = app::App::new();
        app.run(window, hwnd, rx)
    });
    event_loop
        .run_app(&mut handler)
        .unwrap_or_else(|e| panic!("Failed to run event loop: {}", e));

    println!("RS260701 exited cleanly");
}
