use std::collections::HashMap;
use std::path::PathBuf;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const STORE_FILE: &str = "categories.json";

#[derive(Debug, thiserror::Error)]
pub enum CategoryError {
    #[error("io: {0}")]
    Io(String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("invalid regex in category {category:?}: {error}")]
    InvalidRegex { category: Vec<String>, error: String },
    #[error("could not resolve app config dir")]
    NoConfigDir,
}

impl serde::Serialize for CategoryError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: Vec<String>,
    pub rule: Rule,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CategoryConfig {
    pub categories: Vec<Category>,
}

impl CategoryConfig {
    pub fn defaults() -> Self {
        Self {
            categories: vec![
                Category {
                    name: vec!["Work".into(), "Programming".into()],
                    rule: Rule::Regex {
                        regex: "code|kitty|terminal|alacritty|gnome-terminal|wezterm".into(),
                        ignore_case: true,
                    },
                },
                Category {
                    name: vec!["Work".into(), "Documents".into()],
                    rule: Rule::Regex {
                        regex: "libreoffice|writer|calc|notion|obsidian".into(),
                        ignore_case: true,
                    },
                },
                Category {
                    name: vec!["Media".into(), "Music".into()],
                    rule: Rule::Regex {
                        regex: "spotify|rhythmbox".into(),
                        ignore_case: true,
                    },
                },
                Category {
                    name: vec!["Media".into(), "Video".into()],
                    rule: Rule::Regex {
                        regex: "youtube|vlc|mpv".into(),
                        ignore_case: true,
                    },
                },
                Category {
                    name: vec!["Comms".into()],
                    rule: Rule::Regex {
                        regex: "slack|discord|telegram|signal|thunderbird".into(),
                        ignore_case: true,
                    },
                },
                Category {
                    name: vec!["Browsing".into()],
                    rule: Rule::Regex {
                        regex: "firefox|brave|chromium|chrome".into(),
                        ignore_case: true,
                    },
                },
            ],
        }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, CategoryError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| CategoryError::NoConfigDir)?;
    std::fs::create_dir_all(&dir).map_err(|e| CategoryError::Io(e.to_string()))?;
    Ok(dir.join(STORE_FILE))
}

pub fn load(app: &AppHandle) -> Result<CategoryConfig, CategoryError> {
    let path = config_path(app)?;
    if !path.exists() {
        let cfg = CategoryConfig::defaults();
        save(app, &cfg)?;
        return Ok(cfg);
    }
    let bytes = std::fs::read(&path).map_err(|e| CategoryError::Io(e.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|e| CategoryError::Parse(e.to_string()))
}

pub fn save(app: &AppHandle, cfg: &CategoryConfig) -> Result<(), CategoryError> {
    let path = config_path(app)?;
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| CategoryError::Parse(e.to_string()))?;
    std::fs::write(&path, bytes).map_err(|e| CategoryError::Io(e.to_string()))?;
    Ok(())
}

struct CompiledRule {
    name: Vec<String>,
    pattern: Option<Regex>,
}

pub struct Matcher {
    rules: Vec<CompiledRule>,
}

impl Matcher {
    pub fn new(cfg: &CategoryConfig) -> Result<Self, CategoryError> {
        let mut rules = Vec::with_capacity(cfg.categories.len());
        for cat in &cfg.categories {
            let pattern = match &cat.rule {
                Rule::None => None,
                Rule::Regex { regex, ignore_case } => {
                    let mut builder = regex::RegexBuilder::new(regex);
                    builder.case_insensitive(*ignore_case);
                    let re = builder.build().map_err(|e| CategoryError::InvalidRegex {
                        category: cat.name.clone(),
                        error: e.to_string(),
                    })?;
                    Some(re)
                }
            };
            rules.push(CompiledRule { name: cat.name.clone(), pattern });
        }
        Ok(Self { rules })
    }

    pub fn classify(&self, data: &serde_json::Value) -> Vec<String> {
        let app = data.get("app").and_then(|v| v.as_str()).unwrap_or("");
        let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("");
        for rule in &self.rules {
            if let Some(re) = &rule.pattern {
                if re.is_match(app) || re.is_match(title) {
                    return rule.name.clone();
                }
            }
        }
        vec!["Uncategorized".into()]
    }
}

#[derive(Debug, Serialize)]
pub struct CategorySummary {
    pub name: Vec<String>,
    pub duration: f64,
}

pub fn summarize(matcher: &Matcher, events: &[serde_json::Value]) -> Vec<CategorySummary> {
    let mut totals: HashMap<Vec<String>, f64> = HashMap::new();
    for ev in events {
        let data = ev.get("data").cloned().unwrap_or(serde_json::Value::Null);
        let duration = ev.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let name = matcher.classify(&data);
        *totals.entry(name).or_insert(0.0) += duration;
    }
    let mut out: Vec<CategorySummary> = totals
        .into_iter()
        .map(|(name, duration)| CategorySummary { name, duration })
        .collect();
    out.sort_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap_or(std::cmp::Ordering::Equal));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_by_app_name() {
        let cfg = CategoryConfig::defaults();
        let m = Matcher::new(&cfg).unwrap();
        assert_eq!(m.classify(&json!({"app": "kitty", "title": ""})), vec!["Work", "Programming"]);
        assert_eq!(m.classify(&json!({"app": "spotify", "title": ""})), vec!["Media", "Music"]);
        assert_eq!(m.classify(&json!({"app": "brave", "title": ""})), vec!["Browsing"]);
    }

    #[test]
    fn falls_back_to_uncategorized() {
        let cfg = CategoryConfig::defaults();
        let m = Matcher::new(&cfg).unwrap();
        assert_eq!(m.classify(&json!({"app": "weirdapp", "title": ""})), vec!["Uncategorized"]);
    }

    #[test]
    fn case_insensitive_matches() {
        let cfg = CategoryConfig::defaults();
        let m = Matcher::new(&cfg).unwrap();
        assert_eq!(m.classify(&json!({"app": "Code", "title": ""})), vec!["Work", "Programming"]);
        assert_eq!(m.classify(&json!({"app": "FIREFOX", "title": ""})), vec!["Browsing"]);
    }

    #[test]
    fn invalid_regex_surfaces_error() {
        let cfg = CategoryConfig {
            categories: vec![Category {
                name: vec!["Bad".into()],
                rule: Rule::Regex { regex: "(unclosed".into(), ignore_case: false },
            }],
        };
        match Matcher::new(&cfg) {
            Err(CategoryError::InvalidRegex { .. }) => {}
            Err(e) => panic!("expected InvalidRegex, got {e:?}"),
            Ok(_) => panic!("expected InvalidRegex, got Ok"),
        }
    }

    #[test]
    fn summarize_groups_and_sorts() {
        let cfg = CategoryConfig::defaults();
        let m = Matcher::new(&cfg).unwrap();
        let events = vec![
            json!({"data": {"app": "kitty"}, "duration": 100.0}),
            json!({"data": {"app": "code"}, "duration": 50.0}),
            json!({"data": {"app": "spotify"}, "duration": 200.0}),
            json!({"data": {"app": "weirdapp"}, "duration": 25.0}),
        ];
        let out = summarize(&m, &events);
        assert_eq!(out[0].name, vec!["Media", "Music"]);
        assert_eq!(out[0].duration, 200.0);
        assert_eq!(out[1].name, vec!["Work", "Programming"]);
        assert_eq!(out[1].duration, 150.0);
        assert_eq!(out[2].name, vec!["Uncategorized"]);
    }
}
