use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:5600";

#[derive(Debug, thiserror::Error)]
pub enum AwError {
    #[error("aw-server unreachable at {0}")]
    Unreachable(String),
    #[error("aw-server returned {status}: {body}")]
    BadStatus { status: u16, body: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("invalid response: {0}")]
    Decode(String),
}

impl serde::Serialize for AwError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerInfo {
    pub hostname: String,
    pub version: String,
    pub testing: bool,
    #[serde(default)]
    pub device_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bucket {
    pub id: String,
    #[serde(rename = "type")]
    pub bucket_type: String,
    pub client: String,
    pub hostname: String,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Event {
    #[serde(default)]
    pub id: Option<u64>,
    pub timestamp: DateTime<Utc>,
    pub duration: f64,
    pub data: serde_json::Value,
}

pub struct AwClient {
    base_url: String,
    http: reqwest::Client,
}

impl AwClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client build");
        Self { base_url: DEFAULT_BASE_URL.into(), http }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn classify(&self, e: reqwest::Error) -> AwError {
        if e.is_connect() || e.is_timeout() {
            AwError::Unreachable(self.base_url.clone())
        } else {
            AwError::Network(e.to_string())
        }
    }

    async fn decode<T: for<'de> Deserialize<'de>>(&self, resp: reqwest::Response) -> Result<T, AwError> {
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AwError::BadStatus { status: status.as_u16(), body });
        }
        resp.json::<T>().await.map_err(|e| AwError::Decode(e.to_string()))
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, AwError> {
        let resp = self.http.get(self.url(path)).send().await.map_err(|e| self.classify(e))?;
        self.decode(resp).await
    }

    pub async fn info(&self) -> Result<ServerInfo, AwError> {
        self.get_json("/api/0/info").await
    }

    pub async fn buckets(&self) -> Result<Vec<Bucket>, AwError> {
        let map: HashMap<String, Bucket> = self.get_json("/api/0/buckets/").await?;
        let mut out: Vec<Bucket> = map.into_values().collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    pub async fn events(
        &self,
        bucket_id: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, AwError> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(s) = start {
            params.push(("start", s.to_rfc3339()));
        }
        if let Some(e) = end {
            params.push(("end", e.to_rfc3339()));
        }
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }
        let resp = self
            .http
            .get(self.url(&format!("/api/0/buckets/{bucket_id}/events")))
            .query(&params)
            .send()
            .await
            .map_err(|e| self.classify(e))?;
        self.decode(resp).await
    }

    pub async fn query(
        &self,
        query: &str,
        timeperiods: &[String],
    ) -> Result<Vec<serde_json::Value>, AwError> {
        let body = serde_json::json!({
            "query": [query],
            "timeperiods": timeperiods,
        });
        let resp = self
            .http
            .post(self.url("/api/0/query/"))
            .json(&body)
            .send()
            .await
            .map_err(|e| self.classify(e))?;
        self.decode(resp).await
    }

    /// Read a value from aw-server's settings store. Used as the canonical
    /// home for category rules (`classes`), matching the AW WebUI.
    /// Returns `Ok(None)` for 404 (key absent) so callers can seed defaults
    /// without conflating "missing" with "transport error".
    pub async fn get_setting<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Result<Option<T>, AwError> {
        let resp = self
            .http
            .get(self.url(&format!("/api/0/settings/{key}")))
            .send()
            .await
            .map_err(|e| self.classify(e))?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        let v: serde_json::Value = self.decode(resp).await?;
        // aw-server returns `null` for missing keys in some versions; treat
        // either as absent.
        if v.is_null() {
            return Ok(None);
        }
        let parsed = serde_json::from_value(v).map_err(|e| AwError::Decode(e.to_string()))?;
        Ok(Some(parsed))
    }

    pub async fn set_setting<T: Serialize + ?Sized>(
        &self,
        key: &str,
        value: &T,
    ) -> Result<(), AwError> {
        let resp = self
            .http
            .post(self.url(&format!("/api/0/settings/{key}")))
            .json(value)
            .send()
            .await
            .map_err(|e| self.classify(e))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AwError::BadStatus { status: status.as_u16(), body });
        }
        Ok(())
    }
}

/// Canonical AQL query strings, mirroring the patterns the AW WebUI uses
/// (see `webpack://aw-webui/./src/queries.ts` in upstream sourcemaps).
///
/// `flood()` wraps window-event reads so small gaps between watcher samples
/// don't get dropped on the floor — this matches WebUI behavior and avoids
/// undercounting durations.
///
/// Queries that need categorization take a pre-serialized JSON `classes`
/// argument (the `[[name, rule], ...]` shape AQL `categorize()` expects).
pub mod queries {
    pub fn top_apps() -> String {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = flood(query_bucket(find_bucket("aw-watcher-window_")));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = merge_events_by_keys(events, ["app"]);
        events = sort_by_duration(events);
        RETURN = events;
        "#
        .to_string()
    }

    pub fn timeline() -> String {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = flood(query_bucket(find_bucket("aw-watcher-window_")));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        RETURN = events;
        "#
        .to_string()
    }

    pub fn web_top_domains() -> String {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = query_bucket(find_bucket("aw-watcher-web-"));
        events = split_url_events(events);
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = merge_events_by_keys(events, ["$domain"]);
        events = sort_by_duration(events);
        RETURN = events;
        "#
        .to_string()
    }

    pub fn web_top_urls() -> String {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = query_bucket(find_bucket("aw-watcher-web-"));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = merge_events_by_keys(events, ["url"]);
        events = sort_by_duration(events);
        RETURN = events;
        "#
        .to_string()
    }

    pub fn afk_events() -> String {
        r#"
        events = query_bucket(find_bucket("aw-watcher-afk_"));
        RETURN = events;
        "#
        .to_string()
    }

    /// Top categories: server-side `categorize()` + `merge_events_by_keys(["$category"])`.
    /// `classes_json` is the JSON literal `[[name, rule], ...]` array, embedded
    /// verbatim into the AQL string (AQL parses JSON-ish literals).
    pub fn top_categories(classes_json: &str) -> String {
        format!(
            r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = flood(query_bucket(find_bucket("aw-watcher-window_")));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = categorize(events, {classes_json});
        events = merge_events_by_keys(events, ["$category"]);
        events = sort_by_duration(events);
        RETURN = events;
        "#
        )
    }

    /// All AFK-filtered window events, with `data.$category` populated by aw-server.
    /// Drives the timeline + sparkline pipelines.
    pub fn categorized_events(classes_json: &str) -> String {
        format!(
            r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = flood(query_bucket(find_bucket("aw-watcher-window_")));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = categorize(events, {classes_json});
        RETURN = events;
        "#
        )
    }
}
