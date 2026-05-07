//! Category rules. Stored in aw-server's settings bucket under the key
//! `classes`, matching the AW WebUI's convention. Matching itself is done
//! server-side via AQL `categorize()` — this module just handles the rule
//! shape, validation, and AQL serialization.

use std::sync::OnceLock;

use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::aw_client::{AwClient, AwError};

const SETTINGS_KEY: &str = "classes";

/// Process-level cache. Categories rarely change inside a session, but
/// `aw_top_categories` and `aw_categorized_events` both call `load()`
/// per IPC call, so a cold Overview mount used to issue ~14 redundant
/// HTTP roundtrips to aw-server's settings bucket. The cache is filled
/// lazily on first read and invalidated by `save()` when this app is
/// the mutator. External edits (AW WebUI writing the same key) are not
/// observed; that's an accepted miss until categories grow live-edit
/// surface area.
fn cache() -> &'static RwLock<Option<CategoryConfig>> {
    static CACHE: OnceLock<RwLock<Option<CategoryConfig>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

#[derive(Debug, thiserror::Error)]
pub enum CategoryError {
    #[error("aw: {0}")]
    Aw(String),
    #[error("invalid regex in category {category:?}: {error}")]
    InvalidRegex { category: Vec<String>, error: String },
}

impl serde::Serialize for CategoryError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<AwError> for CategoryError {
    fn from(e: AwError) -> Self {
        CategoryError::Aw(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Rule {
    Regex {
        regex: String,
        #[serde(default)]
        ignore_case: bool,
    },
    None,
}

/// Optional decoration that lives alongside a category rule. Daylog doesn't
/// render these yet, but we round-trip them so a future companion (the v0.2
/// GNOME shell extension, or the AW WebUI editing the same settings bucket)
/// can carry color + score without our schema fighting theirs.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CategoryData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: Vec<String>,
    pub rule: Rule,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<CategoryData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CategoryConfig {
    pub categories: Vec<Category>,
}

impl CategoryConfig {
    /// Hybrid defaults: WebUI-style hierarchy + Linux-flavored editor and
    /// terminal coverage so the dashboard isn't all "Uncategorized" on a
    /// fresh install.
    pub fn defaults() -> Self {
        let r = |re: &str| Rule::Regex {
            regex: re.into(),
            ignore_case: true,
        };
        let cat = |name: &[&str], rule: Rule| Category {
            name: name.iter().map(|s| (*s).to_string()).collect(),
            rule,
            data: None,
        };
        Self {
            categories: vec![
                cat(
                    &["Work", "Programming"],
                    r("code|cursor|vscode|atom|sublime|intellij|jetbrains|webstorm|pycharm|rustrover|goland|clion|rider|android.studio|xcode|emacs|vim|neovim|nvim|zed|helix|kitty|alacritty|wezterm|ghostty|gnome-terminal|konsole|xterm|tilix|terminator|activitywatch|aw-|daylog"),
                ),
                cat(
                    &["Work", "Documents"],
                    r("libreoffice|writer|calc|impress|notion|obsidian|joplin|evernote|onenote|logseq"),
                ),
                cat(
                    &["Work", "Image"],
                    r("gimp|krita|inkscape|figma|photoshop|illustrator|affinity"),
                ),
                cat(&["Work", "3D"], r("blender|fusion 360|sketchup")),
                cat(&["Work", "Video"], r("kdenlive|davinci|premiere|after effects|obs studio")),
                cat(&["Media", "Music"], r("spotify|rhythmbox|youtube music|apple music|tidal|deezer")),
                cat(&["Media", "Video"], r("mpv|vlc|youtube|netflix|plex|jellyfin")),
                cat(&["Media", "Games"], r("steam|lutris|heroic|minecraft")),
                cat(&["Media", "Social"], r("twitter|x.com|reddit|facebook|instagram|tiktok|mastodon|bluesky|threads")),
                cat(
                    &["Comms", "IM"],
                    r("slack|discord|telegram|signal|element|riot|whatsapp|messenger"),
                ),
                cat(
                    &["Comms", "Email"],
                    r("thunderbird|geary|evolution|gmail|outlook|protonmail|fastmail"),
                ),
                cat(
                    &["Browsing"],
                    r("firefox|brave|chromium|chrome|vivaldi|librewolf|zen browser|edge"),
                ),
                cat(&["Uncategorized"], Rule::None),
            ],
        }
    }
}

/// Validate every regex compiles. Surfaces the offending category so the
/// frontend can highlight the broken row instead of "something somewhere".
pub fn validate(cfg: &CategoryConfig) -> Result<(), CategoryError> {
    for cat in &cfg.categories {
        if let Rule::Regex { regex, ignore_case } = &cat.rule {
            let mut b = RegexBuilder::new(regex);
            b.case_insensitive(*ignore_case);
            b.build().map_err(|e| CategoryError::InvalidRegex {
                category: cat.name.clone(),
                error: e.to_string(),
            })?;
        }
    }
    Ok(())
}

/// Load rules from aw-server's settings bucket. If the key is absent or
/// empty, seed defaults — best-effort persisted back to aw-server so the
/// AW WebUI sees the same rules. If the persist write fails, we still
/// return defaults in-memory so the dashboard works.
///
/// Memoized: subsequent calls in the same process return the cached
/// config until `save()` mutates it.
pub async fn load(client: &AwClient) -> Result<CategoryConfig, CategoryError> {
    if let Some(cfg) = cache().read().await.as_ref() {
        return Ok(cfg.clone());
    }
    let cfg = load_uncached(client).await?;
    *cache().write().await = Some(cfg.clone());
    Ok(cfg)
}

async fn load_uncached(client: &AwClient) -> Result<CategoryConfig, CategoryError> {
    let stored: Option<Vec<Category>> = client.get_setting(SETTINGS_KEY).await?;
    if let Some(cats) = stored {
        if !cats.is_empty() {
            return Ok(CategoryConfig { categories: cats });
        }
    }
    let cfg = CategoryConfig::defaults();
    let _ = client.set_setting(SETTINGS_KEY, &cfg.categories).await;
    Ok(cfg)
}

pub async fn save(client: &AwClient, cfg: &CategoryConfig) -> Result<(), CategoryError> {
    validate(cfg)?;
    client.set_setting(SETTINGS_KEY, &cfg.categories).await?;
    *cache().write().await = Some(cfg.clone());
    Ok(())
}

/// Serialize the rule list for embedding into AQL. Categories whose rule is
/// `none` (decoration-only parents like "Uncategorized") are filtered out —
/// matching the WebUI's `classes_for_query` getter, which drops `type:null`
/// rules before passing them to `categorize()`.
pub fn classes_to_aql(cfg: &CategoryConfig) -> String {
    let pairs: Vec<Value> = cfg
        .categories
        .iter()
        .filter(|c| !matches!(c.rule, Rule::None))
        .map(|c| serde_json::json!([c.name, c.rule]))
        .collect();
    serde_json::to_string(&pairs).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_uncategorized_sentinel() {
        let cfg = CategoryConfig::defaults();
        assert!(cfg.categories.iter().any(|c| c.name == vec!["Uncategorized"]));
    }

    #[test]
    fn invalid_regex_surfaces_error() {
        let cfg = CategoryConfig {
            categories: vec![Category {
                name: vec!["Bad".into()],
                rule: Rule::Regex {
                    regex: "(unclosed".into(),
                    ignore_case: false,
                },
                data: None,
            }],
        };
        match validate(&cfg) {
            Err(CategoryError::InvalidRegex { category, .. }) => {
                assert_eq!(category, vec!["Bad"]);
            }
            other => panic!("expected InvalidRegex, got {other:?}"),
        }
    }

    #[test]
    fn classes_to_aql_drops_none_rules() {
        let cfg = CategoryConfig {
            categories: vec![
                Category {
                    name: vec!["Work".into()],
                    rule: Rule::Regex {
                        regex: "code".into(),
                        ignore_case: true,
                    },
                    data: None,
                },
                Category {
                    name: vec!["Uncategorized".into()],
                    rule: Rule::None,
                    data: None,
                },
            ],
        };
        let aql = classes_to_aql(&cfg);
        assert!(aql.contains("Work"));
        assert!(!aql.contains("Uncategorized"));
        assert!(aql.starts_with('['));
        assert!(aql.ends_with(']'));
    }

    #[test]
    fn category_serde_round_trip_matches_webui_shape() {
        let raw = serde_json::json!([
            {
                "name": ["Work", "Programming"],
                "rule": {"type": "regex", "regex": "vim|emacs", "ignore_case": true},
                "data": {"color": "#0F0", "score": 8}
            },
            {
                "name": ["Uncategorized"],
                "rule": {"type": "none"}
            }
        ]);
        let cats: Vec<Category> = serde_json::from_value(raw.clone()).unwrap();
        let back = serde_json::to_value(&cats).unwrap();
        assert_eq!(back, raw);
    }
}
