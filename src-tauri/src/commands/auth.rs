use crate::error::Result;
use tauri::Url;

/// Allowlist of URL schemes that may be handed to the OS shell opener.
/// `javascript:`, `data:`, `vbscript:`, `file:` etc. are refused — a remote
/// page must never be able to make the desktop shell execute arbitrary
/// content through this command. (v2.1.0: the command previously forwarded
/// any string straight to `open::that`.)
const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// Open a URL in the OS default browser / handler.
///
/// Called from the injected page engine (`window.__tiktoknow_open`) for every
/// external link the user clicks inside TikTok, so it is deliberately strict:
/// only real web/e-mail URLs pass; everything else is rejected with an error.
#[tauri::command]
pub async fn jump_to_external(url: String) -> Result<()> {
    let parsed =
        Url::parse(&url).map_err(|e| crate::error::AppError::Custom(format!("invalid URL: {e}")))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if !ALLOWED_SCHEMES.contains(&scheme.as_str()) {
        return Err(crate::error::AppError::Custom(format!(
            "scheme '{scheme}' is not allowed for external opening"
        )));
    }
    open::that(parsed.as_str()).map_err(|e| crate::error::AppError::Custom(e.to_string()))?;
    Ok(())
}
