mod app;

use anyhow::{Context, Result};
use krjw_engine::{EngineHandler, winit::{self, dpi::LogicalSize, event_loop::{ControlFlow, EventLoop}, window::WindowAttributes}};

fn main() -> Result<()> {
    let ev_loop = EventLoop::new().context("EventLoop::new() failed")?;
    ev_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new( WindowAttributes::default()
                    .with_title("🐠 eat 🐟")
                    .with_inner_size(winit::dpi::Size::Logical(LogicalSize {
                        width: 960.0,
                        height: 600.0,
                    }))
                    .with_transparent(true),|window, hwnd, rx| {
        let mut app = app::App::default();
        app.run(window, hwnd, rx)
    });
    ev_loop.run_app(&mut handler).context("EventLoop::run_app failed")?;
    Ok(())
}