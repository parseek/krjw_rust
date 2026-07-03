use winit::event::WindowEvent;
use winit::keyboard::PhysicalKey::Code;

use super::key_state::*;

#[derive(Default)]
pub struct KeyboardInput {
    // Use raw key codes.
    key_map: std::collections::HashMap<winit::keyboard::KeyCode, KeyState>,
}

impl KeyboardInput {
    pub fn get_key_state(&self, key_code: winit::keyboard::KeyCode) -> KeyState {
        *self.key_map.get(&key_code).unwrap_or(&KEY_STATE_RELEASED)
    }
    pub fn get_keys_iter(&self) -> impl Iterator<Item = (winit::keyboard::KeyCode, KeyState)> + '_ {
        self.key_map.iter().map(|(k, v)| (*k, *v))
    }
    pub fn end_frame(&mut self) {
        for key_state in self.key_map.values_mut() {
            // turn off the edge bit, but keep the pressed bit.
            *key_state = key_state.off_edge();
        }
    }
    pub fn window_event(&mut self, event: &winit::event::WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                if let Code(key_code) = event.physical_key {
                    let key_state = self.key_map.entry(key_code).or_insert(KEY_STATE_RELEASED);
                    let new_key_state = match event.state {
                        winit::event::ElementState::Pressed => {
                            if key_state.is_pressed() {
                                KEY_STATE_DOWN_EDGE // false edge, because it was already released.
                            } else {
                                KEY_STATE_DOWN_TRUE_EDGE // true edge, because it was pressed before.
                            }
                        }
                        winit::event::ElementState::Released => {
                            if key_state.is_released() {
                                KEY_STATE_UP_EDGE // false edge, because it was already pressed.
                            } else {
                                KEY_STATE_UP_TRUE_EDGE // true edge, because it was not pressed before.
                            }
                        }
                    };
                    *key_state = new_key_state;
                }
            }
            _ => {}
        }
    }
}
