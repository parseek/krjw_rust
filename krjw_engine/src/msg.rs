//! Messages sent from the main thread to the App thread via MPSC channel.
//! 主线程通过 MPSC 通道发送给 App 线程的消息。

use winit::event::ElementState;
use winit::keyboard::KeyCode;

use super::mouse_input::MouseButton;

/// All possible events that the main thread forwards to the App thread.
/// 主线程转发给 App 线程的所有事件类型。
///
/// Every variant uses only `Send + Copy` types so `AppMsg` itself is `Send`.
/// 所有变体仅使用 `Send + Copy` 类型，因此 `AppMsg` 自身也是 `Send`。
#[derive(Debug, Clone, Copy)]
pub enum AppMsg {
    CloseRequested,
    Resized(u32, u32),
    Moved(i32, i32),
    KeyboardInput {
        key_code: KeyCode,
        state: ElementState,
    },
    CursorMoved(f64, f64),
    CursorEntered,
    CursorLeft,
    MouseWheel(f64, f64),
    MouseWheelPixel(f64, f64),
    MouseInput {
        button: MouseButton,
        state: ElementState,
    },
    MouseMotion(f64, f64),
}