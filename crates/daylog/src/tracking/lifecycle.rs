use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::data::aw_client::AwClient;

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

impl From<std::io::Error> for LifecycleError {
    fn from(e: std::io::Error) -> Self {
        LifecycleError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supervisor {
    Systemd,
    XdgAutostart,
    /// Pre-existing aw-server we don't manage.
    #[allow(dead_code)]
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitState {
    Active,
    Inactive,
    Failed,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct TrackerStatus {
    pub supervisor: Supervisor,
    pub server: UnitState,
    pub watcher: UnitState,
}

pub fn detect() -> Supervisor {
    if Path::new("/run/systemd/system").exists() {
        Supervisor::Systemd
    } else {
        Supervisor::XdgAutostart
    }
}

pub async fn install_supervisor(bin_dir: &BinDir) -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::install(&bin_dir.path).await,
        Supervisor::XdgAutostart => xdg_autostart::install(&bin_dir.path).await,
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

pub async fn pause() -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::stop_watcher().await,
        Supervisor::XdgAutostart => xdg_autostart::stop().await,
        Supervisor::External => Ok(()),
    }
}

pub async fn resume(bin_dir: &BinDir) -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::start_watcher().await,
        Supervisor::XdgAutostart => xdg_autostart::start(&bin_dir.path).await,
        Supervisor::External => Ok(()),
    }
}

pub async fn stop() -> Result<(), LifecycleError> {
    match detect() {
        Supervisor::Systemd => systemd::disable_all().await,
        Supervisor::XdgAutostart => xdg_autostart::uninstall().await,
        Supervisor::External => Ok(()),
    }
}

pub async fn uninstall() -> Result<(), LifecycleError> {
    let _ = stop().await;

    let cfg = config_dir()?;
    let _ = std::fs::remove_file(cfg.join("systemd").join("user").join(systemd::SERVER_UNIT));
    let _ = std::fs::remove_file(cfg.join("systemd").join("user").join(systemd::WATCHER_UNIT));
    let _ = std::fs::remove_file(cfg.join("autostart").join("daylog-tracker.desktop"));

    if Path::new("/run/systemd/system").exists() {
        let _ = systemd::daemon_reload().await;
    }

    if let Some(dir) = user_bin_dir() {
        let _ = std::fs::remove_dir_all(&dir);
    }

    Ok(())
}

fn user_bin_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("daylog").join("bin"))
}

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
