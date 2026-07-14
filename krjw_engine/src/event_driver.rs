//! Event driver — receives winit messages from the main thread,
//! updates input state and window state, provides accessors for the App thread.
//! 事件驱动——从主线程接收 winit 消息，
//! 更新输入状态和窗口状态，为 App 线程提供访问器。

use anyhow::Result;
use std::sync::mpsc::Receiver;

use super::keyboard_input::KeyboardInput;
use super::mouse_input::MouseInput;
use super::msg::AppMsg;

/// Frame-level event summary returned by `EventDriver::poll_frame()`.
/// `EventDriver::poll_frame()` 返回的帧事件摘要。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameEvents {
    /// `true` if a CloseRequested message was received.
    /// 如果收到 CloseRequested 消息则为 `true`。
    pub close_requested: bool,
    /// `true` if the channel has disconnected (sender dropped).
    /// 如果通道断开连接（发送端已丢弃）则为 `true`。
    pub disconnected: bool,
}

/// Receives and processes all window/input events from the main thread,
/// and exposes the resulting input state and window geometry for one frame.
/// 从主线程接收并处理所有窗口/输入事件，
/// 并暴露本帧的输入状态和窗口几何信息。
pub struct EventDriver {
    rx: Receiver<AppMsg>,
    keyboard_input: KeyboardInput,
    mouse_input: MouseInput,
    window_pos: (i32, i32),
    window_size: (u32, u32),
    window_size_dirty: bool,
}

impl EventDriver {
    /// Create a new `EventDriver` that reads from the given channel.
    /// 创建一个从给定通道读取事件的 `EventDriver`。
    pub fn new(rx: Receiver<AppMsg>) -> Self {
        Self {
            rx,
            keyboard_input: KeyboardInput::default(),
            mouse_input: MouseInput::default(),
            window_pos: (0, 0),
            window_size: (0, 0),
            window_size_dirty: false,
        }
    }

    /// Set the initial window size (called once after window creation).
    /// 设置初始窗口大小（窗口创建后调用一次）。
    pub fn set_initial_window_size(&mut self, w: u32, h: u32) {
        self.window_size = (w, h);
    }

    /// Drain all pending messages from the channel and update internal state.
    /// Returns a summary of frame-level events.
    /// 从通道中取出所有待处理消息并更新内部状态。
    /// 返回帧级别的事件摘要。
    pub fn poll_frame(&mut self) -> FrameEvents {
        let mut close_requested = false;
        let mut disconnected = false;

        loop {
            match self.rx.try_recv() {
                Ok(msg) => {
                    if !self.handle_msg(msg) {
                        close_requested = true;
                        break;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    println!("[EventDriver] channel disconnected");
                    disconnected = true;
                    break;
                }
            }
        }

        FrameEvents {
            close_requested,
            disconnected,
        }
    }

    /// Process a single message. Returns `false` on CloseRequested.
    /// 处理单条消息。收到 CloseRequested 时返回 `false`。
    fn handle_msg(&mut self, msg: AppMsg) -> bool {
        match msg {
            AppMsg::CloseRequested => {
                println!("[EventDriver] received CloseRequested");
                false
            }
            AppMsg::Resized(w, h) => {
                self.window_size = (w, h);
                self.window_size_dirty = true;
                true
            }
            AppMsg::Moved(x, y) => {
                self.window_pos = (x, y);
                true
            }
            ref other => {
                self.keyboard_input.handle_msg(other);
                self.mouse_input.handle_msg(other);
                true
            }
        }
    }

    // ── Input state accessors ──

    pub fn keyboard(&self) -> &KeyboardInput {
        &self.keyboard_input
    }

    pub fn mouse(&self) -> &MouseInput {
        &self.mouse_input
    }

    // ── Window state accessors ──

    pub fn window_pos(&self) -> (i32, i32) {
        self.window_pos
    }

    pub fn window_size(&self) -> (u32, u32) {
        self.window_size
    }

    pub fn if_window_size_dirty<F>(&mut self, mut then: F) -> Result<()>
    where F: FnMut(u32, u32) -> Result<()> {
        if self.window_size_dirty {
            then(self.window_size.0, self.window_size.1)?;
            self.window_size_dirty = false;
        }
        Ok(())
    }

    // ── Frame lifecycle ──

    /// End frame — advance edge states for keyboard and mouse.
    /// 结束帧——更新键盘和鼠标的边缘状态。
    pub fn end_frame(&mut self) {
        self.keyboard_input.end_frame();
        self.mouse_input.end_frame();
    }
}