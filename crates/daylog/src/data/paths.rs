//! Shared filesystem paths.

use std::path::PathBuf;

const APP_IDENTIFIER: &str = "com.manas-kenge.daylog";

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("config_dir unresolvable on this platform")]
    NoConfigDir,
}

/// `~/.config/com.manas-kenge.daylog/` on Linux.
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
