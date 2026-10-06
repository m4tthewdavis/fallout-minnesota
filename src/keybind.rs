//! Turns the key names in the settings (`sim::keys`) into real key codes,
//! and gives gameplay code one way to ask "is the Jump key down?" that
//! follows the player's bindings.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::menu::GameSettings;
use crate::sim::keys::Bind;

/// Every bindable key: its name in the settings and its Bevy key code.
/// (Same keys, same order, as `sim::keys::KEY_NAMES`.)
pub const TABLE: &[(&str, KeyCode)] = &[
    ("KeyA", KeyCode::KeyA),
    ("KeyB", KeyCode::KeyB),
    ("KeyC", KeyCode::KeyC),
    ("KeyD", KeyCode::KeyD),
    ("KeyE", KeyCode::KeyE),
    ("KeyF", KeyCode::KeyF),
    ("KeyG", KeyCode::KeyG),
    ("KeyH", KeyCode::KeyH),
    ("KeyI", KeyCode::KeyI),
    ("KeyJ", KeyCode::KeyJ),
    ("KeyK", KeyCode::KeyK),
    ("KeyL", KeyCode::KeyL),
    ("KeyM", KeyCode::KeyM),
    ("KeyN", KeyCode::KeyN),
    ("KeyO", KeyCode::KeyO),
    ("KeyP", KeyCode::KeyP),
    ("KeyQ", KeyCode::KeyQ),
    ("KeyR", KeyCode::KeyR),
    ("KeyS", KeyCode::KeyS),
    ("KeyT", KeyCode::KeyT),
    ("KeyU", KeyCode::KeyU),
    ("KeyV", KeyCode::KeyV),
    ("KeyW", KeyCode::KeyW),
    ("KeyX", KeyCode::KeyX),
    ("KeyY", KeyCode::KeyY),
    ("KeyZ", KeyCode::KeyZ),
    ("Digit0", KeyCode::Digit0),
    ("Digit1", KeyCode::Digit1),
    ("Digit2", KeyCode::Digit2),
    ("Digit3", KeyCode::Digit3),
    ("Digit4", KeyCode::Digit4),
    ("Digit5", KeyCode::Digit5),
    ("Digit6", KeyCode::Digit6),
    ("Digit7", KeyCode::Digit7),
    ("Digit8", KeyCode::Digit8),
    ("Digit9", KeyCode::Digit9),
    ("F1", KeyCode::F1),
    ("F2", KeyCode::F2),
    ("F3", KeyCode::F3),
    ("F4", KeyCode::F4),
    ("F5", KeyCode::F5),
    ("F6", KeyCode::F6),
    ("F7", KeyCode::F7),
    ("F8", KeyCode::F8),
    ("F9", KeyCode::F9),
    ("F10", KeyCode::F10),
    ("F11", KeyCode::F11),
    ("F12", KeyCode::F12),
    ("Space", KeyCode::Space),
    ("Tab", KeyCode::Tab),
    ("Enter", KeyCode::Enter),
    ("Backspace", KeyCode::Backspace),
    ("ShiftLeft", KeyCode::ShiftLeft),
    ("ShiftRight", KeyCode::ShiftRight),
    ("ControlLeft", KeyCode::ControlLeft),
    ("ControlRight", KeyCode::ControlRight),
    ("AltLeft", KeyCode::AltLeft),
    ("AltRight", KeyCode::AltRight),
    ("CapsLock", KeyCode::CapsLock),
    ("ArrowUp", KeyCode::ArrowUp),
    ("ArrowDown", KeyCode::ArrowDown),
    ("ArrowLeft", KeyCode::ArrowLeft),
    ("ArrowRight", KeyCode::ArrowRight),
    ("Backquote", KeyCode::Backquote),
    ("Minus", KeyCode::Minus),
    ("Equal", KeyCode::Equal),
    ("BracketLeft", KeyCode::BracketLeft),
    ("BracketRight", KeyCode::BracketRight),
    ("Semicolon", KeyCode::Semicolon),
    ("Quote", KeyCode::Quote),
    ("Comma", KeyCode::Comma),
    ("Period", KeyCode::Period),
    ("Slash", KeyCode::Slash),
    ("Backslash", KeyCode::Backslash),
    ("Insert", KeyCode::Insert),
    ("Delete", KeyCode::Delete),
    ("Home", KeyCode::Home),
    ("End", KeyCode::End),
    ("PageUp", KeyCode::PageUp),
    ("PageDown", KeyCode::PageDown),
    ("Numpad0", KeyCode::Numpad0),
    ("Numpad1", KeyCode::Numpad1),
    ("Numpad2", KeyCode::Numpad2),
    ("Numpad3", KeyCode::Numpad3),
    ("Numpad4", KeyCode::Numpad4),
    ("Numpad5", KeyCode::Numpad5),
    ("Numpad6", KeyCode::Numpad6),
    ("Numpad7", KeyCode::Numpad7),
    ("Numpad8", KeyCode::Numpad8),
    ("Numpad9", KeyCode::Numpad9),
];

pub fn key_code(name: &str) -> Option<KeyCode> {
    TABLE.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}

pub fn name_of(code: KeyCode) -> Option<&'static str> {
    TABLE.iter().find(|(_, k)| *k == code).map(|(n, _)| *n)
}

/// The keyboard, read through the player's key bindings.
#[derive(SystemParam)]
pub struct Controls<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    settings: Res<'w, GameSettings>,
}

impl Controls<'_> {
    fn code(&self, b: Bind) -> Option<KeyCode> {
        key_code(self.settings.0.keys.key(b))
    }

    pub fn pressed(&self, b: Bind) -> bool {
        self.code(b).is_some_and(|k| self.keys.pressed(k))
    }

    pub fn just_pressed(&self, b: Bind) -> bool {
        self.code(b).is_some_and(|k| self.keys.just_pressed(k))
    }

    /// The raw keyboard, for keys that can't be rebound (Escape, the number keys).
    pub fn raw(&self) -> &ButtonInput<KeyCode> {
        &self.keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::keys::{Bindings, KEY_NAMES};

    #[test]
    fn every_bindable_key_has_a_key_code_and_back() {
        assert_eq!(TABLE.len(), KEY_NAMES.len());
        for (i, name) in KEY_NAMES.iter().enumerate() {
            assert_eq!(TABLE[i].0, *name, "same order");
            let code = key_code(name).unwrap();
            assert_eq!(name_of(code), Some(*name));
            // The names are Bevy's own.
            assert_eq!(format!("{code:?}"), *name);
        }
        assert_eq!(key_code("Escape"), None);
        let b = Bindings::default();
        for bind in Bind::ALL {
            assert!(key_code(b.key(bind)).is_some(), "{bind:?}");
        }
    }
}
