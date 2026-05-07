mod icons;
mod tracking;

use chrono::{DateTime, Utc};
use daylog_core::aggregate::{AfkSummary, CategorizedEvent, CategorySummary, HourBucket};
use daylog_core::aw_client::{AwClient, AwError, Bucket, Event, ServerInfo};
use daylog_core::categories::{self, CategoryConfig, CategoryError};
use daylog_core::queries as q;
use daylog_core::queries::{QueryError, TrailingDayPayload};
use daylog_core::time::TimeRange;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tracking::{BinDir, ExtensionStatus, InstallError, LifecycleError, Supervisor, TrackerStatus};

#[tauri::command]
async fn aw_info() -> Result<ServerInfo, AwError> {
    q::info(&AwClient::new()).await
}

#[tauri::command]
async fn aw_buckets() -> Result<Vec<Bucket>, AwError> {
    q::buckets(&AwClient::new()).await
}

#[tauri::command]
async fn aw_events(
    bucket_id: String,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    limit: Option<u32>,
) -> Result<Vec<Event>, AwError> {
    q::events(&AwClient::new(), &bucket_id, start, end, limit).await
}

#[tauri::command]
async fn aw_query(
    query: String,
    timeperiods: Vec<String>,
) -> Result<Vec<serde_json::Value>, AwError> {
    q::raw_query(&AwClient::new(), &query, &timeperiods).await
}

#[tauri::command]
async fn aw_top_apps(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    q::top_apps(&AwClient::new(), range).await
}

#[tauri::command]
async fn aw_timeline(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    q::timeline(&AwClient::new(), range).await
}

#[tauri::command]
async fn aw_top_apps_today() -> Result<Vec<serde_json::Value>, AwError> {
    q::top_apps(&AwClient::new(), TimeRange::Today).await
}

#[tauri::command]
async fn aw_timeline_today() -> Result<Vec<serde_json::Value>, AwError> {
    q::timeline(&AwClient::new(), TimeRange::Today).await
}

#[tauri::command]
async fn categories_get() -> Result<CategoryConfig, CategoryError> {
    categories::load(&AwClient::new()).await
}

#[tauri::command]
async fn categories_set(config: CategoryConfig) -> Result<(), CategoryError> {
    categories::validate(&config)?;
    categories::save(&AwClient::new(), &config).await
}

#[tauri::command]
async fn aw_top_categories(range: TimeRange) -> Result<Vec<CategorySummary>, QueryError> {
    q::top_categories(&AwClient::new(), range).await
}

#[tauri::command]
async fn aw_top_categories_today() -> Result<Vec<CategorySummary>, QueryError> {
    q::top_categories(&AwClient::new(), TimeRange::Today).await
}

#[tauri::command]
async fn aw_hourly(range: TimeRange) -> Result<Vec<HourBucket>, AwError> {
    q::hourly(&AwClient::new(), range).await
}

#[tauri::command]
async fn aw_categorized_events(range: TimeRange) -> Result<Vec<CategorizedEvent>, QueryError> {
    q::categorized_events(&AwClient::new(), range).await
}

#[tauri::command]
async fn aw_trailing_days_past(days: u32) -> Result<Vec<TrailingDayPayload>, QueryError> {
    q::trailing_days_past(days).await
}

#[tauri::command]
async fn aw_afk_summary(
    range: TimeRange,
    include_intervals: Option<bool>,
) -> Result<AfkSummary, AwError> {
    q::afk_summary(&AwClient::new(), range, include_intervals.unwrap_or(false)).await
}

#[tauri::command]
async fn aw_has_web_watcher() -> Result<bool, AwError> {
    q::has_web_watcher(&AwClient::new()).await
}

#[tauri::command]
async fn aw_top_domains(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    q::top_domains(&AwClient::new(), range).await
}

#[tauri::command]
async fn aw_top_urls(range: TimeRange) -> Result<Vec<serde_json::Value>, AwError> {
    q::top_urls(&AwClient::new(), range).await
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

/// First-launch probe: is there an aw-server already answering on :5600?
/// Used by the wizard to decide between "use existing AW" and "install bundled".
/// Never errors — connection refused / parse failure / anything else maps to None.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum Detection {
    Existing { hostname: String, version: String },
    None,
}

#[tauri::command]
async fn tracking_detect() -> Detection {
    match q::info(&AwClient::new()).await {
        Ok(info) => Detection::Existing {
            hostname: info.hostname,
            version: info.version,
        },
        Err(_) => Detection::None,
    }
}

const WIZARD_MARKER: &str = ".wizard-complete";

#[tauri::command]
fn wizard_complete_get(app: AppHandle) -> Result<bool, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(WIZARD_MARKER).exists())
}

#[tauri::command]
fn wizard_complete_set(app: AppHandle, complete: bool) -> Result<(), String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let marker = dir.join(WIZARD_MARKER);
    if complete {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(&marker, b"").map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(&marker);
    }
    Ok(())
}

/// Full first-launch install: place binaries → install systemd units (or
/// XDG autostart) → wait until aw-server answers on :5600.
#[tauri::command]
async fn tracking_install_supervisor(app: AppHandle) -> Result<TrackerStatus, LifecycleError> {
    let bin_dir = tracking::place_binaries(&app)?;
    tracking::install_supervisor(&app, &bin_dir).await?;
    tracking::wait_until_live(15).await?;
    tracking::status().await
}

#[tauri::command]
async fn tracking_status() -> Result<TrackerStatus, LifecycleError> {
    tracking::status().await
}

#[tauri::command]
async fn tracking_pause() -> Result<(), LifecycleError> {
    tracking::pause().await
}

#[tauri::command]
async fn tracking_resume(app: AppHandle) -> Result<(), LifecycleError> {
    let bin_dir = tracking::resolve_bin_dir(&app)?;
    tracking::resume(&app, &bin_dir).await
}

#[tauri::command]
async fn tracking_stop() -> Result<(), LifecycleError> {
    tracking::stop().await
}

#[tauri::command]
async fn tracking_uninstall() -> Result<(), LifecycleError> {
    tracking::uninstall().await
}

#[tauri::command]
async fn tracking_gnome_extension_status() -> ExtensionStatus {
    tracking::gnome::status().await
}

/// Install + enable the GNOME-Wayland focused-window-dbus extension.
/// Returns ExtensionStatus with `applicable: false` on every other DE.
#[tauri::command]
async fn tracking_setup_gnome_extension(app: AppHandle) -> Result<ExtensionStatus, LifecycleError> {
    tracking::gnome::setup(&app).await
}

/// Resolve each app name (X11 WM_CLASS / Wayland app_id, as reported by
/// `aw-watcher-window` in `data.app`) to a `data:`-URL icon. File I/O runs
/// in a blocking task to keep the Tauri command pool responsive on cold
/// cache. Misses return `null` so the frontend can render a letter-chip
/// fallback without a second roundtrip.
#[tauri::command]
async fn app_icons(names: Vec<String>) -> std::collections::HashMap<String, Option<String>> {
    tokio::task::spawn_blocking(move || icons::resolve_many(&names))
        .await
        .unwrap_or_default()
}

/// Synchronous wrapper around `tracking::uninstall()` for the CLI entrypoint
/// (`daylog --uninstall-tracking`). Spins up a small tokio runtime so we don't
/// require the caller to be in an async context.
pub fn uninstall_blocking() -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("create runtime: {e}"))?;
    rt.block_on(tracking::uninstall())
        .map_err(|e| e.to_string())
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
            aw_trailing_days_past,
            aw_afk_summary,
            aw_has_web_watcher,
            aw_top_domains,
            aw_top_urls,
            app_icons,
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
            tracking_gnome_extension_status,
            tracking_setup_gnome_extension,
            tracking_detect,
            wizard_complete_get,
            wizard_complete_set,
            tracking_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
