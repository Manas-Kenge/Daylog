pub mod gnome;
pub mod install;
pub mod lifecycle;
pub mod systemd;
pub mod xdg_autostart;

pub use gnome::ExtensionStatus;
pub use install::{place_binaries, resolve_bin_dir, BinDir, InstallError};
pub use lifecycle::{
    detect, install_supervisor, pause, resume, status, stop, uninstall, wait_until_live,
    LifecycleError, Supervisor, TrackerStatus,
};

use std::path::{Path, PathBuf};

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

/// `~/.config` (or `$XDG_CONFIG_HOME` if set). Shared by systemd-user-unit
/// installation and XDG-autostart entry installation.
pub(crate) fn config_dir() -> Result<PathBuf, LifecycleError> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or_else(|| LifecycleError::Io("could not resolve $XDG_CONFIG_HOME or $HOME".into()))
}

/// Read a `services/<tmpl>` resource, substitute `{BIN_DIR}`, write to `dest`.
pub(crate) fn render_template(
    app: &AppHandle,
    tmpl: &str,
    dest: &Path,
    bin_dir: &Path,
) -> Result<(), LifecycleError> {
    let src = app
        .path()
        .resolve(format!("services/{tmpl}"), BaseDirectory::Resource)
        .map_err(|e| LifecycleError::Io(format!("resolve {tmpl}: {e}")))?;
    let raw = std::fs::read_to_string(&src)
        .map_err(|e| LifecycleError::Io(format!("read {}: {e}", src.display())))?;
    let rendered = raw.replace("{BIN_DIR}", &bin_dir.display().to_string());
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| LifecycleError::Io(format!("mkdir {}: {e}", parent.display())))?;
    }
    std::fs::write(dest, rendered)
        .map_err(|e| LifecycleError::Io(format!("write {}: {e}", dest.display())))?;
    Ok(())
}
