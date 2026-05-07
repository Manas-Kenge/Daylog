//! Shared filesystem paths.
//!
//! Both Tauri's `app_config_dir()` and `daylog-core::paths::config_dir()`
//! must resolve to the same directory so the desktop app, the TUI, and any
//! future surface agree on where state lives. Tauri uses the bundle
//! identifier from `tauri.conf.json` (`com.manas-kenge.daylog`); we hardcode
//! the same string here. If the identifier ever changes, both places get
//! updated together.

use std::path::PathBuf;

const APP_IDENTIFIER: &str = "com.manas-kenge.daylog";

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("config_dir unresolvable on this platform")]
    NoConfigDir,
}

/// `~/.config/com.manas-kenge.daylog/` on Linux. Same path Tauri's
/// `app.path().app_config_dir()` returns.
pub fn config_dir() -> Result<PathBuf, PathError> {
    dirs::config_dir()
        .map(|p| p.join(APP_IDENTIFIER))
        .ok_or(PathError::NoConfigDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_ends_with_identifier() {
        let p = config_dir().expect("config_dir resolves on supported platforms");
        assert!(p.ends_with(APP_IDENTIFIER));
    }
}
