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

pub(crate) fn config_dir() -> Result<PathBuf, LifecycleError> {
    dirs::config_dir()
        .ok_or_else(|| LifecycleError::Io("could not resolve $XDG_CONFIG_HOME or $HOME".into()))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // Regression: --testing on aw-server-rust expects no value; passing one restart-loops.

    fn render(template: &str) -> String {
        template.replace("{BIN_DIR}", "/fake/bin")
    }

    #[test]
    fn server_unit_execstart_is_exact() {
        let rendered = render(SERVER_TEMPLATE);
        assert!(
            rendered.contains("ExecStart=/fake/bin/aw-server-rust --port 5600\n"),
            "server unit ExecStart drifted:\n{rendered}"
        );
        assert!(
            !rendered.contains("--testing"),
            "server unit reintroduced --testing flag:\n{rendered}"
        );
    }

    #[test]
    fn watcher_unit_execstart_is_exact() {
        let rendered = render(WATCHER_TEMPLATE);
        assert!(
            rendered.contains("ExecStart=/fake/bin/aw-awatcher\n"),
            "watcher unit ExecStart drifted:\n{rendered}"
        );
    }

    // Regression: Restart=on-failure ignores clean-exit boot races; must be Restart=always.
    #[test]
    fn both_units_restart_always() {
        for (name, tmpl) in [("server", SERVER_TEMPLATE), ("watcher", WATCHER_TEMPLATE)] {
            let rendered = render(tmpl);
            assert!(
                rendered.contains("Restart=always"),
                "{name} unit must use Restart=always to survive clean exits:\n{rendered}"
            );
            assert!(
                !rendered.contains("Restart=on-failure"),
                "{name} unit still has Restart=on-failure:\n{rendered}"
            );
        }
    }

    #[test]
    fn supervisor_script_has_no_testing_flag() {
        let rendered = render(SUPERVISOR_TEMPLATE);
        assert!(
            rendered.contains(r#"BIN_DIR="/fake/bin""#),
            "supervisor BIN_DIR assignment drifted:\n{rendered}"
        );
        assert!(
            rendered.contains(r#"'$BIN_DIR/aw-server-rust' --port 5600"#),
            "supervisor server invocation drifted:\n{rendered}"
        );
        assert!(
            !rendered.contains("--testing"),
            "supervisor reintroduced --testing flag:\n{rendered}"
        );
    }
}
