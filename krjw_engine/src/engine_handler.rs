//! Generic main-thread handler that forwards winit events to an application thread via MPSC.
//! 通用主线程处理器——通过 MPSC 将 winit 事件转发给应用线程。

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, WindowEvent};

use crate::msg::AppMsg;
use crate::mouse_input::MouseButton;

/// Generic main-thread handler that holds the channel sender and thread handle.
/// 通用主线程处理器——持有通道发送端和线程句柄。
///
/// Takes a closure `app_init` that receives `(Window, isize, Receiver<AppMsg>)`
/// and returns `anyhow::Result<()>`. This closure is called once on `resumed()`
/// to spawn the application thread.
/// 接受一个 `app_init` 闭包，收到 `(Window, isize, Receiver<AppMsg>)`，
/// 返回 `anyhow::Result<()>`。该闭包在 `resumed()` 时调用一次，用于启动应用线程。
pub struct EngineHandler {
    msg_queue: Option<mpsc::Sender<AppMsg>>,
    app_thread: Option<JoinHandle<()>>,
    exit_requested: bool,
    /// Closure to initialise and run the application on a dedicated thread.
    /// 在专用线程上初始化和运行应用的闭包。
    app_init: Option<Box<dyn FnOnce(winit::window::Window, isize, mpsc::Receiver<AppMsg>) -> anyhow::Result<()> + Send>>,
}

impl EngineHandler {
    /// Create a new `EngineHandler` with the given application initialiser.
    /// 用给定的应用初始化器创建 `EngineHandler`。
    pub fn new(
        app_init: impl FnOnce(winit::window::Window, isize, mpsc::Receiver<AppMsg>) -> anyhow::Result<()> + Send + 'static,
    ) -> Self {
        Self {
            msg_queue: None,
            app_thread: None,
            exit_requested: false,
            app_init: Some(Box::new(app_init)),
        }
    }
}

impl ApplicationHandler for EngineHandler {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        println!("[EngineHandler] resumed — creating window & spawning App thread");

        // 1. Create the window + extract HWND on the main thread
        use winit::dpi::LogicalSize;
        use winit::raw_window_handle::HasWindowHandle;
        use winit::window::WindowAttributes;

        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("KrisuRJW")
                    .with_inner_size(winit::dpi::Size::Logical(LogicalSize {
                        width: 960.0,
                        height: 600.0,
                    }))
                    .with_transparent(true),
            )
            .expect("window::create failed");

        let handle = window.window_handle().expect("window_handle failed");
        let hwnd = match handle.as_raw() {
            winit::raw_window_handle::RawWindowHandle::Win32(w) => w.hwnd.get() as isize,
            _ => panic!("only Win32 windows are supported"),
        };

        // 2. Create MPSC channel
        let (tx, rx) = mpsc::channel::<AppMsg>();

        // 3. Take the app_init closure and spawn the App thread
        let init = self.app_init.take()
            .expect("EngineHandler::resumed called more than once");
        let handle = thread::Builder::new()
            .name("app-thread".into())
            .spawn(move || {
                if let Err(e) = init(window, hwnd, rx) {
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
                println!("[EngineHandler] CloseRequested → sending to App thread");
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

impl Drop for EngineHandler {
    fn drop(&mut self) {
        drop(self.msg_queue.take());
        if let Some(handle) = self.app_thread.take() {
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}