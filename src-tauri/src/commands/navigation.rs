use crate::error::Result;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn navigate_to(app: AppHandle, target: String) -> Result<()> {
    if let Some(main) = app.get_webview_window("main") {
        let url = match target.as_str() {
            "foryou" => "https://www.tiktok.com/foryou",
            "following" => "https://www.tiktok.com/following",
            "explore" => "https://www.tiktok.com/explore",
            "live" => "https://www.tiktok.com/live",
            "friends" => "https://www.tiktok.com/friends",
            "upload" => "https://www.tiktok.com/upload",
            _ => "https://www.tiktok.com/",
        };
        let js = format!("window.location.href = '{}';", url);
        let _ = main.eval(&js);
    }
    Ok(())
}
