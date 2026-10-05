//! Where the game keeps its files (settings and saves), and safe reading and
//! writing of them.
//!
//! Windows: `%APPDATA%\FalloutMinnesota`. Elsewhere: `$XDG_DATA_HOME` or
//! `~/.local/share`, then `fallout-minnesota`. `FMN_DATA_DIR` overrides
//! everything (for tests and screenshots).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Work out the data folder from the environment. Pure, so it can be tested.
pub fn resolve_dir(env: impl Fn(&str) -> Option<OsString>, windows: bool) -> PathBuf {
    if let Some(dir) = env("FMN_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if windows {
        if let Some(app) = env("APPDATA").filter(|d| !d.is_empty()) {
            return PathBuf::from(app).join("FalloutMinnesota");
        }
    } else if let Some(xdg) = env("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(xdg).join("fallout-minnesota");
    } else if let Some(home) = env("HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(home).join(".local").join("share").join("fallout-minnesota");
    }
    // No usable home: keep files beside the game.
    PathBuf::from("saves")
}

pub fn data_dir() -> PathBuf {
    resolve_dir(|k| std::env::var_os(k), cfg!(windows))
}

/// Read a file from the data folder; `None` if it isn't there or can't be read.
pub fn read(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name)).ok()
}

/// Write a file to the data folder without ever leaving it half-written: the
/// text goes to a temporary file first, then replaces the real one.
pub fn write(dir: &Path, name: &str, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dir.join(name))
}

/// Delete a file from the data folder (missing is fine).
#[cfg(test)]
pub fn remove(dir: &Path, name: &str) {
    let _ = std::fs::remove_file(dir.join(name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), OsString::from(v))).collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn the_override_beats_everything() {
        let e = env(&[("FMN_DATA_DIR", "/tmp/x"), ("APPDATA", "C:/a"), ("HOME", "/h")]);
        assert_eq!(resolve_dir(&e, true), PathBuf::from("/tmp/x"));
        assert_eq!(resolve_dir(&e, false), PathBuf::from("/tmp/x"));
    }

    #[test]
    fn windows_uses_appdata_and_others_use_the_xdg_data_home_then_home() {
        assert_eq!(resolve_dir(env(&[("APPDATA", "C:/Users/me/AppData/Roaming")]), true), PathBuf::from("C:/Users/me/AppData/Roaming").join("FalloutMinnesota"));
        assert_eq!(resolve_dir(env(&[("XDG_DATA_HOME", "/x/share"), ("HOME", "/h")]), false), PathBuf::from("/x/share/fallout-minnesota"));
        assert_eq!(resolve_dir(env(&[("HOME", "/home/me")]), false), PathBuf::from("/home/me/.local/share/fallout-minnesota"));
    }

    #[test]
    fn with_no_home_it_falls_back_to_a_local_folder_and_empty_values_are_ignored() {
        assert_eq!(resolve_dir(env(&[]), false), PathBuf::from("saves"));
        assert_eq!(resolve_dir(env(&[]), true), PathBuf::from("saves"));
        assert_eq!(resolve_dir(env(&[("FMN_DATA_DIR", ""), ("HOME", "")]), false), PathBuf::from("saves"));
    }

    #[test]
    fn writes_are_atomic_readable_and_replace_old_files() {
        let dir = std::env::temp_dir().join(format!("fmn-test-{}", std::process::id()));
        write(&dir, "a.json", "one").unwrap();
        assert_eq!(read(&dir, "a.json").as_deref(), Some("one"));
        write(&dir, "a.json", "two").unwrap();
        assert_eq!(read(&dir, "a.json").as_deref(), Some("two"));
        assert!(!dir.join("a.json.tmp").exists(), "no temp file left behind");
        assert_eq!(read(&dir, "missing.json"), None);
        remove(&dir, "a.json");
        remove(&dir, "a.json");
        assert_eq!(read(&dir, "a.json"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
