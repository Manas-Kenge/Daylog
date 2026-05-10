use std::path::Path;

use tokio::process::Command;

use crate::tracking::lifecycle::{LifecycleError, UnitState};
use crate::tracking::{config_dir, render_template, AUTOSTART_TEMPLATE, SUPERVISOR_TEMPLATE};

const AUTOSTART_FILE: &str = "daylog-tracker.desktop";
const SUPERVISOR_FILE: &str = "daylog-supervisor.sh";

pub async fn install(bin_dir: &Path) -> Result<(), LifecycleError> {
    // 1. Drop the supervisor script into bin_dir, executable.
    let supervisor = bin_dir.join(SUPERVISOR_FILE);
    render_template(SUPERVISOR_TEMPLATE, &supervisor, bin_dir)?;
    chmod_exec(&supervisor)?;

    // 2. Drop the autostart .desktop entry so the user picks up tracking
    //    after their next login automatically.
    let autostart_dir = config_dir()?.join("autostart");
    std::fs::create_dir_all(&autostart_dir)
        .map_err(|e| LifecycleError::Io(format!("mkdir {}: {e}", autostart_dir.display())))?;
    render_template(
        AUTOSTART_TEMPLATE,
        &autostart_dir.join(AUTOSTART_FILE),
        bin_dir,
    )?;

    // 3. Start it now so the user doesn't have to log out/in to begin tracking.
    start(bin_dir).await?;
    Ok(())
}

pub async fn start(bin_dir: &Path) -> Result<(), LifecycleError> {
    if is_running().await {
        return Ok(());
    }
    let supervisor = bin_dir.join(SUPERVISOR_FILE);
    Command::new(&supervisor)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            LifecycleError::XdgAutostart(format!(
                "spawn supervisor at {}: {e}",
                supervisor.display()
            ))
        })?;
    // Detached: don't await the child. The supervisor outlives this call.
    Ok(())
}

pub async fn stop() -> Result<(), LifecycleError> {
    // pkill is on every Linux distro that ships procps. Best-effort — if the
    // supervisor isn't running, exit 1 is fine; we silence it.
    let _ = Command::new("pkill")
        .args(["-TERM", "-f", SUPERVISOR_FILE])
        .output()
        .await;
    // The supervisor's `trap 'kill 0' EXIT` propagates the kill to the child
    // binaries, so we don't need to kill them individually.
    Ok(())
}

pub async fn uninstall() -> Result<(), LifecycleError> {
    stop().await?;
    let autostart_path = config_dir()?.join("autostart").join(AUTOSTART_FILE);
    let _ = std::fs::remove_file(&autostart_path);
    Ok(())
}

pub async fn status() -> (UnitState, UnitState) {
    let aw = pgrep("aw-server-rust").await;
    let watcher = pgrep("aw-awatcher").await;
    (state_of(aw), state_of(watcher))
}

async fn is_running() -> bool {
    pgrep(SUPERVISOR_FILE).await
}

async fn pgrep(name: &str) -> bool {
    Command::new("pgrep")
        .args(["-f", name])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn state_of(running: bool) -> UnitState {
    if running {
        UnitState::Active
    } else {
        UnitState::Inactive
    }
}

#[cfg(unix)]
fn chmod_exec(path: &Path) -> Result<(), LifecycleError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| LifecycleError::Io(format!("stat {}: {e}", path.display())))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
        .map_err(|e| LifecycleError::Io(format!("chmod {}: {e}", path.display())))?;
    Ok(())
}
