use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use daylog_core::aw_client::AwClient;
use crate::tracking::{config_dir, systemd, xdg_autostart, BinDir, InstallError};

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
    /// Daylog is using a pre-existing aw-server we don't manage. Constructed
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

pub async fn status() -> Result<TrackerStatus, LifecycleError> {
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
pub async fn pause() -> Result<(), LifecycleError> {
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

/// Disable + stop everything. Used on the Settings → "Stop background tracking"
/// toggle. Leaves unit files / autostart entries / binaries in place so a
/// re-enable is one command. For full removal, see `uninstall()`.
pub async fn stop() -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::disable_all().await,
        Supervisor::XdgAutostart => xdg_autostart::uninstall().await,
        Supervisor::External => Ok(()),
    }
}

/// Full removal: stop services, delete unit files / autostart entries, delete
/// the user-extracted binaries dir. Used by `daylog --uninstall-tracking` (the
/// AppImage user's escape hatch) and the future Settings → "Uninstall tracking"
/// button. Best-effort — missing files are not errors.
///
/// We do **not** delete `~/.local/share/activitywatch/` — that's the user's
/// tracking history, not ours to remove.
pub async fn uninstall() -> Result<(), LifecycleError> {
    // 1. Stop and disable everything we can. Errors here are best-effort.
    let _ = stop().await;

    // 2. Remove our unit files / autostart entries. Missing files are fine.
    let cfg = config_dir()?;
    let _ = std::fs::remove_file(cfg.join("systemd").join("user").join(systemd::SERVER_UNIT));
    let _ = std::fs::remove_file(cfg.join("systemd").join("user").join(systemd::WATCHER_UNIT));
    let _ = std::fs::remove_file(cfg.join("autostart").join("daylog-tracker.desktop"));

    // 3. systemd needs a daemon-reload after removing unit files so it forgets them.
    //    Best-effort; user may not have systemd or the units may already be gone.
    if Path::new("/run/systemd/system").exists() {
        let _ = systemd::daemon_reload().await;
    }

    // 4. Remove the AppImage-extracted binaries dir, if any. System packages
    //    (.deb / .rpm) own their copies under /usr/lib/<bundle-id>/ and the
    //    package manager handles cleanup; nothing to do for those.
    if let Some(dir) = user_bin_dir() {
        let _ = std::fs::remove_dir_all(&dir);
    }

    Ok(())
}

/// `~/.local/share/daylog/bin/` (or under `$XDG_DATA_HOME`). Mirrors the path
/// `install::place_binaries` uses on the AppImage carrier. Returns `None` if
/// neither `XDG_DATA_HOME` nor `HOME` is set.
fn user_bin_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .map(|d| d.join("daylog").join("bin"))
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
