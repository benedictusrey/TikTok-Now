//! WebView2 sleep for the hidden TikTok-Now window (Windows only).
//!
//! When the window is minimized or hidden, the page is put into WebView2's
//! suspended state and the memory target is lowered, which releases most of the
//! renderer memory. Restoring resumes the page and returns the target to normal.
//! Every call is best effort: a failure leaves the page running, as before.

use tauri::webview::PlatformWebview;
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    },
    TrySuspendCompletedHandler,
};
use windows::core::Interface;

/// Sleep (`true`) or wake (`false`) the page behind this webview.
pub fn set_sleeping(webview: &PlatformWebview, sleeping: bool) {
    unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        if let Ok(core3) = core.cast::<ICoreWebView2_3>() {
            if sleeping {
                let handler = TrySuspendCompletedHandler::create(Box::new(|_error, _suspended| {
                    #[cfg(debug_assertions)]
                    eprintln!("[TikTok-Now] Page suspend: hr={_error:?} suspended={_suspended:?}");
                    Ok(())
                }));
                let _ = core3.TrySuspend(&handler);
            } else {
                let _ = core3.Resume();
            }
        }
        if let Ok(core19) = core.cast::<ICoreWebView2_19>() {
            let level = if sleeping {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            let _ = core19.SetMemoryUsageTargetLevel(level);
        }
    }
}
