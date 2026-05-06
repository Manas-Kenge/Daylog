use serde::Serialize;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};
use tokio::process::Command;

use crate::tracking::lifecycle::LifecycleError;

const EXT_UUID: &str = "focused-window-dbus@flexagoon.com";
const EXT_RESOURCE: &str = "extensions/focused-window-dbus@flexagoon.com.zip";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExtensionStatus {
    /// True iff this is a GNOME-Wayland session (the only case that needs the
    /// extension). aw-awatcher handles X11, KDE-Wayland, and wlroots-Wayland
    /// natively without it.
    pub applicable: bool,
    /// True iff `gnome-extensions` is on `$PATH`.
    pub available: bool,
    pub installed: bool,
    pub enabled: bool,
    /// Set to true after a successful install/enable on GNOME-Wayland — the
    /// frontend should prompt for a logout/login since Wayland can't live-reload.
    pub needs_relogin: bool,
}

impl ExtensionStatus {
    fn not_applicable() -> Self {
        Self {
            applicable: false,
            available: false,
            installed: false,
            enabled: false,
            needs_relogin: false,
        }
    }
}

/// True iff this is a GNOME session running on Wayland.
pub fn is_gnome_wayland() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    desktop.to_uppercase().contains("GNOME") && session.eq_ignore_ascii_case("wayland")
}

/// Probe state without making changes. Safe on any DE.
pub async fn status() -> ExtensionStatus {
    if !is_gnome_wayland() {
        return ExtensionStatus::not_applicable();
    }
    let available = which("gnome-extensions").await;
    if !available {
        return ExtensionStatus {
            applicable: true,
            available: false,
            installed: false,
            enabled: false,
            needs_relogin: false,
        };
    }
    let installed = ext_present(EXT_UUID).await;
    let enabled = ext_enabled(EXT_UUID).await;
    ExtensionStatus {
        applicable: true,
        available: true,
        installed,
        enabled,
        needs_relogin: false,
    }
}

/// Install + enable the extension if we're on GNOME-Wayland and the host has
/// `gnome-extensions`. No-op (returns `applicable: false`) on every other DE.
pub async fn setup(app: &AppHandle) -> Result<ExtensionStatus, LifecycleError> {
    if !is_gnome_wayland() {
        return Ok(ExtensionStatus::not_applicable());
    }
    if !which("gnome-extensions").await {
        return Ok(ExtensionStatus {
            applicable: true,
            available: false,
            installed: false,
            enabled: false,
            needs_relogin: false,
        });
    }

    let zip = app
        .path()
        .resolve(EXT_RESOURCE, BaseDirectory::Resource)
        .map_err(|e| LifecycleError::Io(format!("resolve {EXT_RESOURCE}: {e}")))?;

    // `gnome-extensions install --force <pack>` is the official path: handles
    // versioning, places under ~/.local/share/gnome-shell/extensions/, and
    // overwrites cleanly on upgrade. No need to unzip ourselves.
    let out = Command::new("gnome-extensions")
        .arg("install")
        .arg("--force")
        .arg(&zip)
        .output()
        .await
        .map_err(|e| LifecycleError::Io(format!("exec gnome-extensions: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(LifecycleError::Io(format!(
            "gnome-extensions install failed: {stderr}"
        )));
    }

    // Enable. Already-enabled is a no-op.
    let out = Command::new("gnome-extensions")
        .arg("enable")
        .arg(EXT_UUID)
        .output()
        .await
        .map_err(|e| LifecycleError::Io(format!("exec gnome-extensions: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        // "already enabled" is fine; other errors propagate.
        if !stderr.contains("already") {
            return Err(LifecycleError::Io(format!(
                "gnome-extensions enable failed: {stderr}"
            )));
        }
    }

    Ok(ExtensionStatus {
        applicable: true,
        available: true,
        installed: true,
        enabled: ext_enabled(EXT_UUID).await,
        needs_relogin: true,
    })
}

async fn ext_present(uuid: &str) -> bool {
    Command::new("gnome-extensions")
        .args(["info", uuid])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn ext_enabled(uuid: &str) -> bool {
    let out = Command::new("gnome-extensions")
        .args(["list", "--enabled"])
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|line| line.trim() == uuid)
        }
        _ => false,
    }
}

async fn which(prog: &str) -> bool {
    Command::new("which")
        .arg(prog)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}
