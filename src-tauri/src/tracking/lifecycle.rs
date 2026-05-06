use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use crate::aw_client::AwClient;
use crate::tracking::{systemd, xdg_autostart, BinDir, InstallError};

#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("systemd: {0}")]
    Systemd(String),
    #[error("xdg-autostart: {0}")]
    XdgAutostart(String),
    #[error("io: {0}")]
    Io(String),
    #[error("aw-server didn't come up within {0}s")]
    Timeout(u64),
    #[error("install: {0}")]
    Install(#[from] InstallError),
}

impl serde::Serialize for LifecycleError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for LifecycleError {
    fn from(e: std::io::Error) -> Self {
        LifecycleError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Supervisor {
    Systemd,
    XdgAutostart,
    /// Pulse is using a pre-existing aw-server we don't manage. Constructed
    /// by the 5e first-launch wizard when `tracking_detect()` finds AW
    /// already running on :5600. Not produced by `lifecycle::detect()`.
    #[allow(dead_code)]
    External,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UnitState {
    Active,
    Inactive,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackerStatus {
    pub supervisor: Supervisor,
    pub server: UnitState,
    pub watcher: UnitState,
}

/// Pick a supervisor for this machine. systemd if `/run/systemd/system`
/// exists; XDG-autostart otherwise.
pub fn detect() -> Supervisor {
    if Path::new("/run/systemd/system").exists() {
        Supervisor::Systemd
    } else {
        Supervisor::XdgAutostart
    }
}

/// Install + start the tracker. Picks systemd or XDG-autostart based on
/// `detect()`. Idempotent — re-running on an already-installed system
/// re-renders templates and restarts services.
pub async fn install_supervisor(app: &AppHandle, bin_dir: &BinDir) -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::install(app, &bin_dir.path).await,
        Supervisor::XdgAutostart => xdg_autostart::install(app, &bin_dir.path).await,
        Supervisor::External => Ok(()),
    }
}

pub async fn status(_app: &AppHandle) -> Result<TrackerStatus, LifecycleError> {
    match detect() {
        Supervisor::Systemd => {
            let server = systemd::is_active(systemd::SERVER_UNIT).await?;
            let watcher = systemd::is_active(systemd::WATCHER_UNIT).await?;
            Ok(TrackerStatus {
                supervisor: Supervisor::Systemd,
                server,
                watcher,
            })
        }
        Supervisor::XdgAutostart => {
            let (server, watcher) = xdg_autostart::status().await;
            Ok(TrackerStatus {
                supervisor: Supervisor::XdgAutostart,
                server,
                watcher,
            })
        }
        Supervisor::External => Ok(TrackerStatus {
            supervisor: Supervisor::External,
            server: UnitState::Unknown,
            watcher: UnitState::Unknown,
        }),
    }
}

/// Pause tracking. On systemd: stops the watcher only — server keeps running
/// so historical queries still work. On XDG-autostart: stops the supervisor
/// (and therefore both binaries), since selectively pausing a child of a
/// shell-loop supervisor is fragile. Documented limitation.
pub async fn pause(_app: &AppHandle) -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::stop_watcher().await,
        Supervisor::XdgAutostart => xdg_autostart::stop().await,
        Supervisor::External => Ok(()),
    }
}

pub async fn resume(_app: &AppHandle, bin_dir: &BinDir) -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::start_watcher().await,
        Supervisor::XdgAutostart => xdg_autostart::start(&bin_dir.path).await,
        Supervisor::External => Ok(()),
    }
}

/// Disable + stop everything. Used on uninstall paths and the Settings →
/// "Stop background tracking" toggle.
pub async fn stop(_app: &AppHandle) -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::disable_all().await,
        Supervisor::XdgAutostart => xdg_autostart::uninstall().await,
        Supervisor::External => Ok(()),
    }
}

/// Poll `127.0.0.1:5600/api/0/info` until it answers, or `timeout_secs` elapses.
pub async fn wait_until_live(timeout_secs: u64) -> Result<(), LifecycleError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    let client = AwClient::new();
    loop {
        if client.info().await.is_ok() {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(LifecycleError::Timeout(timeout_secs));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
