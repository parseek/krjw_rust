use super::key_state::*;
use glam::Vec2;
use winit::event::WindowEvent;

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    X1 = 3,
    X2 = 4,
}

#[derive(Default)]
pub struct MouseInput {
    mouse_position: (f64, f64),
    mouse_delta: (f64, f64),
    mouse_buttons: [KeyState; 5],  // Left, Right, Middle, X1, X2
    mouse_wheel_delta: (f64, f64), // LineDelta (x, y), accumulated
    pixel_wheel: Option<(f64, f64)>, // PixelDelta, accumulated per frame
    in_window: bool,
}

impl MouseInput {
    #[allow(unused)]
    pub fn get_mouse_position(&self) -> (f64, f64) {
        self.mouse_position
    }
    /// Returns the mouse position as a glam::Vec2 (f32) for convenience.
    #[allow(unused)]
    pub fn get_mouse_pos_vec2(&self) -> Vec2 {
        Vec2::new(self.mouse_position.0 as f32, self.mouse_position.1 as f32)
    }
    #[allow(unused)]
    pub fn get_mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }
    #[allow(unused)]
    pub fn get_mouse_button_state(&self, button_index: MouseButton) -> KeyState {
        self.mouse_buttons[button_index as usize]
    }
    #[allow(unused)]
    pub fn get_mouse_wheel_delta(&self) -> (f64, f64) {
        self.mouse_wheel_delta
    }
    #[allow(unused)]
    pub fn is_in_window(&self) -> bool {
        self.in_window
    }
    #[allow(unused)]
    pub fn get_pixel_wheel(&self) -> Option<(f64, f64)> {
        self.pixel_wheel
    }
    pub fn end_frame(&mut self) {
        for button_state in self.mouse_buttons.iter_mut() {
            *button_state = button_state.off_edge();
            if button_state.is_sudden_up()
            {
                *button_state = KEY_STATE_UP_TRUE_EDGE
            }
        }
        self.mouse_delta = (0.0, 0.0);
        self.mouse_wheel_delta = (0.0, 0.0);
        self.pixel_wheel = None;
    }

    /// Handle an AppMsg directly, bypassing winit event synthesis.
    /// 直接处理 AppMsg，绕过 winit 事件合成。
    pub fn handle_msg(&mut self, msg: &crate::msg::AppMsg) {
        use crate::msg::AppMsg;
        match msg {
            AppMsg::CursorMoved(x, y) => {
                self.mouse_position = (*x, *y);
            }
            AppMsg::CursorEntered => {
                self.in_window = true;
            }
            AppMsg::CursorLeft => {
                self.in_window = false;
            }
            AppMsg::MouseWheel(x, y) => {
                self.mouse_wheel_delta.0 += x;
                self.mouse_wheel_delta.1 += y;
            }
            AppMsg::MouseWheelPixel(x, y) => {
                let current = self.pixel_wheel.unwrap_or((0.0, 0.0));
                self.pixel_wheel = Some((current.0 + x, current.1 + y));
            }
            AppMsg::MouseInput { button, state } => {
                let button_index = *button as usize;
                if button_index >= 5 {
                    return;
                }
                let key_state = &mut self.mouse_buttons[button_index];
                let new_state = match state {
                    winit::event::ElementState::Pressed => {
                        if key_state.is_pressed() {
                            KEY_STATE_DOWN_EDGE
                        } else {
                            KEY_STATE_DOWN_TRUE_EDGE
                        }
                    }
                    winit::event::ElementState::Released => {
                        if key_state.is_released() {
                            KEY_STATE_UP_EDGE
                        } else {
                            if key_state.is_down_true_edge() {
                                key_state.sudden_up()
                            }
                            else {
                                KEY_STATE_UP_TRUE_EDGE
                            }
                        }
                    }
                };
                *key_state = new_state;
            }
            AppMsg::MouseMotion(dx, dy) => {
                self.mouse_delta.0 += dx;
                self.mouse_delta.1 += dy;
            }
            _ => {}
        }
    }
    #[allow(unused)]
    pub fn get_mouse_button_states_iter(
        &self,
    ) -> impl Iterator<Item = (MouseButton, KeyState)> + '_ {
        self.mouse_buttons.iter().enumerate().map(|(i, state)| {
            (
                match i {
                    0 => MouseButton::Left,
                    1 => MouseButton::Right,
                    2 => MouseButton::Middle,
                    3 => MouseButton::X1,
                    4 => MouseButton::X2,
                    _ => unreachable!(),
                },
                *state,
            )
        })
    }
    pub fn window_event(&mut self, event: &winit::event::WindowEvent) {
        match event {
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                // Fun fact: If you move the mouse from inside the window to outside the window, you will not get a CursorMoved event,
                //     but if you do so while you are holding down a mouse button, you will get a CursorMoved event. This is because
                //     the OS sends mouse move events to the window that has captured the mouse, which is usually the window that has
                //     the mouse button pressed.
                self.mouse_position = (position.x, position.y);
            }
            #[allow(unused)]
            winit::event::WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        self.mouse_wheel_delta.0 += *x as f64;
                        self.mouse_wheel_delta.1 += *y as f64;
                    }
                    // winit::event::MouseScrollDelta::PixelDelta(pos) => {
                    //     self.mouse_wheel_delta.0 += pos.x;
                    //     self.mouse_wheel_delta.1 += pos.y;
                    // }
                    _ => {}
                }
            }
            #[allow(unused)]
            winit::event::WindowEvent::CursorEntered { device_id } => {
                self.in_window = true;
            }
            #[allow(unused)]
            winit::event::WindowEvent::CursorLeft { device_id } => {
                self.in_window = false;
            }
            #[allow(unused)]
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                let button_index = match *button {
                    winit::event::MouseButton::Left => 0,
                    winit::event::MouseButton::Right => 1,
                    winit::event::MouseButton::Middle => 2,
                    winit::event::MouseButton::Back => 3,
                    winit::event::MouseButton::Forward => 4,
                    _ => return,
                };
                let button_state = &mut self.mouse_buttons[button_index];
                let new_button_state = match state {
                    winit::event::ElementState::Pressed => {
                        if button_state.is_pressed() {
                            KEY_STATE_DOWN_EDGE
                        } else {
                            KEY_STATE_DOWN_TRUE_EDGE
                        }
                    }
                    winit::event::ElementState::Released => {
                        if button_state.is_released() {
                            KEY_STATE_UP_EDGE
                        } else {
                            if button_state.is_down_true_edge() {
                                button_state.sudden_up()
                            }
                            else {
                                KEY_STATE_UP_TRUE_EDGE
                            }
                        }
                    }
                };
                *button_state = new_button_state;
            }
            _ => {}
        }
    }
    pub fn device_event(&mut self, event: &winit::event::DeviceEvent) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                // Frame delta is accumulated, and will be reset at the end of the frame.
                self.mouse_delta.0 += delta.0;
                self.mouse_delta.1 += delta.1;
            }
            // winit::event::DeviceEvent::Button { button, state } =>
            // winit::event::DeviceEvent::MouseWheel { delta } => {
            //     match delta {
            //         winit::event::MouseScrollDelta::LineDelta(x, y) => {
            //             self.mouse_wheel_delta.0 += *x as f64;
            //             self.mouse_wheel_delta.1 += *y as f64;
            //         }
            //         // winit::event::MouseScrollDelta::PixelDelta(pos) => {
            //         //     self.mouse_wheel_delta.0 += pos.x;
            //         //     self.mouse_wheel_delta.1 += pos.y;
            //         // }
            //         _ => {}
            //     }
            // }
            _ => {}
        }
    }
}
