//! Main-thread handler that forwards winit events to the App thread via MPSC.
//! 主线程处理器——通过 MPSC 将 winit 事件转发给 App 线程。

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, WindowEvent};

use super::app::msg::AppMsg;
use super::app::mouse_input::MouseButton;
use super::app::App;

/// Main-thread handler that holds the channel sender and thread handle.
/// 主线程处理器——持有通道发送端和线程句柄。
pub struct AppHandler {
    msg_queue: Option<mpsc::Sender<AppMsg>>,
    app_thread: Option<JoinHandle<()>>,
    exit_requested: bool,
}

impl Default for AppHandler {
    fn default() -> Self {
        Self {
            msg_queue: None,
            app_thread: None,
            exit_requested: false,
        }
    }
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        println!("[AppHandler] resumed — creating window & spawning App thread");

        // 1. Create the window + extract HWND on the main thread
        let (window, hwnd) = App::create_window(event_loop);

        // 2. Create MPSC channel
        let (tx, rx) = mpsc::channel::<AppMsg>();

        // 3. Spawn the App thread — moves window, hwnd and rx in
        let handle = thread::Builder::new()
            .name("app-thread".into())
            .spawn(move || {
                let mut app = App::default();
                if let Err(e) = app.run(window, hwnd, rx) {
                    eprintln!("[AppThread] fatal error: {:#}", e);
                } else {
                    println!("[AppThread] exited cleanly");
                }
            })
            .expect("Failed to spawn App thread");

        self.msg_queue = Some(tx);
        self.app_thread = Some(handle);
        self.exit_requested = false;
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        // App thread is self-driven — nothing to do here.
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let tx = match &self.msg_queue {
            Some(tx) => tx,
            None => return,
        };

        // Convert WindowEvent → AppMsg and send
        let msg = match event {
            WindowEvent::RedrawRequested => return, // ignore, App thread drives itself
            WindowEvent::CloseRequested => {
                println!("[AppHandler] CloseRequested → sending to App thread");
                self.exit_requested = true;
                let _ = tx.send(AppMsg::CloseRequested);
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(size) => AppMsg::Resized(size.width, size.height),
            WindowEvent::Moved(pos) => AppMsg::Moved(pos.x, pos.y),
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                    AppMsg::KeyboardInput {
                        key_code,
                        state: event.state,
                    }
                } else {
                    return; // skip non-physical-key events
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                AppMsg::CursorMoved(position.x, position.y)
            }
            WindowEvent::CursorEntered { .. } => AppMsg::CursorEntered,
            WindowEvent::CursorLeft { .. } => AppMsg::CursorLeft,
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => AppMsg::MouseWheel(x as f64, y as f64),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => AppMsg::MouseWheelPixel(pos.x, pos.y),
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let btn = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    winit::event::MouseButton::Back => MouseButton::X1,
                    winit::event::MouseButton::Forward => MouseButton::X2,
                    _ => return,
                };
                AppMsg::MouseInput {
                    button: btn,
                    state,
                }
            }
            _ => return,
        };

        let _ = tx.send(msg);
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let tx = match &self.msg_queue {
            Some(tx) => tx,
            None => return,
        };

        let msg = match event {
            DeviceEvent::MouseMotion { delta } => AppMsg::MouseMotion(delta.0, delta.1),
            _ => return,
        };

        let _ = tx.send(msg);
    }
}

impl Drop for AppHandler {
    fn drop(&mut self) {
        drop(self.msg_queue.take());
        if let Some(handle) = self.app_thread.take() {
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}