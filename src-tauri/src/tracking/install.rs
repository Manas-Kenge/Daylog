use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

const BINARIES: &[&str] = &["aw-server-rust", "aw-awatcher"];
const STAMP_FILENAME: &str = ".version";

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("could not resolve XDG_DATA_HOME or $HOME")]
    NoHome,
    #[error("tauri resource resolution failed: {0}")]
    Resource(String),
    #[error("bundled binary not found at {0}")]
    BundledMissing(PathBuf),
    #[error("bundled binary directory has no parent: {0}")]
    NoParent(PathBuf),
    #[error("io: {0}")]
    Io(String),
}

impl serde::Serialize for InstallError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BinDirSource {
    /// Running inside an AppImage; binaries copied to user data dir.
    AppImageExtracted,
    /// Running from a system package (.deb / .rpm) — binaries are at a stable system path.
    SystemPackage,
    /// `bun run tauri dev` — binaries are inside src-tauri/target/debug/.
    Development,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinDir {
    pub path: PathBuf,
    pub source: BinDirSource,
    /// Daylog version that produced the binaries currently at `path`.
    /// `None` when the source is SystemPackage or Development (managed by package manager / cargo).
    pub stamped_version: Option<String>,
}

/// Inspect the current state without performing any extraction. Returns where
/// `{BIN_DIR}` is (or would be) and whether the bundled binaries are present.
pub fn resolve_bin_dir(app: &AppHandle) -> Result<BinDir, InstallError> {
    if is_appimage() {
        let user_bin = user_bin_dir()?;
        let stamp_path = user_bin.join(STAMP_FILENAME);
        let stamped_version = fs::read_to_string(&stamp_path).ok();
        Ok(BinDir {
            path: user_bin,
            source: BinDirSource::AppImageExtracted,
            stamped_version,
        })
    } else {
        let resource = resolve_resource(app, BINARIES[0])?;
        let dir = resource
            .parent()
            .ok_or_else(|| InstallError::NoParent(resource.clone()))?
            .to_path_buf();
        Ok(BinDir {
            path: dir,
            source: detect_non_appimage_source(),
            stamped_version: None,
        })
    }
}

/// Resolve `{BIN_DIR}` and ensure all bundled binaries are present + executable.
/// AppImage carrier: extracts to `~/.local/share/daylog/bin/` on first launch and
/// whenever the running Daylog version differs from the stamped one.
/// System package or dev: binaries are already in place; no extraction needed.
pub fn place_binaries(app: &AppHandle) -> Result<BinDir, InstallError> {
    if is_appimage() {
        let user_bin = user_bin_dir()?;
        ensure_extracted(app, &user_bin)?;
        Ok(BinDir {
            path: user_bin.clone(),
            source: BinDirSource::AppImageExtracted,
            stamped_version: Some(daylog_version().to_string()),
        })
    } else {
        // .deb / .rpm / dev — binaries are at stable on-disk paths placed by the
        // package manager (or `bun run tauri dev`). Just verify they exist.
        let dir = resolve_bin_dir(app)?.path;
        for name in BINARIES {
            let p = dir.join(name);
            if !p.is_file() {
                return Err(InstallError::BundledMissing(p));
            }
        }
        Ok(BinDir {
            path: dir,
            source: detect_non_appimage_source(),
            stamped_version: None,
        })
    }
}

fn is_appimage() -> bool {
    // AppImage runtime sets both vars; presence of either is sufficient.
    std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some()
}

fn detect_non_appimage_source() -> BinDirSource {
    if cfg!(debug_assertions) {
        BinDirSource::Development
    } else {
        BinDirSource::SystemPackage
    }
}

fn daylog_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn user_bin_dir() -> Result<PathBuf, InstallError> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .ok_or(InstallError::NoHome)?;
    Ok(base.join("daylog").join("bin"))
}

fn resolve_resource(app: &AppHandle, name: &str) -> Result<PathBuf, InstallError> {
    let p = app
        .path()
        .resolve(format!("binaries/{name}"), BaseDirectory::Resource)
        .map_err(|e| InstallError::Resource(e.to_string()))?;
    if !p.exists() {
        return Err(InstallError::BundledMissing(p));
    }
    Ok(p)
}

fn ensure_extracted(app: &AppHandle, dest: &Path) -> Result<(), InstallError> {
    fs::create_dir_all(dest)?;

    let stamp = dest.join(STAMP_FILENAME);
    let want = daylog_version();
    let have = fs::read_to_string(&stamp).unwrap_or_default();

    if have == want && all_present(dest) {
        return Ok(());
    }

    for name in BINARIES {
        let src = resolve_resource(app, name)?;
        let dst = dest.join(name);
        atomic_install(&src, &dst)?;
    }

    fs::write(&stamp, want)?;
    Ok(())
}

fn all_present(dir: &Path) -> bool {
    BINARIES.iter().all(|n| dir.join(n).is_file())
}

#[cfg(unix)]
fn atomic_install(src: &Path, dst: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    // Per-PID tmp name so two simultaneous Daylog processes don't clobber.
    let tmp = dst.with_extension(format!("tmp.{}", std::process::id()));
    fs::copy(src, &tmp)?;
    let mut perms = fs::metadata(&tmp)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tmp, perms)?;
    fs::rename(&tmp, dst)?;
    Ok(())
}

#[cfg(not(unix))]
fn atomic_install(_src: &Path, _dst: &Path) -> Result<(), InstallError> {
    compile_error!("Daylog is Linux-only for v0.1");
}
