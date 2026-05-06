use std::path::Path;

use tauri::AppHandle;
use tokio::process::Command;

use crate::tracking::lifecycle::{LifecycleError, UnitState};
use crate::tracking::{config_dir, render_template};

pub const SERVER_UNIT: &str = "daylog-aw-server.service";
pub const WATCHER_UNIT: &str = "daylog-awatcher.service";

pub async fn install(app: &AppHandle, bin_dir: &Path) -> Result<(), LifecycleError> {
    let unit_dir = config_dir()?.join("systemd").join("user");
    std::fs::create_dir_all(&unit_dir)
        .map_err(|e| LifecycleError::Io(format!("mkdir {}: {e}", unit_dir.display())))?;

    render_template(
        app,
        "daylog-aw-server.service.tmpl",
        &unit_dir.join(SERVER_UNIT),
        bin_dir,
    )?;
    render_template(
        app,
        "daylog-awatcher.service.tmpl",
        &unit_dir.join(WATCHER_UNIT),
        bin_dir,
    )?;

    run("systemctl", &["--user", "daemon-reload"]).await?;
    run(
        "systemctl",
        &["--user", "enable", "--now", SERVER_UNIT, WATCHER_UNIT],
    )
    .await?;
    Ok(())
}

pub async fn is_active(unit: &str) -> Result<UnitState, LifecycleError> {
    // `is-active` exits 0 when active and non-zero otherwise, but always prints
    // the state on stdout. We parse stdout, ignoring exit code.
    let out = Command::new("systemctl")
        .args(["--user", "is-active", unit])
        .output()
        .await?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(match stdout.as_str() {
        "active" => UnitState::Active,
        "inactive" => UnitState::Inactive,
        "failed" => UnitState::Failed,
        _ => UnitState::Unknown,
    })
}

pub async fn daemon_reload() -> Result<(), LifecycleError> {
    run("systemctl", &["--user", "daemon-reload"]).await
}

pub async fn stop_watcher() -> Result<(), LifecycleError> {
    run("systemctl", &["--user", "stop", WATCHER_UNIT]).await
}

pub async fn start_watcher() -> Result<(), LifecycleError> {
    run("systemctl", &["--user", "start", WATCHER_UNIT]).await
}

pub async fn disable_all() -> Result<(), LifecycleError> {
    // Best-effort: ignore errors so a partial install can still be torn down.
    let _ = run("systemctl", &["--user", "stop", SERVER_UNIT, WATCHER_UNIT]).await;
    let _ = run(
        "systemctl",
        &["--user", "disable", SERVER_UNIT, WATCHER_UNIT],
    )
    .await;
    Ok(())
}

async fn run(prog: &str, args: &[&str]) -> Result<(), LifecycleError> {
    let out = Command::new(prog).args(args).output().await.map_err(|e| {
        LifecycleError::Systemd(format!("could not exec `{prog}`: {e}"))
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(LifecycleError::Systemd(format!(
            "`{prog} {}` failed: {stderr}",
            args.join(" ")
        )));
    }
    Ok(())
}
