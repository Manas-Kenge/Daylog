use std::fs;
use std::path::PathBuf;

use crate::tracking::download::{extract_one_from_zip, fetch_archive};
use crate::tracking::pins::{Extraction, TRACKER_BINARIES};

const STAMP_FILENAME: &str = ".version";

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("could not resolve $XDG_DATA_HOME or $HOME")]
    NoHome,
    #[error("io: {0}")]
    Io(String),
    #[error("network: {0}")]
    Network(String),
    #[error("sha256 mismatch for {name}: expected {expected}")]
    Sha256Mismatch { name: String, expected: String },
    #[error("zip: {0}")]
    Zip(String),
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct BinDir {
    pub path: PathBuf,
    pub stamped_version: Option<String>,
}

pub fn resolve_bin_dir() -> Result<BinDir, InstallError> {
    let path = user_bin_dir()?;
    let stamped_version = fs::read_to_string(path.join(STAMP_FILENAME)).ok();
    Ok(BinDir {
        path,
        stamped_version,
    })
}

pub async fn place_binaries() -> Result<BinDir, InstallError> {
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

    for pin in TRACKER_BINARIES {
        let archive = fetch_archive(pin).await?;
        match &pin.extract {
            Extraction::OneFromZip { archive_path } => {
                let dst = path.join(pin.name);
                extract_one_from_zip(&archive, archive_path, &dst)?;
            }
            Extraction::WholeZip => {
                let dst = path.join(pin.name);
                fs::copy(&archive, &dst)?;
            }
        }
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

fn all_present(dir: &std::path::Path) -> bool {
    TRACKER_BINARIES
        .iter()
        .all(|pin| dir.join(pin.name).is_file())
}
