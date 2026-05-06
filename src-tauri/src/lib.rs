mod aggregate;
mod aw_client;
mod categories;
mod time;
mod tracking;

use aggregate::{
    bucketize_hourly, categorize_events, fetch_afk_events, fetch_window_events, summarize_afk,
    unwrap_first_array, AfkSummary, CategorizedEvent, HourBucket,
};
use aw_client::{queries, AwClient, AwError, Bucket, Event, ServerInfo};
use categories::{CategoryConfig, CategoryError, CategorySummary, Matcher};
use chrono::{DateTime, Utc};
use tauri::AppHandle;
use time::TimeRange;
use tracking::{BinDir, InstallError, LifecycleError, Supervisor, TrackerStatus};

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
async fn aw_top_apps(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    let res = AwClient::new()
        .query(queries::top_apps(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

#[tauri::command]
async fn aw_timeline(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    let res = AwClient::new()
        .query(queries::timeline(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

#[tauri::command]
async fn aw_top_apps_today() -> Result<Vec<serde_json::Value>, AwError> {
    aw_top_apps(TimeRange::Today).await
}

#[tauri::command]
async fn aw_timeline_today() -> Result<Vec<serde_json::Value>, AwError> {
    aw_timeline(TimeRange::Today).await
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
    Matcher::new(&config)?;
    categories::save(&app, &config)
}

#[tauri::command]
async fn aw_top_categories(
    app: AppHandle,
    range: TimeRange,
) -> Result<Vec<CategorySummary>, AppError> {
    let cfg = categories::load(&app)?;
    let matcher = Matcher::new(&cfg)?;
    let client = AwClient::new();
    let events = fetch_window_events(&client, &range).await?;
    Ok(categories::summarize(&matcher, &events))
}

#[tauri::command]
async fn aw_top_categories_today(app: AppHandle) -> Result<Vec<CategorySummary>, AppError> {
    aw_top_categories(app, TimeRange::Today).await
}

#[tauri::command]
async fn aw_hourly(range: TimeRange) -> Result<Vec<HourBucket>, AwError> {
    let client = AwClient::new();
    let events = fetch_window_events(&client, &range).await?;
    Ok(bucketize_hourly(&events))
}

#[tauri::command]
async fn aw_categorized_events(
    app: AppHandle,
    range: TimeRange,
) -> Result<Vec<CategorizedEvent>, AppError> {
    let cfg = categories::load(&app)?;
    let matcher = Matcher::new(&cfg)?;
    let client = AwClient::new();
    let events = fetch_window_events(&client, &range).await?;
    Ok(categorize_events(&matcher, &events))
}

#[tauri::command]
async fn aw_afk_summary(
    range: TimeRange,
    include_intervals: Option<bool>,
) -> Result<AfkSummary, AwError> {
    let client = AwClient::new();
    let buckets = client.buckets().await?;
    if !buckets.iter().any(|b| b.id.starts_with("aw-watcher-afk_")) {
        return Ok(summarize_afk(&[], include_intervals.unwrap_or(false)));
    }
    let events = fetch_afk_events(&client, &range).await?;
    Ok(summarize_afk(&events, include_intervals.unwrap_or(false)))
}

#[tauri::command]
async fn aw_has_web_watcher() -> Result<bool, AwError> {
    let buckets = AwClient::new().buckets().await?;
    Ok(buckets.iter().any(|b| b.id.starts_with("aw-watcher-web-")))
}

#[tauri::command]
async fn aw_top_domains(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    let client = AwClient::new();
    let buckets = client.buckets().await?;
    if !buckets.iter().any(|b| b.id.starts_with("aw-watcher-web-")) {
        return Ok(vec![]);
    }
    let res = client
        .query(queries::web_top_domains(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
}

#[tauri::command]
async fn tracking_resolve_bin_dir(app: AppHandle) -> Result<BinDir, InstallError> {
    tracking::resolve_bin_dir(&app)
}

#[tauri::command]
async fn tracking_place_binaries(app: AppHandle) -> Result<BinDir, InstallError> {
    tracking::place_binaries(&app)
}

#[tauri::command]
fn tracking_detect_supervisor() -> Supervisor {
    tracking::detect()
}

/// Full first-launch install: place binaries → install systemd units (or
/// XDG autostart) → wait until aw-server answers on :5600.
#[tauri::command]
async fn tracking_install_supervisor(app: AppHandle) -> Result<TrackerStatus, LifecycleError> {
    let bin_dir = tracking::place_binaries(&app)?;
    tracking::install_supervisor(&app, &bin_dir).await?;
    tracking::wait_until_live(15).await?;
    tracking::status(&app).await
}

#[tauri::command]
async fn tracking_status(app: AppHandle) -> Result<TrackerStatus, LifecycleError> {
    tracking::status(&app).await
}

#[tauri::command]
async fn tracking_pause(app: AppHandle) -> Result<(), LifecycleError> {
    tracking::pause(&app).await
}

#[tauri::command]
async fn tracking_resume(app: AppHandle) -> Result<(), LifecycleError> {
    let bin_dir = tracking::resolve_bin_dir(&app)?;
    tracking::resume(&app, &bin_dir).await
}

#[tauri::command]
async fn tracking_stop(app: AppHandle) -> Result<(), LifecycleError> {
    tracking::stop(&app).await
}

#[tauri::command]
async fn aw_top_urls(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    let client = AwClient::new();
    let buckets = client.buckets().await?;
    if !buckets.iter().any(|b| b.id.starts_with("aw-watcher-web-")) {
        return Ok(vec![]);
    }
    let res = client
        .query(queries::web_top_urls(), &[range.as_aw_timeperiod()])
        .await?;
    Ok(unwrap_first_array(res))
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
            aw_top_apps,
            aw_timeline,
            aw_top_categories,
            aw_top_apps_today,
            aw_timeline_today,
            aw_top_categories_today,
            aw_hourly,
            aw_categorized_events,
            aw_afk_summary,
            aw_has_web_watcher,
            aw_top_domains,
            aw_top_urls,
            categories_get,
            categories_set,
            tracking_resolve_bin_dir,
            tracking_place_binaries,
            tracking_detect_supervisor,
            tracking_install_supervisor,
            tracking_status,
            tracking_pause,
            tracking_resume,
            tracking_stop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
