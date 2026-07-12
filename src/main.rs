//! # RS260701
//!
//! A Direct3D 11 2D sprite engine built with `winit`, `glam`, and `kira`.
//! 基于 Direct3D 11 的 2D 精灵引擎。
//!
//! ## Architecture / 架构
//!
//! - `app::App` — main application state, event handling, render loop / 主应用状态、事件处理、渲染循环
//! - `app::AppContext` — GPU/audio/texture resources, created after window init / GPU/音频/纹理资源
//! - `app::sprite2d` — sprite description, typed buffer with pipeline-sorted iteration / 精灵描述、带流水线排序的缓冲区
//! - `app::transform2d` — position/scale/rotation transform / 位置/缩放/旋转变换

use winit::event_loop::ControlFlow;

mod app;

fn main() {
    println!("RS260701 by KrisuRJW");

    // Create a winit event loop that polls continuously.
    // 创建持续轮询的 winit 事件循环。
    let event_loop = winit::event_loop::EventLoop::new()
        .unwrap_or_else(|e| panic!("Failed to create event loop: {}", e));
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = app::App::default();
    event_loop
        .run_app(&mut app)
        .unwrap_or_else(|e| panic!("Failed to run event loop: {}", e));
}
