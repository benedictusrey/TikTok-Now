use crate::error::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub author: String,
}

#[tauri::command]
pub async fn get_app_info() -> Result<AppInfo> {
    Ok(AppInfo {
        name: "TikTok-Now".to_string(),
        version: "1.0.0".to_string(),
        author: "@benedictusrey".to_string(),
    })
}
