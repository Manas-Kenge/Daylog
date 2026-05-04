mod aw_client;

use aw_client::{AwClient, AwError, Bucket, ServerInfo};

#[tauri::command]
async fn aw_info() -> Result<ServerInfo, AwError> {
    AwClient::new().info().await
}

#[tauri::command]
async fn aw_buckets() -> Result<Vec<Bucket>, AwError> {
    AwClient::new().buckets().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![aw_info, aw_buckets])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
