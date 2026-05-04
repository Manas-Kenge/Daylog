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
}

pub mod queries {
    pub fn top_apps() -> &'static str {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = query_bucket(find_bucket("aw-watcher-window_"));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = merge_events_by_keys(events, ["app"]);
        events = sort_by_duration(events);
        RETURN = events;
        "#
    }

    pub fn timeline() -> &'static str {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = query_bucket(find_bucket("aw-watcher-window_"));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        RETURN = events;
        "#
    }

    pub fn web_top_domains() -> &'static str {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = query_bucket(find_bucket("aw-watcher-web-"));
        events = split_url_events(events);
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = merge_events_by_keys(events, ["$domain"]);
        events = sort_by_duration(events);
        RETURN = events;
        "#
    }

    pub fn web_top_urls() -> &'static str {
        r#"
        afk = query_bucket(find_bucket("aw-watcher-afk_"));
        events = query_bucket(find_bucket("aw-watcher-web-"));
        events = filter_period_intersect(events, filter_keyvals(afk, "status", ["not-afk"]));
        events = merge_events_by_keys(events, ["url"]);
        events = sort_by_duration(events);
        RETURN = events;
        "#
    }

    pub fn afk_events() -> &'static str {
        r#"
        events = query_bucket(find_bucket("aw-watcher-afk_"));
        RETURN = events;
        "#
    }
}
