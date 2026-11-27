use crate::error::Result;
use tauri::Url;

/// Allowlist of URL schemes that may be handed to the OS shell opener.
/// `javascript:`, `data:`, `vbscript:`, `file:` etc. are refused — a remote
/// page must never be able to make the desktop shell execute arbitrary
/// content through this command.
const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

pub(crate) fn open_safe_url_internal(url: &str) -> Result<()> {
    let parsed = Url::parse(url.trim())
        .map_err(|e| crate::error::AppError::Custom(format!("invalid URL: {e}")))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if !ALLOWED_SCHEMES.contains(&scheme.as_str()) {
        return Err(crate::error::AppError::Custom(format!(
            "scheme '{scheme}' is not allowed for external opening"
        )));
    }
    open::that(parsed.as_str()).map_err(|e| crate::error::AppError::Custom(e.to_string()))?;
    Ok(())
}

/// Open a URL in the OS default browser / handler (legacy command alias).
#[tauri::command]
pub async fn jump_to_external(url: String) -> Result<()> {
    open_safe_url_internal(&url)
}

/// Open a URL in the OS default browser / handler.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<()> {
    open_safe_url_internal(&url)
}
