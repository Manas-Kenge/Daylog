use tokio::process::Command;

use crate::tracking::download::fetch_archive;
use crate::tracking::lifecycle::LifecycleError;
use crate::tracking::pins::GNOME_EXTENSION;

const EXT_UUID: &str = "focused-window-dbus@flexagoon.com";

#[derive(Debug, Clone)]
pub struct ExtensionStatus {
    pub applicable: bool,
    pub available: bool,
    pub installed: bool,
    pub enabled: bool,
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

pub fn is_gnome_wayland() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    desktop.to_uppercase().contains("GNOME") && session.eq_ignore_ascii_case("wayland")
}

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

pub async fn setup() -> Result<ExtensionStatus, LifecycleError> {
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

    let zip_path = fetch_archive(&GNOME_EXTENSION)
        .await
        .map_err(|e| LifecycleError::Io(format!("fetch GNOME extension: {e}")))?;

    let out = Command::new("gnome-extensions")
        .arg("install")
        .arg("--force")
        .arg(&zip_path)
        .output()
        .await
        .map_err(|e| LifecycleError::Io(format!("exec gnome-extensions: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(LifecycleError::Io(format!(
            "gnome-extensions install failed: {stderr}"
        )));
    }

    let out = Command::new("gnome-extensions")
        .arg("enable")
        .arg(EXT_UUID)
        .output()
        .await
        .map_err(|e| LifecycleError::Io(format!("exec gnome-extensions: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
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
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|line| line.trim() == uuid),
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
