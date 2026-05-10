//! Tracker bootstrap + lifecycle for the daylog TUI.
//!
//! On first launch, daylog downloads pinned upstream binaries
//! (aw-server-rust + aw-awatcher) into `~/.cache/daylog/binaries/`,
//! sha256-verifies them, extracts to `~/.local/share/daylog/bin/`, then
//! writes either systemd-user units or an XDG-autostart supervisor
//! (depending on what the host distro supports), and starts both. On
//! GNOME-Wayland it also offers to install the upstream
//! `focused-window-dbus` shell extension that aw-watcher-window relies
//! on for window titles.
//!
//! Service templates are tiny and embedded at compile time via
//! `include_str!`. Upstream binaries are NOT bundled in the crate (would
//! blow past crates.io's 10 MB limit) — they're fetched lazily.

pub mod download;
pub mod gnome;
pub mod install;
pub mod lifecycle;
pub mod pins;
pub mod systemd;
pub mod xdg_autostart;

pub use install::{place_binaries, BinDir, InstallError};
pub use lifecycle::{
    detect, install_supervisor, pause, resume, status, stop, uninstall, wait_until_live,
    LifecycleError, Supervisor, TrackerStatus,
};

use std::path::{Path, PathBuf};

pub(crate) const SERVER_TEMPLATE: &str =
    include_str!("../../services/daylog-aw-server.service.tmpl");
pub(crate) const WATCHER_TEMPLATE: &str =
    include_str!("../../services/daylog-awatcher.service.tmpl");
pub(crate) const SUPERVISOR_TEMPLATE: &str =
    include_str!("../../services/daylog-supervisor.sh.tmpl");
pub(crate) const AUTOSTART_TEMPLATE: &str =
    include_str!("../../services/daylog-tracker.desktop.tmpl");

/// `~/.config` (or `$XDG_CONFIG_HOME` if set). Shared by systemd-user-unit
/// installation and XDG-autostart entry installation.
pub(crate) fn config_dir() -> Result<PathBuf, LifecycleError> {
    dirs::config_dir()
        .ok_or_else(|| LifecycleError::Io("could not resolve $XDG_CONFIG_HOME or $HOME".into()))
}

/// Substitute `{BIN_DIR}` in an embedded template and write to `dest`.
pub(crate) fn render_template(
    template: &str,
    dest: &Path,
    bin_dir: &Path,
) -> Result<(), LifecycleError> {
    let rendered = template.replace("{BIN_DIR}", &bin_dir.display().to_string());
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| LifecycleError::Io(format!("mkdir {}: {e}", parent.display())))?;
    }
    std::fs::write(dest, rendered)
        .map_err(|e| LifecycleError::Io(format!("write {}: {e}", dest.display())))?;
    Ok(())
}
