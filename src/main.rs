//! # RS260701
//!
//! A Direct3D 11 2D sprite engine built with `winit`, `glam`, and `kira`.
//! 基于 Direct3D 11 的 2D 精灵引擎。
//!
//! ## Architecture / 架构
//!
//! - `app_handler::AppHandler` — main-thread handler, forwards events via MPSC / 主线程处理器，通过 MPSC 转发事件
//! - `app::App` — runs on a dedicated thread: init, frame loop, rendering / 在专用线程运行：初始化、帧循环、渲染
//! - `app::AppContext` — GPU/audio/texture resources / GPU/音频/纹理资源
//! - `app::sprite2d` — sprite description, typed buffer with pipeline-sorted iteration / 精灵描述、带流水线排序的缓冲区
//! - `app::transform2d` — position/scale/rotation transform / 位置/缩放/旋转变换

mod app;
mod app_handler;

fn main() {
    println!("RS260701 by KrisuRJW");

    // Create a winit event loop that polls continuously.
    // 创建持续轮询的 winit 事件循环。
    let event_loop = winit::event_loop::EventLoop::new()
        .unwrap_or_else(|e| panic!("Failed to create event loop: {}", e));
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut handler = app_handler::AppHandler::default();
    event_loop
        .run_app(&mut handler)
        .unwrap_or_else(|e| panic!("Failed to run event loop: {}", e));

    println!("RS260701 exited cleanly");
}