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

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, AwError> {
        let url = self.url(path);
        let resp = self.http.get(&url).send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AwError::Unreachable(self.base_url.clone())
            } else {
                AwError::Network(e.to_string())
            }
        })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AwError::BadStatus { status: status.as_u16(), body });
        }

        resp.json::<T>().await.map_err(|e| AwError::Decode(e.to_string()))
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
}
