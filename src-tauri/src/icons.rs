//! Resolve `aw-watcher-window` app names (X11 WM_CLASS / Wayland app_id) to
//! `data:` URLs by scanning XDG application dirs and the icon-theme cascade.
//!
//! Match heuristic, weakest-first so strongest wins on collision: `Name=` →
//! `.desktop` filename → `StartupWMClass=`. Lookups are cached per process;
//! the .desktop index is built lazily on the first miss.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use freedesktop_entry_parser::parse_entry;
use freedesktop_icons::lookup;

#[derive(Default)]
struct Index {
    /// `lowercased_key → icon_name` (from .desktop `Icon=` field).
    by_key: HashMap<String, String>,
    /// `first_token → all icon_names whose first-token derivation hit it`.
    /// Used as a fallback when exact match misses — the lookup path only
    /// consults this when the token is unique (len == 1) so we don't
    /// over-match (e.g. multiple `google-*` apps fighting for "google").
    by_first_token: HashMap<String, Vec<String>>,
}

#[derive(Default)]
struct Cache {
    /// Built lazily on first call. `None` until ready.
    index: Option<Index>,
    /// Per-app-name resolved data URL. `None` value = "tried, no match" so
    /// repeated misses don't re-scan the index.
    resolved: HashMap<String, Option<String>>,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

fn xdg_application_dirs() -> Vec<PathBuf> {
    // XDG basedir spec: $XDG_DATA_HOME (default $HOME/.local/share) +
    // $XDG_DATA_DIRS (default /usr/local/share:/usr/share). Plus the
    // Flatpak/Snap exports that aren't always in DATA_DIRS but are
    // ubiquitous enough to be worth checking unconditionally.
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        let q = p.join("applications");
        if !dirs.iter().any(|d| d == &q) {
            dirs.push(q);
        }
    };
    if let Ok(home) = std::env::var("XDG_DATA_HOME") {
        push(PathBuf::from(home));
    } else if let Ok(home) = std::env::var("HOME") {
        push(PathBuf::from(home).join(".local/share"));
    }
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in data_dirs.split(':') {
        push(PathBuf::from(d));
    }
    push(PathBuf::from("/var/lib/flatpak/exports/share"));
    if let Ok(home) = std::env::var("HOME") {
        push(PathBuf::from(home).join(".local/share/flatpak/exports/share"));
    }
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));
    dirs
}

/// Lowercase, then return the first token split on common separators.
/// Returns `None` if the token equals the input (no useful split — exact
/// match would already cover it).
fn first_token(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    let token = lower
        .split(|c: char| c == '_' || c == '-' || c == '.' || c == ' ')
        .next()?;
    if token.is_empty() || token == lower {
        None
    } else {
        Some(token.to_string())
    }
}

fn build_index() -> Index {
    let mut by_key: HashMap<String, String> = HashMap::new();
    let mut by_first_token: HashMap<String, Vec<String>> = HashMap::new();

    for dir in xdg_application_dirs() {
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for ent in read.flatten() {
            let path = ent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }
            let parsed = match parse_entry(&path) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let section = parsed.section("Desktop Entry");
            let icon = match section.attr("Icon") {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };

            // Insert weakest match first so stronger keys overwrite on
            // collision. `entry().or_insert` for Name= so a later same-name
            // app doesn't clobber an earlier one with a real WM_CLASS hit.
            if let Some(name) = section.attr("Name") {
                by_key
                    .entry(name.to_lowercase())
                    .or_insert_with(|| icon.clone());
            }
            let basename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            if let Some(b) = &basename {
                by_key.insert(b.to_lowercase(), icon.clone());
            }
            if let Some(wm) = section.attr("StartupWMClass") {
                by_key.insert(wm.to_lowercase(), icon.clone());
            }

            // First-token fallback. Snap names like `brave_brave.desktop`
            // never match a reported `data.app == "Brave-browser"` on
            // exact key, but their first token ("brave") does — provided
            // the token is unique across the index.
            let mut tokens: Vec<String> = Vec::new();
            if let Some(b) = &basename {
                if let Some(t) = first_token(b) {
                    tokens.push(t);
                }
            }
            if let Some(wm) = section.attr("StartupWMClass") {
                if let Some(t) = first_token(wm) {
                    tokens.push(t);
                }
            }
            tokens.sort();
            tokens.dedup();
            for t in tokens {
                by_first_token.entry(t).or_default().push(icon.clone());
            }
        }
    }
    Index {
        by_key,
        by_first_token,
    }
}

fn resolve_icon_to_data_url(icon: &str) -> Option<String> {
    let path: PathBuf = if Path::new(icon).is_absolute() {
        PathBuf::from(icon)
    } else {
        // Prefer a 32px theme variant; fall back to whatever exists at
        // any size if the theme doesn't have a 32px PNG (SVG-only themes,
        // sparse hicolor entries, etc).
        lookup(icon)
            .with_size(32)
            .find()
            .or_else(|| lookup(icon).find())?
    };
    let bytes = std::fs::read(&path).ok()?;
    let mime = match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        // XPM and other legacy formats don't render in webviews — treat
        // as miss so the frontend falls back to the letter chip.
        _ => return None,
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{};base64,{}", mime, b64))
}

pub fn resolve_many(names: &[String]) -> HashMap<String, Option<String>> {
    let mut out: HashMap<String, Option<String>> = HashMap::new();
    let mut misses: Vec<String> = Vec::new();

    {
        let cache_lock = cache().lock().expect("icon cache poisoned");
        for n in names {
            match cache_lock.resolved.get(n) {
                Some(v) => {
                    out.insert(n.clone(), v.clone());
                }
                None => misses.push(n.clone()),
            }
        }
    }

    if misses.is_empty() {
        return out;
    }

    // Build the .desktop index outside the lock — file scan is slow and
    // we don't want to block other resolve calls while it runs.
    let need_build = cache()
        .lock()
        .expect("icon cache poisoned")
        .index
        .is_none();
    if need_build {
        let idx = build_index();
        let mut c = cache().lock().expect("icon cache poisoned");
        if c.index.is_none() {
            c.index = Some(idx);
        }
    }

    // Pull the icon names we need under-lock, then resolve files outside.
    let pending: Vec<(String, Option<String>)> = {
        let c = cache().lock().expect("icon cache poisoned");
        let idx = c.index.as_ref().expect("built above");
        misses
            .iter()
            .map(|n| {
                let key = n.to_lowercase();
                // Exact match wins. Otherwise fall back to first-token
                // — but only if the token resolves to exactly one entry,
                // so ambiguous prefixes (multiple `google-*` apps) don't
                // silently pick the wrong icon.
                let hit = idx.by_key.get(&key).cloned().or_else(|| {
                    let token = first_token(n)?;
                    let bucket = idx.by_first_token.get(&token)?;
                    if bucket.len() == 1 {
                        Some(bucket[0].clone())
                    } else {
                        None
                    }
                });
                (n.clone(), hit)
            })
            .collect()
    };

    let resolutions: Vec<(String, Option<String>)> = pending
        .into_iter()
        .map(|(name, icon_name)| {
            let data = icon_name.as_deref().and_then(resolve_icon_to_data_url);
            (name, data)
        })
        .collect();

    let mut c = cache().lock().expect("icon cache poisoned");
    for (name, data) in resolutions {
        c.resolved.insert(name.clone(), data.clone());
        out.insert(name, data);
    }
    out
}
