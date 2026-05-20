use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use crate::tracking::install::InstallError;
use crate::tracking::pins::BinaryPin;

pub(crate) fn cache_dir() -> Result<PathBuf, InstallError> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| InstallError::Io("could not resolve $XDG_CACHE_HOME or $HOME".into()))?
        .join("daylog")
        .join("binaries");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Returns the cache path; re-downloads on stale or missing sha.
pub(crate) async fn fetch_archive(pin: &BinaryPin) -> Result<PathBuf, InstallError> {
    let cache = cache_dir()?;
    let cached = cache.join(format!("{}-{}.zip", &pin.archive_sha256[..16], pin.name));

    if cached.exists() {
        match verify_sha256(&cached, pin.archive_sha256) {
            Ok(true) => return Ok(cached),
            _ => {
                let _ = fs::remove_file(&cached);
            }
        }
    }

    download(pin.url, &cached).await?;
    if !verify_sha256(&cached, pin.archive_sha256)? {
        let _ = fs::remove_file(&cached);
        return Err(InstallError::Sha256Mismatch {
            name: pin.name.to_string(),
            expected: pin.archive_sha256.to_string(),
        });
    }
    Ok(cached)
}

pub(crate) async fn download(url: &str, dest: &Path) -> Result<(), InstallError> {
    let tmp = dest.with_extension(format!("tmp.{}", std::process::id()));
    let resp = reqwest::get(url)
        .await
        .map_err(|e| InstallError::Network(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(InstallError::Network(format!(
            "GET {url}: HTTP {}",
            resp.status()
        )));
    }
    let mut file = fs::File::create(&tmp)?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| InstallError::Network(format!("stream {url}: {e}")))?;
        file.write_all(&chunk)?;
    }
    file.flush()?;
    drop(file);
    fs::rename(&tmp, dest)?;
    Ok(())
}

pub(crate) fn verify_sha256(path: &Path, expected: &str) -> Result<bool, InstallError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got = hex_lower(&hasher.finalize());
    Ok(got == expected)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub(crate) fn extract_one_from_zip(
    archive: &Path,
    member: &str,
    dest: &Path,
) -> Result<(), InstallError> {
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| InstallError::Zip(format!("open {}: {e}", archive.display())))?;
    let mut entry = zip
        .by_name(member)
        .map_err(|e| InstallError::Zip(format!("find {member} in {}: {e}", archive.display())))?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension(format!("tmp.{}", std::process::id()));
    let mut out = fs::File::create(&tmp)?;
    std::io::copy(&mut entry, &mut out)?;
    out.flush()?;
    drop(out);
    chmod_exec(&tmp)?;
    fs::rename(&tmp, dest)?;
    Ok(())
}

#[cfg(unix)]
fn chmod_exec(path: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn chmod_exec(_path: &Path) -> Result<(), InstallError> {
    compile_error!("daylog is Linux-only");
}
