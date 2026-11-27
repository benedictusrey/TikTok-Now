#[cfg(windows)]
mod audio;
mod commands;
mod downloader;
mod error;
mod geometry;
mod tray;
#[cfg(windows)]
mod webview_sleep;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
#[cfg(windows)]
use std::sync::atomic::AtomicU32;

use tauri::{
    webview::NewWindowResponse, window::Color, AppHandle, Manager, Url, WebviewUrl,
    WebviewWindowBuilder,
};

use geometry::{
    fit_window_to_work_area, fit_window_to_work_area_on_monitor, DEFAULT_INNER_SIZE,
    DOWNLOADER_MIN_HEIGHT, DOWNLOADER_MIN_WIDTH, DOWNLOADER_WINDOW_HEIGHT,
    DOWNLOADER_WINDOW_WIDTH, MIN_INNER_SIZE,
};

const MAIN_WINDOW_LABEL: &str = "main";
const DOWNLOADER_WINDOW_LABEL: &str = "downloader";
const INIT_JS: &str = include_str!("../../frontend/tiktok-tools.js");

/// WebView2 browser arguments for the main window.
/// Overriding `additional_browser_args` replaces wry's built-in defaults, so we
/// re-include `msWebOOUI,msPdfOOUI,msSmartScreenProtection` together with
/// `CalculateNativeWinOcclusion`, `--disable-direct-composition-video-overlays`,
/// and `--autoplay-policy=no-user-gesture-required`.
const DEFAULT_WEBVIEW_BROWSER_ARGS: &str = "--enable-features=msTranslate --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --disable-direct-composition-video-overlays --no-first-run --autoplay-policy=no-user-gesture-required";

#[cfg(windows)]
static POPUP_COUNTER: AtomicU32 = AtomicU32::new(0);
static WATCHDOG_STARTED: AtomicBool = AtomicBool::new(false);
#[cfg(all(debug_assertions, not(target_os = "linux")))]
static PAUSE_RECHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static WATCHDOG_WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

#[cfg(not(target_os = "linux"))]
const HIDDEN_POLL: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(not(target_os = "linux"))]
const VISIBLE_POLL: std::time::Duration = std::time::Duration::from_millis(800);

#[cfg(not(target_os = "linux"))]
struct WatchdogGuard;
#[cfg(not(target_os = "linux"))]
impl Drop for WatchdogGuard {
    fn drop(&mut self) {
        WATCHDOG_STARTED.store(false, Ordering::Release);
    }
}

/// Diagnostics (`eprintln!` traces) are debug-build-only.
#[cfg(debug_assertions)]
macro_rules! diag {
    ($($arg:tt)*) => { eprintln!($($arg)*) };
}
#[cfg(not(debug_assertions))]
macro_rules! diag {
    ($($arg:tt)*) => {};
}

/// Interrupt the watchdog's wait so it re-evaluates window state immediately.
pub(crate) fn wake_watchdog() {
    let mut pending = WATCHDOG_WAKE
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *pending = true;
    WATCHDOG_WAKE.1.notify_all();
}

#[cfg_attr(target_os = "linux", allow(dead_code))]
fn is_window_hidden_state(minimized: Option<bool>, visible: Option<bool>) -> bool {
    // On Wayland, is_minimized() is unsupported (None); treat as not minimized
    // and rely on visibility so playback is never permanently paused.
    minimized.unwrap_or(false) || !visible.unwrap_or(false)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn dmabuf_disable_reason(
    dmabuf_override: Option<&std::ffi::OsStr>,
    is_wsl: bool,
    is_nvidia: bool,
    has_render_node: bool,
    using_x11: bool,
    x11_has_dri3: bool,
) -> Option<&'static str> {
    if !cfg!(target_os = "linux") || dmabuf_override.is_some() {
        return None;
    }
    if is_wsl {
        return Some("wslg");
    }
    if is_nvidia {
        return Some("nvidia");
    }
    if !has_render_node {
        return Some("no-render-node");
    }
    if using_x11 && !x11_has_dri3 {
        return Some("x11-no-dri3");
    }
    None
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn should_demote_nvdec_plugins(
    gst_rank_override: Option<&std::ffi::OsStr>,
    dmabuf_disabled: bool,
) -> bool {
    cfg!(target_os = "linux") && dmabuf_disabled && gst_rank_override.is_none()
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn recommended_skia_cpu_threads(available_cpus: usize) -> usize {
    if available_cpus < 4 {
        available_cpus.max(1)
    } else {
        (available_cpus / 2).clamp(4, 8)
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn should_configure_cpu_fallback_tuning(
    dmabuf_disabled: bool,
    env_override: Option<&std::ffi::OsStr>,
) -> bool {
    cfg!(target_os = "linux") && dmabuf_disabled && env_override.is_none()
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn should_default_x11_backend(
    gdk_backend: Option<&std::ffi::OsStr>,
    wayland_display: Option<&std::ffi::OsStr>,
    x11_display: Option<&std::ffi::OsStr>,
) -> bool {
    cfg!(target_os = "linux")
        && gdk_backend.is_none()
        && wayland_display.is_some_and(|v| !v.is_empty())
        && x11_display.is_some_and(|v| !v.is_empty())
}

#[cfg(target_os = "linux")]
fn is_wsl_runtime() -> bool {
    if std::env::var_os("WSL_DISTRO_NAME").is_some_and(|v| !v.is_empty())
        || std::env::var_os("WSL_INTEROP").is_some_and(|v| !v.is_empty())
        || std::env::var_os("WSL2_GUI_APPS_ENABLED").is_some_and(|v| !v.is_empty())
    {
        return true;
    }
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            lower.contains("microsoft") || lower.contains("wsl")
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn has_drm_render_node_in_dir(dir: &Path) -> bool {
    fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("renderD"))
        })
}

#[cfg(target_os = "linux")]
fn is_nvidia_gpu() -> bool {
    Path::new("/proc/driver/nvidia/version").exists() || Path::new("/sys/module/nvidia").exists()
}

#[cfg(target_os = "linux")]
fn x11_supports_dri3() -> bool {
    let Ok(xlib) = x11_dl::xlib::Xlib::open() else {
        return false;
    };
    unsafe {
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return false;
        }
        let mut major = 0;
        let mut event = 0;
        let mut error = 0;
        let ok = (xlib.XQueryExtension)(
            display,
            c"DRI3".as_ptr(),
            &mut major,
            &mut event,
            &mut error,
        ) != 0;
        (xlib.XCloseDisplay)(display);
        ok
    }
}

#[cfg(target_os = "linux")]
static LINUX_DMABUF_REASON: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// Configure Linux graphics and GStreamer environment fallbacks before GTK/WebKitGTK starts.
pub fn configure_linux_runtime_env() {
    if should_default_x11_backend(
        std::env::var_os("GDK_BACKEND").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os("DISPLAY").as_deref(),
    ) {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    #[cfg(target_os = "linux")]
    {
        let dmabuf_override = std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER");
        let reason = if dmabuf_override.is_some() {
            "user-override"
        } else {
            let is_wsl = is_wsl_runtime();
            let is_nvidia = is_nvidia_gpu();
            let has_render_node = has_drm_render_node_in_dir(Path::new("/dev/dri"));
            let using_x11 = !geometry::is_wayland_session();
            let x11_has_dri3 = if using_x11 && !is_wsl && !is_nvidia && has_render_node {
                x11_supports_dri3()
            } else {
                false
            };
            if let Some(disable_reason) = dmabuf_disable_reason(
                dmabuf_override.as_deref(),
                is_wsl,
                is_nvidia,
                has_render_node,
                using_x11,
                x11_has_dri3,
            ) {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
                disable_reason
            } else {
                "enabled"
            }
        };
        let _ = LINUX_DMABUF_REASON.set(reason);
    }

    let dmabuf_disabled = std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER")
        .is_some_and(|v| v == std::ffi::OsStr::new("1"));
    if should_demote_nvdec_plugins(
        std::env::var_os("GST_PLUGIN_FEATURE_RANK").as_deref(),
        dmabuf_disabled,
    ) {
        std::env::set_var(
            "GST_PLUGIN_FEATURE_RANK",
            "nvh264dec:NONE,nvh265dec:NONE,nvvp8dec:NONE,nvvp9dec:NONE,nvav1dec:NONE",
        );
    }
    if should_configure_cpu_fallback_tuning(
        dmabuf_disabled,
        std::env::var_os("WEBKIT_SKIA_CPU_PAINTING_THREADS").as_deref(),
    ) {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        std::env::set_var(
            "WEBKIT_SKIA_CPU_PAINTING_THREADS",
            recommended_skia_cpu_threads(cpus).to_string(),
        );
    }
    if should_configure_cpu_fallback_tuning(
        dmabuf_disabled,
        std::env::var_os("WEBKIT_FORCE_VBLANK_TIMER").as_deref(),
    ) {
        std::env::set_var("WEBKIT_FORCE_VBLANK_TIMER", "1");
    }
    if std::env::var_os("WEBKIT_GST_ALLOW_PLAYBACK_OF_INVISIBLE_VIDEOS").is_none() {
        std::env::set_var("WEBKIT_GST_ALLOW_PLAYBACK_OF_INVISIBLE_VIDEOS", "0");
    }
}

/// JS that pauses everything via the page helpers and returns a diagnostics report.
#[cfg(not(target_os = "linux"))]
const PAUSE_JS: &str = r#"(function(){
    if (window.__onWindowHidden) window.__onWindowHidden();
    return window.__tiktoknowPauseReport ? window.__tiktoknowPauseReport() : 'no-report';
})()"#;

#[cfg(all(debug_assertions, not(target_os = "linux")))]
const REPORT_JS: &str = r#"(function(){
    var _pickVideo = function() {
        var v = (window.__tiktoknowActiveVideo && window.__tiktoknowActiveVideo()) || null;
        if (!v) {
            var vs = document.querySelectorAll('video');
            for (var i = 0; i < vs.length; i++) { if (!vs[i].paused) { v = vs[i]; break; } }
            v = v || vs[0] || null;
        }
        return v;
    };
    return JSON.stringify({
        url: location.href,
        title: document.title,
        ready: document.readyState,
        hasPause: typeof window.__onWindowHidden,
        hasResume: typeof window.__resumeIfNeeded,
        hasReport: typeof window.__tiktoknowPauseReport,
        hasToast: typeof window.showToast,
        hasAutoScroll: typeof window.toggleAutoScroll,
        hasDarkCss: !!document.getElementById('tiktok-now-dark-canvas'),
        media: document.querySelectorAll('video, audio').length,
        mediaMuted: (function(){ var v = _pickVideo(); return v ? v.muted : null; })(),
        mediaVolume: (function(){ var v = _pickVideo(); return v ? v.volume : null; })(),
        mediaPaused: (function(){ var v = _pickVideo(); return v ? v.paused : null; })()
    });
})()"#;

#[cfg(windows)]
fn set_page_sleep(w: &tauri::WebviewWindow, sleeping: bool) {
    let _ = w.with_webview(move |webview| webview_sleep::set_sleeping(&webview, sleeping));
}

#[cfg(not(target_os = "linux"))]
fn pause_media_for_hidden(w: &tauri::WebviewWindow) {
    let w_sleep = w.clone();
    let _ = w.eval_with_callback(PAUSE_JS, move |_report| {
        diag!("[TikTok-Now] Watchdog pause report: {}", _report);
        #[cfg(windows)]
        if is_window_hidden_state(w_sleep.is_minimized().ok(), w_sleep.is_visible().ok()) {
            set_page_sleep(&w_sleep, true);
        }
        #[cfg(not(windows))]
        let _ = &w_sleep;
    });
    #[cfg(debug_assertions)]
    if !PAUSE_RECHECK_IN_FLIGHT.swap(true, Ordering::Relaxed) {
        let w2 = w.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(2500));
            let _ = w2.eval_with_callback(REPORT_JS, |report| {
                diag!("[TikTok-Now] Watchdog recheck: {}", report);
            });
            PAUSE_RECHECK_IN_FLIGHT.store(false, Ordering::Relaxed);
        });
    }
    #[cfg(windows)]
    if audio::sync(audio::Mode::Hidden) {
        diag!("[TikTok-Now] Watchdog: audio sessions muted");
    }
}

#[cfg(not(target_os = "linux"))]
fn resume_media_for_visible(w: &tauri::WebviewWindow) {
    #[cfg(windows)]
    set_page_sleep(w, false);
    let _ = w.eval("if (window.__resumeIfNeeded) window.__resumeIfNeeded();");
    #[cfg(debug_assertions)]
    let _ = w.eval_with_callback(REPORT_JS, |report| {
        diag!("[TikTok-Now] Watchdog visible report: {}", report);
    });
    #[cfg(windows)]
    if audio::sync(audio::Mode::Visible) {
        diag!("[TikTok-Now] Watchdog: audio sessions unmuted");
    }
}

fn parse_css_color_to_native(value: &str) -> Option<Color> {
    let v = value.trim().to_lowercase();
    if v.is_empty() || v == "transparent" {
        return None;
    }
    if let Some(hex) = v.strip_prefix('#') {
        if !hex.is_ascii() {
            return None;
        }
        return match hex.len() {
            3 => Some(Color(
                u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?,
                u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?,
                u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?,
                255,
            )),
            6 => Some(Color(
                u8::from_str_radix(&hex[0..2], 16).ok()?,
                u8::from_str_radix(&hex[2..4], 16).ok()?,
                u8::from_str_radix(&hex[4..6], 16).ok()?,
                255,
            )),
            _ => None,
        };
    }
    if v.starts_with("rgb") {
        let nums: Vec<u8> = v
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<u8>().ok())
            .collect();
        if nums.len() >= 3 {
            return Some(Color(nums[0], nums[1], nums[2], 255));
        }
    }
    if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == ',') {
        let parts: Vec<u8> = v.split(',').filter_map(|s| s.parse::<u8>().ok()).collect();
        if parts.len() == 3 {
            return Some(Color(parts[0], parts[1], parts[2], 255));
        }
    }
    None
}

fn apply_page_theme_background(window: &tauri::WebviewWindow, css_rgb: &str) {
    if let Some(color) = parse_css_color_to_native(css_rgb) {
        if window.set_background_color(Some(color)).is_ok() {
            diag!(
                "[TikTok-Now] Native background synced to page theme: #{:02x}{:02x}{:02x}",
                color.0,
                color.1,
                color.2
            );
        }
    }
}

/// `true` when the URL host is TikTok (`tiktok.com` and any subdomain).
fn is_tiktok_host(url: &Url) -> bool {
    match url.host_str() {
        Some(h) => {
            let lower = h.to_ascii_lowercase();
            lower == "tiktok.com" || lower.ends_with(".tiktok.com")
        }
        None => false,
    }
}

fn is_allowed_in_page_navigation(url: &Url) -> bool {
    if matches!(url.scheme(), "about" | "blob" | "data") {
        return true;
    }
    if url.scheme() != "http" && url.scheme() != "https" {
        return true;
    }
    if url.host_str().map(is_local_host).unwrap_or(false) || is_tiktok_host(url) {
        return true;
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if host == "accounts.google.com" && url.path().starts_with("/gsi/") {
        return true;
    }
    matches!(
        host.as_str(),
        "appleid.apple.com"
            | "appleid.cdn-apple.com"
            | "recaptcha.net"
            | "www.recaptcha.net"
            | "challenges.cloudflare.com"
    ) || host.ends_with(".recaptcha.net")
        || host.ends_with(".arkoselabs.com")
        || host.ends_with(".funcaptcha.com")
}

#[cfg(not(target_os = "linux"))]
fn watch_main_window(w: tauri::WebviewWindow) {
    let _guard = WatchdogGuard;

    #[cfg(windows)]
    let mut _audio_keepalive = audio::register_app_sessions();
    #[cfg(windows)]
    let mut last_endpoints = audio::active_render_endpoint_ids();

    let state_hidden = || is_window_hidden_state(w.is_minimized().ok(), w.is_visible().ok());
    let mut last_hidden = state_hidden();
    #[cfg(windows)]
    let mut last_foreground = !last_hidden && w.is_focused().unwrap_or(false);
    #[cfg(windows)]
    let mut audio_synced: Option<(bool, std::time::Instant)> = None;
    #[cfg(windows)]
    const VISIBLE_AUDIO_RECONCILE: std::time::Duration = std::time::Duration::from_secs(15);
    let mut startup_ticks = 0u32;
    let mut startup_reported = false;

    loop {
        let poll = if last_hidden { HIDDEN_POLL } else { VISIBLE_POLL };
        let pending_guard = WATCHDOG_WAKE
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (mut pending_guard, _timeout) = WATCHDOG_WAKE
            .1
            .wait_timeout_while(pending_guard, poll, |flag| !*flag)
            .unwrap_or_else(|e| e.into_inner());
        *pending_guard = false;
        drop(pending_guard);

        if w.app_handle()
            .get_webview_window(MAIN_WINDOW_LABEL)
            .is_none()
        {
            break;
        }

        let hidden = state_hidden();
        #[cfg(windows)]
        {
            let in_foreground = !hidden && w.is_focused().unwrap_or(false);
            let endpoints = audio::active_render_endpoint_ids();
            if endpoints != last_endpoints {
                _audio_keepalive = audio::register_app_sessions_with_mute(hidden);
                last_endpoints = endpoints;
            }
            let interval = if hidden {
                HIDDEN_POLL
            } else {
                VISIBLE_AUDIO_RECONCILE
            };
            let due = match audio_synced {
                Some((was_hidden, at)) => was_hidden != hidden || at.elapsed() >= interval,
                None => true,
            };
            if due {
                let mode = if hidden {
                    audio::Mode::Hidden
                } else {
                    audio::Mode::Visible
                };
                audio::sync(mode);
                audio_synced = Some((hidden, std::time::Instant::now()));
            }
            if in_foreground && audio::ensure_foreground_audio_ready() {
                diag!("[TikTok-Now] Watchdog: foreground audio session unmuted");
            }
            if in_foreground && !last_foreground {
                let _ = w.eval(
                    "if (window.__tiktoknowEnsureForegroundAudio) window.__tiktoknowEnsureForegroundAudio();",
                );
            }
            last_foreground = in_foreground;
        }

        if hidden && !last_hidden {
            diag!("[TikTok-Now] Watchdog: window hidden -> pausing media");
            pause_media_for_hidden(&w);
        } else if !hidden && last_hidden {
            diag!("[TikTok-Now] Watchdog: window visible -> resuming media");
            resume_media_for_visible(&w);
        }
        last_hidden = hidden;

        if !startup_reported {
            if hidden {
                startup_reported = true;
            } else {
                startup_ticks += 1;
                #[cfg(debug_assertions)]
                if startup_ticks >= 25 {
                    startup_reported = true;
                    let _ = w.eval_with_callback(REPORT_JS, |report| {
                        diag!("[TikTok-Now] Watchdog startup report: {}", report);
                    });
                    #[cfg(windows)]
                    audio::report_audio_state();
                }
                #[cfg(not(debug_assertions))]
                if startup_ticks >= 25 {
                    startup_reported = true;
                }
            }
        }
    }
}

fn launch_main_window(app: &AppHandle, start_minimized: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if !start_minimized {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        return Ok(());
    }

    let username_cache: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cache_for_title = username_cache.clone();
    let last_applied_title: Arc<Mutex<String>> = Arc::new(Mutex::new(String::from("TikTok-Now")));
    let title_guard = last_applied_title.clone();

    let app_handle = app.clone();
    let nav_app = app.clone();
    let new_win_app = app.clone();

    // On Linux/Wayland, creating a hidden GTK window and calling `.show()` later
    // can freeze input/decorations; show immediately unless `--minimized`.
    let start_visible_now = !start_minimized && cfg!(target_os = "linux");

    let window = WebviewWindowBuilder::new(
        app,
        MAIN_WINDOW_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("TikTok-Now")
    .inner_size(DEFAULT_INNER_SIZE.0, DEFAULT_INNER_SIZE.1)
    .min_inner_size(MIN_INNER_SIZE.0, MIN_INNER_SIZE.1)
    .shadow(false)
    .center()
    .visible(!start_minimized || start_visible_now)
    .background_color(Color(13, 14, 21, 255))
    .additional_browser_args(DEFAULT_WEBVIEW_BROWSER_ARGS)
    .on_navigation(move |url| {
        if is_allowed_in_page_navigation(url) {
            return true;
        }
        diag!(
            "[TikTok-Now] External link intercepted ({}), opening in OS default browser...",
            url
        );
        let _ = nav_app;
        let _ = commands::auth::open_safe_url_internal(url.as_str());
        false
    })
    .on_document_title_changed(move |window, page_title| {
        let app = window.app_handle().clone();
        let sync_cached_title = |win: &tauri::WebviewWindow| {
            let cached = cache_for_title
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let desired =
                cached.map_or_else(|| "TikTok-Now".to_string(), |u| format!("TikTok-Now (@{u})"));
            let mut last = title_guard.lock().unwrap_or_else(|e| e.into_inner());
            if *last != desired && win.set_title(&desired).is_ok() {
                *last = desired;
            }
        };

        if let Some(rgb) = page_title.strip_prefix("TTNOWBG:") {
            apply_page_theme_background(&window, rgb);
            sync_cached_title(&window);
            return;
        }

        if page_title.contains("LOGGED_IN") || page_title.starts_with("TIKTOKNOW:") {
            let user = page_title
                .trim_start_matches("TIKTOKNOW:")
                .trim_start_matches("TIKTOKNOW_LOGGED_IN")
                .trim()
                .to_string();
            let is_pure_numeric = !user.is_empty() && user.chars().all(|c| c.is_ascii_digit());

            if !user.is_empty() && !is_pure_numeric {
                *cache_for_title.lock().unwrap_or_else(|e| e.into_inner()) = Some(user.clone());
            }

            let mut closed_any = false;
            let popup_labels: Vec<String> = app
                .webview_windows()
                .keys()
                .filter(|label| label.starts_with("popup-"))
                .cloned()
                .collect();
            for label in popup_labels {
                if let Some(popup_win) = app.get_webview_window(&label) {
                    diag!(
                        "[TikTok-Now] Logged-in toast/title detected. Closing popup window '{}' immediately!",
                        label
                    );
                    let _ = popup_win.close();
                    closed_any = true;
                }
            }

            if closed_any {
                let _ = window.eval("window.location.reload();");
            }

            sync_cached_title(&window);
        } else if !page_title.starts_with("TIKTOKNOW:")
            && !page_title.starts_with("TIKTOKNOW_LOGGED_IN")
        {
            let cached = cache_for_title
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if cached.is_some() {
                sync_cached_title(&window);
            } else {
                let _ = window.eval(TITLE_JS);
            }
        }
    })
    .initialization_script(INIT_JS)
    .on_new_window(move |url, features| {
        let host = url.host_str().unwrap_or("");
        let path = url.path();
        let is_tiktok = is_tiktok_host(&url);
        let is_oauth_provider_host = host == "accounts.google.com"
            || host.ends_with(".google.com")
            || host == "google.com"
            || host == "appleid.apple.com"
            || host.ends_with(".apple.com")
            || host == "facebook.com"
            || host.ends_with(".facebook.com")
            || host == "twitter.com"
            || host.ends_with(".twitter.com")
            || host == "x.com"
            || host.ends_with(".x.com")
            || host == "microsoft.com"
            || host.ends_with(".microsoft.com")
            || host == "live.com"
            || host.ends_with(".live.com")
            || host == "login.microsoftonline.com"
            || host.ends_with(".login.microsoftonline.com");
        let is_login_path = path.contains("login")
            || path.contains("auth")
            || path.contains("sso")
            || path.contains("passport");
        let is_auth_provider =
            url.as_str() == "about:blank" || is_oauth_provider_host || (is_tiktok && is_login_path);

        if is_auth_provider {
            #[cfg(not(windows))]
            {
                let _ = (&features, &app_handle, &new_win_app);
                diag!("[TikTok-Now] Allowing native related-view OAuth popup: {}", url);
                NewWindowResponse::Allow
            }
            #[cfg(windows)]
            {
                let label = format!("popup-{}", POPUP_COUNTER.fetch_add(1, Ordering::Relaxed));
                diag!("[TikTok-Now] Intercepted OAuth login window: {}", url);

                let (w, h) = features
                    .size()
                    .map(|s| (s.width, s.height))
                    .unwrap_or((540.0, 700.0));
                let env = features.opener().environment.clone();

                let app_close = app_handle.clone();
                let label_close = label.clone();
                let flag_nav = Arc::new(AtomicBool::new(false));

                let builder = WebviewWindowBuilder::new(
                    &app_handle,
                    label,
                    WebviewUrl::External(url.clone()),
                )
                .title("TikTok-Now — Sign in")
                .inner_size(w.max(500.0), h.max(600.0))
                .center()
                .background_color(Color(13, 14, 21, 255))
                .initialization_script(POPUP_INIT_JS)
                .on_navigation(move |nav_url| {
                    let nav_path = nav_url.path();
                    let is_tiktok_nav = is_tiktok_host(nav_url);
                    let is_login_selection = nav_path == "/login" || nav_path == "/login/";

                    if is_tiktok_nav && !is_login_selection && !flag_nav.load(Ordering::Relaxed) {
                        flag_nav.store(true, Ordering::Relaxed);
                        let app = app_close.clone();
                        let label = label_close.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            if let Some(popup) = app.get_webview_window(&label) {
                                let _ = popup.close();
                            }
                            if let Some(main) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                                let _ = main.show();
                                let _ = main.set_focus();
                                let _ = main.eval("window.location.reload();");
                            }
                        });
                    }
                    true
                })
                .with_environment(env);

                match builder.build() {
                    Ok(popup) => NewWindowResponse::Create { window: popup },
                    Err(_) => NewWindowResponse::Allow,
                }
            }
        } else if !is_tiktok {
            diag!(
                "[TikTok-Now] External hyperlink in new window ({}), launching OS default browser...",
                url
            );
            let _ = (&new_win_app, commands::auth::open_safe_url_internal(url.as_str()));
            NewWindowResponse::Deny
        } else {
            NewWindowResponse::Allow
        }
    })
    .build()
    .map_err(|e| format!("Failed to build TikTok-Now main window: {e}"))?;

    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::{
            CacheModel, HardwareAccelerationPolicy, SettingsExt, WebContextExt, WebViewExt,
        };
        let _ = window.with_webview(move |platform_webview| {
            let wv = platform_webview.inner();
            if let Some(settings) = wv.settings() {
                settings.set_enable_smooth_scrolling(true);
                settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Always);
            }
            if let Some(ctx) = wv.context() {
                if ctx.cache_model() != CacheModel::WebBrowser {
                    ctx.set_cache_model(CacheModel::WebBrowser);
                }
            }
        });
    }

    {
        let w = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            for _ in 0..120 {
                if w.app_handle()
                    .get_webview_window(MAIN_WINDOW_LABEL)
                    .is_none()
                {
                    return;
                }
                if w.is_visible().unwrap_or(false) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            fit_window_to_work_area(&w);
        });
    }

    if !WATCHDOG_STARTED.swap(true, Ordering::Relaxed) {
        #[cfg(not(target_os = "linux"))]
        {
            let w = window.clone();
            std::thread::spawn(move || watch_main_window(w));
        }
    }

    Ok(())
}

// ── Downloader IPC & Media Helpers ──────────────────────────────────────────

#[tauri::command]
fn prepare_download_folder(app: AppHandle) -> Result<String, String> {
    downloader::prepare_download_folder(&app).map(|dir| dir.to_string_lossy().into_owned())
}

#[tauri::command]
fn open_downloads_folder(app: AppHandle) -> Result<(), String> {
    downloader::open_downloads_folder(&app)
}

fn downloader_profile_path(app_local_data_dir: &Path) -> PathBuf {
    app_local_data_dir.join("downloader-profile")
}

fn downloader_data_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve local app data directory for downloader: {e}"))?;
    let dir = downloader_profile_path(&base);
    fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "Failed to create downloader WebView profile directory '{}': {e}",
            dir.display()
        )
    })?;
    Ok(dir)
}

pub(crate) fn open_downloader_window_internal(
    app: &AppHandle,
    url: Option<String>,
    payload_json: Option<String>,
    item_index: Option<usize>,
    item_hint: Option<String>,
    theme: Option<String>,
) -> Result<(), String> {
    let clean_url = url.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let clean_payload = payload_json
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let clean_hint = item_hint
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let clean_theme = theme
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if clean_url.is_some()
        || clean_payload.is_some()
        || item_index.is_some()
        || clean_hint.is_some()
        || clean_theme.is_some()
    {
        let probe_state = app.state::<downloader::DownloaderProbeState>();
        probe_state.set_pending_open(Some(downloader::PendingDownloadOpen {
            url: clean_url.unwrap_or_default(),
            payload_json: clean_payload,
            item_index,
            item_hint: clean_hint,
            theme: clean_theme,
        }));
    }

    let main_monitor = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .and_then(|m| m.current_monitor().ok().flatten());

    if let Some(window) = app.get_webview_window(DOWNLOADER_WINDOW_LABEL) {
        let hidden_or_min =
            !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false);
        let _ = window.unminimize();
        if hidden_or_min {
            fit_window_to_work_area_on_monitor(
                &window,
                main_monitor,
                DOWNLOADER_WINDOW_WIDTH,
                DOWNLOADER_WINDOW_HEIGHT,
                DOWNLOADER_MIN_WIDTH,
                DOWNLOADER_MIN_HEIGHT,
            );
        }
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.eval(
            "if (window.__tiktoknowLoadDownloaderContext) window.__tiktoknowLoadDownloaderContext();",
        );
        return Ok(());
    }

    // Isolate Downloader in its own WebView2 data directory while matching
    // DEFAULT_WEBVIEW_BROWSER_ARGS so it never collides with "main"'s default
    // userDataFolder (which preserves existing TikTok login cookies).
    let downloader_data_dir = downloader_data_directory(app)?;
    let dl_visible_now = cfg!(target_os = "linux");
    let _built_dl_win = WebviewWindowBuilder::new(
        app,
        DOWNLOADER_WINDOW_LABEL,
        WebviewUrl::App("downloader.html".into()),
    )
    .title("TikTok-Now Downloader")
    .inner_size(DOWNLOADER_WINDOW_WIDTH, DOWNLOADER_WINDOW_HEIGHT)
    .min_inner_size(DOWNLOADER_MIN_WIDTH, DOWNLOADER_MIN_HEIGHT)
    .resizable(true)
    .center()
    .visible(dl_visible_now)
    .background_color(Color(13, 14, 21, 255))
    .data_directory(downloader_data_dir)
    .additional_browser_args(DEFAULT_WEBVIEW_BROWSER_ARGS)
    .build()
    .map_err(|e| format!("Failed to build TikTok-Now Downloader window: {e}"))?;

    let Some(dl_win) = app.get_webview_window(DOWNLOADER_WINDOW_LABEL) else {
        return Err(
            "TikTok-Now Downloader window failed to initialize (missing after build)."
                .to_string(),
        );
    };
    #[cfg(windows)]
    dl_win
        .hwnd()
        .map_err(|e| format!("TikTok-Now Downloader window has no native HWND after build: {e}"))?;

    fit_window_to_work_area_on_monitor(
        &dl_win,
        main_monitor,
        DOWNLOADER_WINDOW_WIDTH,
        DOWNLOADER_WINDOW_HEIGHT,
        DOWNLOADER_MIN_WIDTH,
        DOWNLOADER_MIN_HEIGHT,
    );
    if !dl_visible_now {
        let _ = dl_win.show();
        let _ = dl_win.set_focus();
    }
    Ok(())
}

#[tauri::command]
async fn open_downloader_window(
    app: AppHandle,
    url: Option<String>,
    payload_json: Option<String>,
    item_index: Option<usize>,
    item_hint: Option<String>,
    theme: Option<String>,
) -> Result<(), String> {
    open_downloader_window_internal(&app, url, payload_json, item_index, item_hint, theme)
}

#[tauri::command]
fn take_pending_downloader_context(
    state: tauri::State<'_, downloader::DownloaderProbeState>,
) -> Option<downloader::PendingDownloadOpen> {
    state.take_pending_open()
}

#[tauri::command]
fn submit_downloader_probe(
    state: tauri::State<'_, downloader::DownloaderProbeState>,
    request_id: String,
    payload_json: String,
) -> bool {
    state.resolve_probe(&request_id, payload_json)
}

#[tauri::command]
async fn inspect_tiktok_post(
    app: AppHandle,
    probe_state: tauri::State<'_, downloader::DownloaderProbeState>,
    url: String,
    seed_payload_json: Option<String>,
) -> Result<downloader::InspectedPost, String> {
    downloader::inspect_tiktok_post_http(
        &app,
        &probe_state,
        MAIN_WINDOW_LABEL,
        url,
        seed_payload_json,
    )
    .await
}

fn sanitize_header_value(raw: &str) -> String {
    raw.chars()
        .filter(|&c| c != '\r' && c != '\n' && c != '\0')
        .collect()
}

fn resolve_cookie_header(app: &AppHandle, fallback_cookie: Option<&str>) -> Option<String> {
    downloader::tiktok_auth_cookies(app, MAIN_WINDOW_LABEL).or_else(|| {
        fallback_cookie
            .map(sanitize_header_value)
            .filter(|s| !s.is_empty())
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn download_media(
    app: AppHandle,
    url: String,
    media_type: String,
    referer: Option<String>,
    subfolder: Option<String>,
    file_stem: Option<String>,
    cookies: Option<String>,
    user_agent: Option<String>,
    audio_url: Option<String>,
) -> Result<String, String> {
    let cookie_header = resolve_cookie_header(&app, cookies.as_deref());
    downloader::download_media_url(
        &app,
        url,
        media_type,
        referer,
        subfolder,
        file_stem,
        cookie_header,
        user_agent,
        audio_url,
    )
    .await
}

#[tauri::command]
async fn save_media_bytes(
    app: AppHandle,
    data: Vec<u8>,
    media_type: String,
    subfolder: Option<String>,
    file_stem: Option<String>,
) -> Result<String, String> {
    downloader::save_media_bytes(&app, data, media_type, subfolder, file_stem).await
}

#[tauri::command]
async fn save_caption_file(
    app: AppHandle,
    post: downloader::InspectedPost,
    custom_caption: Option<String>,
    subfolder: Option<String>,
    file_stem: Option<String>,
) -> Result<String, String> {
    downloader::save_caption_file(&app, post, custom_caption, subfolder, file_stem).await
}

#[tauri::command]
async fn download_post_selection(
    app: AppHandle,
    post: downloader::InspectedPost,
    item_index: usize,
    format_id: Option<String>,
    mode: String,
    custom_caption: Option<String>,
) -> Result<downloader::DownloadOutcome, String> {
    let cookie_header = downloader::tiktok_auth_cookies(&app, MAIN_WINDOW_LABEL);
    downloader::download_post_selection(
        &app,
        post,
        item_index,
        format_id,
        mode,
        custom_caption,
        cookie_header,
        Some(DOWNLOADER_WINDOW_LABEL),
    )
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(downloader::DownloaderProbeState::default())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            #[cfg(windows)]
            if let Ok(data_dir) = app.path().app_data_dir() {
                let _ = fs::create_dir_all(&data_dir);
                audio::init(&data_dir);
            }

            if let Err(_e) = tray::setup_tray(app.handle()) {
                diag!("[TikTok-Now] Tray setup failed: {}", _e);
            }

            let start_minimized = std::env::args().any(|arg| arg == "--minimized");
            if let Err(_e) = launch_main_window(app.handle(), start_minimized) {
                diag!("[TikTok-Now] Failed to launch main window: {}", _e);
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if window.label() == MAIN_WINDOW_LABEL {
                    api.prevent_close();
                    wake_watchdog();
                    if let Some(main) = window.get_webview_window(MAIN_WINDOW_LABEL) {
                        let _ = main.eval("if (window.__onWindowHidden) window.__onWindowHidden();");
                        let _ = main.hide();
                    }
                }
            }
            tauri::WindowEvent::Resized(_) => {
                if window.label() == MAIN_WINDOW_LABEL {
                    wake_watchdog();
                }
            }
            tauri::WindowEvent::Focused(true) if window.label() == MAIN_WINDOW_LABEL => {
                wake_watchdog();
                if window.is_visible().unwrap_or(false)
                    && !window.is_minimized().unwrap_or(false)
                {
                    #[cfg(windows)]
                    let _ = audio::ensure_foreground_audio_ready();
                    if let Some(main) = window.get_webview_window(MAIN_WINDOW_LABEL) {
                        #[cfg(target_os = "linux")]
                        let _ = main.eval(
                            "if (window.__resumeIfNeeded) window.__resumeIfNeeded(); if (window.__tiktoknowEnsureForegroundAudio) window.__tiktoknowEnsureForegroundAudio();",
                        );
                        #[cfg(not(target_os = "linux"))]
                        let _ = main.eval(
                            "if (window.__tiktoknowEnsureForegroundAudio) window.__tiktoknowEnsureForegroundAudio();",
                        );
                    }
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::jump_to_external,
            commands::auth::open_external_url,
            prepare_download_folder,
            open_downloads_folder,
            download_media,
            save_media_bytes,
            open_downloader_window,
            take_pending_downloader_context,
            inspect_tiktok_post,
            submit_downloader_probe,
            download_post_selection,
            save_caption_file
        ])
        .build(tauri::generate_context!())
        .expect(concat!(
            "error while running TikTok-Now v",
            env!("CARGO_PKG_VERSION")
        ))
        .run(|app_handle, event| {
            match event {
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen {
                    has_visible_windows: false,
                    ..
                } => {
                    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    } else {
                        let _ = launch_main_window(app_handle, false);
                    }
                }
                tauri::RunEvent::Exit => {
                    #[cfg(windows)]
                    audio::sync(audio::Mode::Release);
                }
                _ => {
                    let _ = app_handle;
                }
            }
        });
}

/// Local-origin recognition shared by navigation interceptors.
fn is_local_host(host: &str) -> bool {
    host == "localhost"
        || host == "tauri.localhost"
        || host.ends_with(".localhost")
        || host == "127.0.0.1"
        || host == "[::1]"
        || host == "ipc.localhost"
}

const TITLE_JS: &str = r#"
    (function() {
        try {
            function getLoggedInHandle() {
                try {
                    var uData = window.__UNIVERSAL_DATA_FOR_REHYDRATION__;
                    if (uData && uData.__DEFAULT_SCOPE__) {
                        var s = uData.__DEFAULT_SCOPE__;
                        if (s['webapp.app-context'] && s['webapp.app-context'].user && s['webapp.app-context'].user.uniqueId) {
                            var u1 = s['webapp.app-context'].user.uniqueId;
                            if (u1 && !/^\d+$/.test(u1)) return u1;
                        }
                        if (s['webapp.user-detail'] && s['webapp.user-detail'].userInfo && s['webapp.user-detail'].userInfo.user && s['webapp.user-detail'].userInfo.user.uniqueId) {
                            var u2 = s['webapp.user-detail'].userInfo.user.uniqueId;
                            if (u2 && !/^\d+$/.test(u2)) return u2;
                        }
                    }
                } catch(e1) {}

                try {
                    if (window.SIGI_STATE && window.SIGI_STATE.AppContext && window.SIGI_STATE.AppContext.user && window.SIGI_STATE.AppContext.user.uniqueId) {
                        var u3 = window.SIGI_STATE.AppContext.user.uniqueId;
                        if (u3 && !/^\d+$/.test(u3)) return u3;
                    }
                } catch(e2) {}

                try {
                    if (window.__INITIAL_STATE__ && window.__INITIAL_STATE__.appContext && window.__INITIAL_STATE__.appContext.user && window.__INITIAL_STATE__.appContext.user.uniqueId) {
                        var u4 = window.__INITIAL_STATE__.appContext.user.uniqueId;
                        if (u4 && !/^\d+$/.test(u4)) return u4;
                    }
                } catch(e3) {}

                var navProfile = document.querySelector('a[data-e2e="nav-profile"]') ||
                                 document.querySelector('a[data-e2e="profile-icon"]') ||
                                 document.querySelector('a[data-e2e="user-profile"]') ||
                                 document.querySelector('a[href^="/@"][class*="SideNav"]') ||
                                 document.querySelector('[class*="HeaderRight"] a[href*="/@"]') ||
                                 document.querySelector('[class*="DivHeaderRight"] a[href*="/@"]');
                if (navProfile) {
                    var href = navProfile.getAttribute('href') || '';
                    var m = href.match(/\/@([a-zA-Z0-9_\.]+)/);
                    if (m && m[1]) {
                        var handle = m[1].replace(/\/$/, '');
                        if (!/^\d+$/.test(handle)) return handle;
                    }
                }

                if (location.pathname.indexOf('/@') === 0) {
                    var isMyProfile = document.querySelector('[data-e2e="edit-profile-entrance"]') ||
                                      Array.from(document.querySelectorAll('button')).some(function(b) {
                                          return b.textContent && b.textContent.indexOf('Edit profile') !== -1;
                                      });
                    if (isMyProfile) {
                        var pathHandle = location.pathname.split('/@')[1].split('/')[0];
                        if (pathHandle && !/^\d+$/.test(pathHandle)) return pathHandle;
                    }
                }
                return null;
            }

            var h = getLoggedInHandle();
            if (h) {
                document.title = 'TIKTOKNOW:' + h;
            }
        } catch(e) {}
    })();
"#;

#[cfg(windows)]
const POPUP_INIT_JS: &str = r#"
    (function() {
        'use strict';
        function applyDark() {
            try {
                var root = document.head || document.documentElement;
                if (root && !document.getElementById('tiktok-now-dark-canvas')) {
                    var darkStyle = document.createElement('style');
                    darkStyle.id = 'tiktok-now-dark-canvas';
                    darkStyle.textContent = 'html, body { background-color: #0d0e15 !important; color: #fff !important; }';
                    root.appendChild(darkStyle);
                }
                if (document.documentElement) {
                    document.documentElement.style.backgroundColor = '#0d0e15';
                }
            } catch (e) {}
        }
        if (document.documentElement || document.head) {
            applyDark();
        } else {
            var popupDarkObserver = new MutationObserver(function () {
                if (document.documentElement || document.head) {
                    popupDarkObserver.disconnect();
                    applyDark();
                }
            });
            popupDarkObserver.observe(document, { childList: true, subtree: true });
            applyDark();
        }
        setInterval(function() {
            try {
                var isTikTok = location.hostname === 'tiktok.com' || location.hostname.endsWith('.tiktok.com');
                var isLoginSelection = location.pathname === '/login' || location.pathname === '/login/';
                if (isTikTok && !isLoginSelection) {
                    window.close();
                }
            } catch (e) {}
        }, 300);
    })();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_css_color_handles_hex_rgb_and_ttnowbg_triples() {
        assert_eq!(parse_css_color_to_native("#0d0e15"), Some(Color(13, 14, 21, 255)));
        assert_eq!(parse_css_color_to_native("#fff"), Some(Color(255, 255, 255, 255)));
        assert_eq!(parse_css_color_to_native("rgb(18, 19, 24)"), Some(Color(18, 19, 24, 255)));
        assert_eq!(parse_css_color_to_native("13,14,21"), Some(Color(13, 14, 21, 255)));
        assert_eq!(parse_css_color_to_native("transparent"), None);
    }

    #[test]
    fn unsupported_minimized_query_does_not_force_window_hidden() {
        assert!(!is_window_hidden_state(None, Some(true)));
        assert!(is_window_hidden_state(Some(true), Some(true)));
        assert!(is_window_hidden_state(Some(false), Some(false)));
        assert!(is_window_hidden_state(None, None));
    }

    #[test]
    fn navigation_allowlist_permits_tiktok_local_and_auth_iframes_only() {
        assert!(is_allowed_in_page_navigation(&Url::parse("https://www.tiktok.com/@user/video/123").unwrap()));
        assert!(is_allowed_in_page_navigation(&Url::parse("https://accounts.google.com/gsi/iframe/select").unwrap()));
        assert!(!is_allowed_in_page_navigation(&Url::parse("https://example.com/phish").unwrap()));
    }

    #[test]
    fn sanitize_path_segment_protects_traversal_and_reserved_names() {
        assert_eq!(downloader::sanitize_path_segment("../../etc/passwd"), "etc_passwd");
        assert_eq!(downloader::sanitize_path_segment("CON"), "_CON");
        assert_eq!(downloader::sanitize_path_segment(".."), "");
    }

    #[test]
    fn downloader_profile_path_isolates_subdir() {
        let base = Path::new("app-local-data");
        assert_eq!(
            downloader_profile_path(base),
            PathBuf::from("app-local-data").join("downloader-profile")
        );
    }
}
