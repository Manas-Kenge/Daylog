//! Place the embedded aw-server-rust and aw-awatcher binaries into the
//! user's data dir. Idempotent — re-running on an already-installed system
//! is a no-op when the stamped daylog version matches.
//!
//! The two binaries are pulled in at compile time via `include_bytes!`
//! and live in `crates/daylog/binaries/`. `scripts/fetch-binaries.sh`
//! refreshes them from upstream pinned versions in `scripts/binaries.lock`.
//!
//! Bin dir resolution prefers `$XDG_DATA_HOME/daylog/bin/`, falling back
//! to `~/.local/share/daylog/bin/`. We never write under `/usr/lib` —
//! daylog is a userspace install, no sudo required.

use std::fs;
use std::path::{Path, PathBuf};

const AW_SERVER_RUST: &[u8] = include_bytes!("../../binaries/aw-server-rust");
const AW_AWATCHER: &[u8] = include_bytes!("../../binaries/aw-awatcher");

const BINARIES: &[(&str, &[u8])] = &[
    ("aw-server-rust", AW_SERVER_RUST),
    ("aw-awatcher", AW_AWATCHER),
];

const STAMP_FILENAME: &str = ".version";

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("could not resolve $XDG_DATA_HOME or $HOME")]
    NoHome,
    #[error("io: {0}")]
    Io(String),
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct BinDir {
    pub path: PathBuf,
    /// daylog version that produced the binaries currently extracted at `path`.
    pub stamped_version: Option<String>,
}

/// Inspect the bin-dir state without performing any extraction. Returns
/// where `{BIN_DIR}` is (or would be) and what version is stamped there,
/// if anything.
pub fn resolve_bin_dir() -> Result<BinDir, InstallError> {
    let path = user_bin_dir()?;
    let stamped_version = fs::read_to_string(path.join(STAMP_FILENAME)).ok();
    Ok(BinDir {
        path,
        stamped_version,
    })
}

/// Resolve `{BIN_DIR}` and ensure the embedded binaries are extracted +
/// executable. Re-extracts when the stamped daylog version differs from
/// the running one (covers upgrades that ship newer pinned upstream
/// binaries).
pub fn place_binaries() -> Result<BinDir, InstallError> {
    let path = user_bin_dir()?;
    fs::create_dir_all(&path)?;

    let stamp = path.join(STAMP_FILENAME);
    let want = daylog_version();
    let have = fs::read_to_string(&stamp).unwrap_or_default();

    if have == want && all_present(&path) {
        return Ok(BinDir {
            path,
            stamped_version: Some(want.to_string()),
        });
    }

    for (name, bytes) in BINARIES {
        let dst = path.join(name);
        atomic_install(bytes, &dst)?;
    }
    fs::write(&stamp, want)?;
    Ok(BinDir {
        path,
        stamped_version: Some(want.to_string()),
    })
}

fn daylog_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn user_bin_dir() -> Result<PathBuf, InstallError> {
    dirs::data_dir()
        .map(|d| d.join("daylog").join("bin"))
        .ok_or(InstallError::NoHome)
}

fn all_present(dir: &Path) -> bool {
    BINARIES.iter().all(|(name, _)| dir.join(name).is_file())
}

#[cfg(unix)]
fn atomic_install(bytes: &[u8], dst: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    // Per-PID tmp name so two simultaneous daylog processes don't clobber.
    let tmp = dst.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    let mut perms = fs::metadata(&tmp)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tmp, perms)?;
    fs::rename(&tmp, dst)?;
    Ok(())
}

#[cfg(not(unix))]
fn atomic_install(_bytes: &[u8], _dst: &Path) -> Result<(), InstallError> {
    compile_error!("daylog is Linux-only");
}
