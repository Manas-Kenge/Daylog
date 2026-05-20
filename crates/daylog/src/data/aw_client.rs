use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:5600";

fn shared_http() -> &'static reqwest::Client {
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client build")
    })
}

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
pub struct Event {
    #[serde(default)]
    pub id: Option<u64>,
    pub timestamp: DateTime<Utc>,
    pub duration: f64,
    pub data: serde_json::Value,
}

pub struct AwClient {
    base_url: String,
    http: &'static reqwest::Client,
}

impl Default for AwClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AwClient {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            http: shared_http(),
        }
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

    async fn decode<T: for<'de> Deserialize<'de>>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, AwError> {
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AwError::BadStatus {
                status: status.as_u16(),
                body,
            });
        }
        resp.json::<T>()
            .await
            .map_err(|e| AwError::Decode(e.to_string()))
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, AwError> {
        let resp = self
            .http
            .get(self.url(path))
            .send()
            .await
            .map_err(|e| self.classify(e))?;
        self.decode(resp).await
    }

    pub async fn info(&self) -> Result<ServerInfo, AwError> {
        self.get_json("/api/0/info").await
    }

    /// 404 / null both map to `Ok(None)`.
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
            return Err(AwError::BadStatus {
                status: status.as_u16(),
                body,
            });
        }
        Ok(())
    }
}

