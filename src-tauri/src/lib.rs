mod aw_client;
mod categories;

use aw_client::{queries, AwClient, AwError, Bucket, Event, ServerInfo};
use categories::{CategoryConfig, CategoryError, CategorySummary, Matcher};
use chrono::{DateTime, Datelike, Local, NaiveTime, TimeZone, Utc};
use tauri::AppHandle;

#[tauri::command]
async fn aw_info() -> Result<ServerInfo, AwError> {
    AwClient::new().info().await
}

#[tauri::command]
async fn aw_buckets() -> Result<Vec<Bucket>, AwError> {
    AwClient::new().buckets().await
}

#[tauri::command]
async fn aw_events(
    bucket_id: String,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    limit: Option<u32>,
) -> Result<Vec<Event>, AwError> {
    AwClient::new().events(&bucket_id, start, end, limit).await
}

#[tauri::command]
async fn aw_query(
    query: String,
    timeperiods: Vec<String>,
) -> Result<Vec<serde_json::Value>, AwError> {
    AwClient::new().query(&query, &timeperiods).await
}

#[tauri::command]
async fn aw_top_apps_today() -> Result<Vec<serde_json::Value>, AwError> {
    let tp = today_local_period();
    let res = AwClient::new().query(queries::top_apps_today(), &[tp]).await?;
    Ok(res.into_iter().next().and_then(|v| v.as_array().cloned()).unwrap_or_default())
}

#[tauri::command]
async fn aw_timeline_today() -> Result<Vec<serde_json::Value>, AwError> {
    let tp = today_local_period();
    let res = AwClient::new().query(queries::timeline_today(), &[tp]).await?;
    Ok(res.into_iter().next().and_then(|v| v.as_array().cloned()).unwrap_or_default())
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("{0}")]
    Aw(#[from] AwError),
    #[error("{0}")]
    Category(#[from] CategoryError),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[tauri::command]
async fn categories_get(app: AppHandle) -> Result<CategoryConfig, CategoryError> {
    categories::load(&app)
}

#[tauri::command]
async fn categories_set(app: AppHandle, config: CategoryConfig) -> Result<(), CategoryError> {
    // Validate by compiling.
    Matcher::new(&config)?;
    categories::save(&app, &config)
}

#[tauri::command]
async fn aw_top_categories_today(app: AppHandle) -> Result<Vec<CategorySummary>, AppError> {
    let cfg = categories::load(&app)?;
    let matcher = Matcher::new(&cfg)?;
    let tp = today_local_period();
    let res = AwClient::new()
        .query(queries::timeline_today(), &[tp])
        .await?;
    let events = res
        .into_iter()
        .next()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    Ok(categories::summarize(&matcher, &events))
}

fn today_local_period() -> String {
    let now = Local::now();
    let start_local = Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .unwrap_or_else(|| now.with_time(NaiveTime::MIN).single().unwrap_or(now));
    let end_local = start_local + chrono::Duration::days(1);
    format!(
        "{}/{}",
        start_local.with_timezone(&Utc).to_rfc3339(),
        end_local.with_timezone(&Utc).to_rfc3339()
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            aw_info,
            aw_buckets,
            aw_events,
            aw_query,
            aw_top_apps_today,
            aw_timeline_today,
            aw_top_categories_today,
            categories_get,
            categories_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
