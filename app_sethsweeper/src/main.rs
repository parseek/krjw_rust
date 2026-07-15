//! # app_sethsweeper — Application using krjw_engine
//!
//! Based on the original RS260701 application.
//! 基于原 RS260701 的应用。

mod app;

use krjw_engine::EngineHandler;
use krjw_engine::winit::dpi::LogicalSize;
use krjw_engine::winit::event_loop::{EventLoop, ControlFlow};
use krjw_engine::winit::window::WindowAttributes;
use krjw_engine::winit;

fn main() {
    println!("RS260701 by KrisuRJW");

    // Create a winit event loop that polls continuously.
    let event_loop = EventLoop::new()
        .unwrap_or_else(|e| panic!("Failed to create event loop: {}", e));
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new( WindowAttributes::default()
                    .with_title("KrisuRJW - SethSweeper")
                    .with_inner_size(winit::dpi::Size::Logical(LogicalSize {
                        width: 960.0,
                        height: 600.0,
                    }))
                    .with_transparent(true),|window, hwnd, rx| {
        let mut app = app::App::new();
        app.run(window, hwnd, rx)
    });
    event_loop
        .run_app(&mut handler)
        .unwrap_or_else(|e| panic!("Failed to run event loop: {}", e));

    println!("RS260701 exited cleanly");
}
