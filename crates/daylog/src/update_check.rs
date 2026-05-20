//! Passive, low-noise "newer release available" check. Hits the GitHub
//! Releases API at most once per 24h; everything fails silent.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const CACHE_TTL: chrono::Duration = chrono::Duration::hours(24);
const HTTP_TIMEOUT: Duration = Duration::from_secs(4);
const GH_URL: &str = "https://api.github.com/repos/Manas-Kenge/Daylog/releases/latest";
const OPT_OUT_ENV: &str = "DAYLOG_NO_UPDATE_CHECK";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub latest: String,
    pub release_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    checked_at: DateTime<Utc>,
    latest_version: Option<String>,
    release_url: Option<String>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
}

/// Fire-and-forget at startup. Returns Some only if a strictly newer release exists.
pub async fn check(current: &'static str) -> Option<UpdateInfo> {
    if std::env::var_os(OPT_OUT_ENV).is_some() {
        return None;
    }

    let path = cache_path()?;
    let now = Utc::now();

    if let Some(cf) = read_cache(&path) {
        if now.signed_duration_since(cf.checked_at) < CACHE_TTL {
            return from_cache(cf, current);
        }
    }

    let fetched = fetch_latest().await;
    let cf = match &fetched {
        Some((tag, url)) => CacheFile {
            checked_at: now,
            latest_version: Some(tag.clone()),
            release_url: Some(url.clone()),
        },
        None => CacheFile {
            checked_at: now,
            latest_version: None,
            release_url: None,
        },
    };
    write_cache(&path, &cf);

    let (tag, url) = fetched?;
    is_newer(&tag, current).then(|| UpdateInfo {
        latest: tag,
        release_url: url,
    })
}

fn from_cache(cf: CacheFile, current: &str) -> Option<UpdateInfo> {
    let latest = cf.latest_version?;
    let release_url = cf.release_url?;
    is_newer(&latest, current).then(|| UpdateInfo {
        latest,
        release_url,
    })
}

async fn fetch_latest() -> Option<(String, String)> {
    let ua = concat!("daylog-tui/", env!("CARGO_PKG_VERSION"));
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(ua)
        .build()
        .ok()?;
    let resp = client
        .get(GH_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let rel: GhRelease = resp.json().await.ok()?;
    let tag = rel.tag_name.trim_start_matches('v').to_string();
    Some((tag, rel.html_url))
}

fn cache_path() -> Option<PathBuf> {
    let dir = dirs::cache_dir()?.join("daylog");
    let _ = fs::create_dir_all(&dir);
    Some(dir.join("update-check.json"))
}

fn read_cache(path: &Path) -> Option<CacheFile> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_cache(path: &Path, cf: &CacheFile) {
    if let Ok(s) = serde_json::to_string(cf) {
        let _ = fs::write(path, s);
    }
}

fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut it = s.splitn(3, '.');
    let major: u32 = it.next()?.parse().ok()?;
    let minor: u32 = it.next()?.parse().ok()?;
    let patch: u32 = it.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_accepts_x_y_z() {
        assert_eq!(parse_semver("0.2.1"), Some((0, 2, 1)));
        assert_eq!(parse_semver("v0.2.1"), Some((0, 2, 1)));
        assert_eq!(parse_semver("12.34.56"), Some((12, 34, 56)));
    }

    #[test]
    fn parse_semver_rejects_garbage() {
        assert_eq!(parse_semver(""), None);
        assert_eq!(parse_semver("0.2"), None);
        assert_eq!(parse_semver("0.2.x"), None);
        assert_eq!(parse_semver("not.a.version"), None);
    }

    #[test]
    fn is_newer_compares_numerically_not_lexically() {
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("0.3.0", "0.2.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.10.0", "0.9.9"), "lexical compare would fail this");
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
    }

    #[test]
    fn is_newer_handles_v_prefix_on_either_side() {
        assert!(is_newer("v0.2.1", "0.2.0"));
        assert!(is_newer("0.2.1", "v0.2.0"));
    }

    #[test]
    fn is_newer_returns_false_on_unparseable_input() {
        assert!(!is_newer("garbage", "0.2.0"));
        assert!(!is_newer("0.2.1", "garbage"));
    }

    fn tmp_path(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "daylog-update-check-test-{}-{}-{suffix}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn cache_round_trip() {
        let path = tmp_path("round-trip");
        let cf = CacheFile {
            checked_at: Utc::now(),
            latest_version: Some("0.2.1".into()),
            release_url: Some("https://example.test/v0.2.1".into()),
        };
        write_cache(&path, &cf);
        let read = read_cache(&path).expect("cache read");
        assert_eq!(read.latest_version.as_deref(), Some("0.2.1"));
        assert_eq!(read.release_url.as_deref(), Some("https://example.test/v0.2.1"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn failure_marker_round_trips_and_yields_no_update() {
        let path = tmp_path("failure-marker");
        let cf = CacheFile {
            checked_at: Utc::now(),
            latest_version: None,
            release_url: None,
        };
        write_cache(&path, &cf);
        let read = read_cache(&path).expect("cache read");
        assert!(read.latest_version.is_none());
        assert!(from_cache(read, "0.2.0").is_none());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn from_cache_yields_update_only_when_newer() {
        let fresh = || CacheFile {
            checked_at: Utc::now(),
            latest_version: Some("0.2.1".into()),
            release_url: Some("u".into()),
        };
        assert!(from_cache(fresh(), "0.2.0").is_some());
        assert!(from_cache(fresh(), "0.2.1").is_none());
        assert!(from_cache(fresh(), "0.3.0").is_none());
    }

    #[test]
    fn stale_cache_is_detected_by_ttl_math() {
        let old = Utc::now() - chrono::Duration::hours(25);
        let fresh_threshold = Utc::now().signed_duration_since(old);
        assert!(fresh_threshold >= CACHE_TTL);
    }
}
