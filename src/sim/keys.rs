//! Key bindings: which key does what, as plain data that can be saved with
//! the settings and changed in the menu. Keys are named the way Bevy names
//! them ("KeyW", "ShiftLeft"), from a fixed list of keys that can be bound;
//! `keybind.rs` turns the names into real key codes.

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};

/// Every key that can be bound, by name. (Escape is kept for the menu.)
pub const KEY_NAMES: &[&str] = &[
    "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU",
    "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ", "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "F1", "F2", "F3", "F4",
    "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "Space", "Tab", "Enter", "Backspace", "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft",
    "AltRight", "CapsLock", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Backquote", "Minus", "Equal", "BracketLeft", "BracketRight", "Semicolon", "Quote",
    "Comma", "Period", "Slash", "Backslash", "Insert", "Delete", "Home", "End", "PageUp", "PageDown", "Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4",
    "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9",
];

/// The key name as written in the list, if it's one we know.
pub fn known(name: &str) -> Option<&'static str> {
    KEY_NAMES.iter().copied().find(|k| *k == name)
}

/// How a key is shown to the player: "W", "1", "Left Shift", "Up".
pub fn display(name: &str) -> String {
    if let Some(letter) = name.strip_prefix("Key") {
        return letter.to_string();
    }
    if let Some(digit) = name.strip_prefix("Digit") {
        return digit.to_string();
    }
    if let Some(n) = name.strip_prefix("Numpad") {
        return format!("Num {n}");
    }
    if let Some(dir) = name.strip_prefix("Arrow") {
        return dir.to_string();
    }
    for (side, word) in [("Left", "Left"), ("Right", "Right")] {
        if let Some(base) = name.strip_suffix(side) {
            let base = match base {
                "Control" => "Ctrl",
                other => other,
            };
            return format!("{word} {base}");
        }
    }
    match name {
        "Backquote" => "`".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        "PageUp" => "Page Up".to_string(),
        "PageDown" => "Page Down".to_string(),
        "CapsLock" => "Caps Lock".to_string(),
        other => other.to_string(),
    }
}

/// Something you do with a key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Bind {
    Forward,
    Back,
    Left,
    Right,
    Sprint,
    Jump,
    Interact,
    TakeAll,
    Reload,
    PipBoy,
    Stimpak,
    RadAway,
    Hotdish,
    Craft,
    Workbench,
    Geiger,
}

impl Bind {
    pub const ALL: [Bind; 16] = [
        Bind::Forward,
        Bind::Back,
        Bind::Left,
        Bind::Right,
        Bind::Sprint,
        Bind::Jump,
        Bind::Interact,
        Bind::TakeAll,
        Bind::Reload,
        Bind::PipBoy,
        Bind::Stimpak,
        Bind::RadAway,
        Bind::Hotdish,
        Bind::Craft,
        Bind::Workbench,
        Bind::Geiger,
    ];
    pub const COUNT: usize = Self::ALL.len();

    fn index(self) -> usize {
        self as usize
    }

    /// The name in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            Bind::Forward => "forward",
            Bind::Back => "back",
            Bind::Left => "left",
            Bind::Right => "right",
            Bind::Sprint => "sprint",
            Bind::Jump => "jump",
            Bind::Interact => "interact",
            Bind::TakeAll => "take_all",
            Bind::Reload => "reload",
            Bind::PipBoy => "pip_boy",
            Bind::Stimpak => "stimpak",
            Bind::RadAway => "radaway",
            Bind::Hotdish => "hotdish",
            Bind::Craft => "craft",
            Bind::Workbench => "workbench",
            Bind::Geiger => "geiger",
        }
    }

    /// The name in the menu.
    pub fn label(self) -> &'static str {
        match self {
            Bind::Forward => "Move Forward",
            Bind::Back => "Move Back",
            Bind::Left => "Move Left",
            Bind::Right => "Move Right",
            Bind::Sprint => "Sprint",
            Bind::Jump => "Jump",
            Bind::Interact => "Use / Take",
            Bind::TakeAll => "Take All",
            Bind::Reload => "Reload / Unjam",
            Bind::PipBoy => "Pip-Boy",
            Bind::Stimpak => "Use Stimpak",
            Bind::RadAway => "Use RadAway",
            Bind::Hotdish => "Eat Hotdish",
            Bind::Craft => "Craft Coat",
            Bind::Workbench => "Fit Weapon Mod",
            Bind::Geiger => "Geiger Counter",
        }
    }

    pub fn default_key(self) -> &'static str {
        match self {
            Bind::Forward => "KeyW",
            Bind::Back => "KeyS",
            Bind::Left => "KeyA",
            Bind::Right => "KeyD",
            Bind::Sprint => "ShiftLeft",
            Bind::Jump => "Space",
            Bind::Interact => "KeyE",
            Bind::TakeAll => "KeyT",
            Bind::Reload => "KeyR",
            Bind::PipBoy => "Tab",
            Bind::Stimpak => "KeyH",
            Bind::RadAway => "KeyX",
            Bind::Hotdish => "KeyF",
            Bind::Craft => "KeyC",
            Bind::Workbench => "KeyB",
            Bind::Geiger => "KeyG",
        }
    }
}

/// Which key each action is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bindings([&'static str; Bind::COUNT]);

impl Default for Bindings {
    fn default() -> Self {
        Bindings(Bind::ALL.map(Bind::default_key))
    }
}

impl Bindings {
    pub fn key(&self, b: Bind) -> &'static str {
        self.0[b.index()]
    }

    /// The action on `key`, if any.
    pub fn action_on(&self, key: &str) -> Option<Bind> {
        Bind::ALL.into_iter().find(|b| self.key(*b) == key)
    }

    /// Put `bind` on `key`. If another action already had that key, the two
    /// swap, so no key ever does two things and no action is left without one.
    /// Unknown keys are refused. Returns true if anything changed.
    pub fn assign(&mut self, bind: Bind, key: &str) -> bool {
        let Some(key) = known(key) else { return false };
        let old = self.key(bind);
        if old == key {
            return false;
        }
        if let Some(other) = self.action_on(key) {
            self.0[other.index()] = old;
        }
        self.0[bind.index()] = key;
        true
    }

    pub fn reset(&mut self) {
        *self = Bindings::default();
    }
}

impl Serialize for Bindings {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(Bind::COUNT))?;
        for b in Bind::ALL {
            map.serialize_entry(b.id(), self.key(b))?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Bindings {
    /// Starts from the defaults and applies what the file says, one action at
    /// a time (so clashes swap rather than duplicate); anything unknown is ignored.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Bindings, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Bindings;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map of actions to key names")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Bindings, A::Error> {
                let mut b = Bindings::default();
                while let Some((action, key)) = map.next_entry::<String, serde_json::Value>()? {
                    let (Some(bind), Some(key)) = (Bind::ALL.into_iter().find(|b| b.id() == action), key.as_str()) else { continue };
                    b.assign(bind, key);
                }
                Ok(b)
            }
        }
        d.deserialize_map(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_usual_keys_and_never_clash() {
        let b = Bindings::default();
        assert_eq!(b.key(Bind::Forward), "KeyW");
        assert_eq!(b.key(Bind::PipBoy), "Tab");
        for x in Bind::ALL {
            assert!(known(b.key(x)).is_some(), "{x:?} is on an unknown key");
            for y in Bind::ALL {
                if x != y {
                    assert_ne!(b.key(x), b.key(y), "{x:?} and {y:?} share a key");
                }
            }
        }
    }

    #[test]
    fn rebinding_onto_a_used_key_swaps_the_two() {
        let mut b = Bindings::default();
        assert!(b.assign(Bind::Jump, "KeyE"));
        assert_eq!(b.key(Bind::Jump), "KeyE");
        assert_eq!(b.key(Bind::Interact), "Space", "Use took Jump's old key");
        assert!(!b.assign(Bind::Jump, "KeyE"), "no change");
        assert!(!b.assign(Bind::Jump, "Escape"), "Escape stays the menu's");
        assert!(!b.assign(Bind::Jump, "NotAKey"));
        assert!(b.assign(Bind::Reload, "KeyZ"));
        assert_eq!(b.action_on("KeyR"), None, "R is free now");
        b.reset();
        assert_eq!(b, Bindings::default());
    }

    #[test]
    fn bindings_survive_a_round_trip_and_bad_files_fall_back() {
        let mut b = Bindings::default();
        b.assign(Bind::Forward, "ArrowUp");
        b.assign(Bind::Sprint, "ControlLeft");
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("\"forward\":\"ArrowUp\""), "{json}");
        let back: Bindings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
        // Junk is ignored, missing actions keep their defaults, clashes swap.
        let odd: Bindings = serde_json::from_str(r#"{ "forward": "Nope", "jump": 7, "reload": "KeyW", "flying": "KeyQ" }"#).unwrap();
        assert_eq!(odd.key(Bind::Forward), "KeyR", "R's old action moved over when Reload took W");
        assert_eq!(odd.key(Bind::Reload), "KeyW");
        assert_eq!(odd.key(Bind::Jump), "Space");
    }

    #[test]
    fn keys_have_friendly_names() {
        assert_eq!(display("KeyW"), "W");
        assert_eq!(display("Digit4"), "4");
        assert_eq!(display("ShiftLeft"), "Left Shift");
        assert_eq!(display("ControlRight"), "Right Ctrl");
        assert_eq!(display("ArrowUp"), "Up");
        assert_eq!(display("Space"), "Space");
        assert_eq!(display("Numpad7"), "Num 7");
        assert_eq!(display("Backquote"), "`");
    }

    #[test]
    fn the_key_list_has_no_repeats_and_no_escape() {
        let mut names: Vec<&str> = KEY_NAMES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), KEY_NAMES.len());
        assert!(known("Escape").is_none());
    }
}
