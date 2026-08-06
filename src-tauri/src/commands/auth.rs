use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthStatus {
    pub is_authenticated: bool,
    pub username: Option<String>,
}

#[tauri::command]
pub async fn check_auth() -> Result<AuthStatus> {
    Ok(AuthStatus {
        is_authenticated: false,
        username: None,
    })
}

#[tauri::command]
pub async fn jump_to_external(url: String) -> Result<()> {
    open::that(url).map_err(|e| crate::error::AppError::Custom(e.to_string()))?;
    Ok(())
}
