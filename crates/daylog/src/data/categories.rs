use std::sync::OnceLock;

use fancy_regex::Regex as FancyRegex;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::data::aw_client::{AwClient, AwError};

const SETTINGS_KEY: &str = "classes";

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

/// Patterns must compile under `fancy_regex` — the runtime engine in
/// `transforms::compile_rules`.
pub fn validate(cfg: &CategoryConfig) -> Result<(), CategoryError> {
    for cat in &cfg.categories {
        if let Rule::Regex { regex, ignore_case } = &cat.rule {
            let pat = if *ignore_case {
                format!("(?i){regex}")
            } else {
                regex.clone()
            };
            FancyRegex::new(&pat).map_err(|e| CategoryError::InvalidRegex {
                category: cat.name.clone(),
                error: e.to_string(),
            })?;
        }
    }
    Ok(())
}

/// Memoized. Seeds defaults on absent key; persist write is best-effort.
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
    fn validate_accepts_lookahead_patterns() {
        let cfg = CategoryConfig {
            categories: vec![Category {
                name: vec!["LookaheadOk".into()],
                rule: Rule::Regex {
                    regex: "testing (?!lookahead)".into(),
                    ignore_case: false,
                },
                data: None,
            }],
        };
        validate(&cfg).expect("fancy_regex lookahead should validate");
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
