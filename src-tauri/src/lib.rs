#[cfg(windows)]
mod audio;
mod commands;
mod error;
mod geometry;
mod tray;

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
use tauri::{window::Color, webview::NewWindowResponse, Manager};

use geometry::{DEFAULT_INNER_SIZE, MIN_INNER_SIZE};

static POPUP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// v2.1.0: diagnostics (`eprintln!` watchdog/audio traces) are debug-build-only.
/// Release builds no longer stream a forensic log of the user's browsing
/// (URLs, handles, playback state) to stderr.
#[cfg(debug_assertions)]
macro_rules! diag {
    ($($arg:tt)*) => { eprintln!($($arg)*) };
}
#[cfg(not(debug_assertions))]
macro_rules! diag {
    ($($arg:tt)*) => {};
}

/// JS that pauses everything via the page helpers and returns a diagnostics report.
const PAUSE_JS: &str = r#"(function(){
    if (window.__onWindowHidden) window.__onWindowHidden();
    return window.__tiktoknowPauseReport ? window.__tiktoknowPauseReport() : 'no-report';
})()"#;

/// JS that returns the current page-state diagnostics (for re-checks).
/// NOTE: must be an IIFE — WebView2's ExecuteScript rejects top-level `return`.
/// v2.1.0: only built into debug binaries — release builds produce no
/// page-state JSON of the user's session at all.
#[cfg(debug_assertions)]
const REPORT_JS: &str = r#"(function(){
    // Pick the AUDIBLE video: the observer's active video, else the first
    // currently-playing one, else the first video element. Sampling only the
    // first element was misleading — TikTok preloads neighbors, and the first
    // element may not be the one that is actually playing.
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
        mediaPaused: (function(){ var v = _pickVideo(); return v ? v.paused : null; })(),
        carousel: (function(){
            var n = document.querySelector('[data-e2e="arrow-next"]');
            var p = document.querySelector('[data-e2e="arrow-prev"]');
            if (!n || !p) return null;
            function vis(el) {
                try {
                    var s = getComputedStyle(el);
                    var r = el.getBoundingClientRect();
                    return s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0;
                } catch (e) { return true; }
            }
            return { nextVisible: vis(n), prevVisible: vis(p) };
        })()
    });
})()"#;

/// Pause playback when the window becomes hidden/minimized. Layered:
/// page-side pause + TikTok state-sync click + mute (in `__onWindowHidden`),
/// then a hard mute of this app's Windows audio sessions as a guarantee.
fn pause_media_for_hidden(w: &tauri::WebviewWindow) {
    let _ = w.eval_with_callback(PAUSE_JS, |_report| {
        diag!("[TikTok-Now] Watchdog pause report: {}", _report);
    });
    // Re-check a moment later: if currentTime advanced, some engine re-played it.
    #[cfg(debug_assertions)]
    {
        let w2 = w.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(2500));
            let _ = w2.eval_with_callback(REPORT_JS, |report| {
                diag!("[TikTok-Now] Watchdog recheck: {}", report);
            });
        });
    }
    pause_os_backup(true);
}

/// OS-level audio-session mute backup (Windows). `true` = mute (hidden),
/// `false` = unmute (visible). Covers this process and all its WebView2
/// children, so no window parameter is needed.
#[cfg(windows)]
fn pause_os_backup(mute: bool) {
    if audio::set_app_audio_mute(mute) {
        diag!(
            "[TikTok-Now] Watchdog: audio sessions {}",
            if mute { "muted" } else { "unmuted" }
        );
    } else if mute {
        diag!("[TikTok-Now] Watchdog: no audio session found to mute");
    }
}

#[cfg(not(windows))]
fn pause_os_backup(_mute: bool) {}

/// Resume playback when the window becomes visible again.
fn resume_media_for_visible(w: &tauri::WebviewWindow) {
    let _ = w.eval("if (window.__resumeIfNeeded) window.__resumeIfNeeded();");
    #[cfg(debug_assertions)]
    let _ = w.eval_with_callback(REPORT_JS, |report| {
        diag!("[TikTok-Now] Watchdog visible report: {}", report);
    });
    pause_os_backup(false);
}

/// `true` when the URL host is TikTok (www.tiktok.com, and every subdomain).
fn is_tiktok_host(url: &tauri::Url) -> bool {
    match url.host_str() {
        Some(h) => h == "tiktok.com" || h.ends_with(".tiktok.com"),
        None => false,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            if let Err(_e) = tray::setup_tray(app.handle()) {
                diag!("[TikTok-Now] Tray setup failed: {}", _e);
            }

            let username_cache: Arc<std::sync::Mutex<Option<String>>> =
                Arc::new(std::sync::Mutex::new(None));
            let cache_for_title = username_cache.clone();

            let app_handle = app.handle().clone();

            // Launch hidden to tray when started by the OS autostart feature.
            let start_minimized = std::env::args().any(|arg| arg == "--minimized");

            // ── Main Window with Native Dark Canvas Background (#0D0E15) ─────────
            let _window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("TikTok-Now")
            // v2.1.0: 1326 × 1032 — exactly fits the visible frame between the
            // screen top and the Windows taskbar on the reference machine
            // (1920×1080, 100% DPI). `fit_window_to_work_area` below refines
            // this with the REAL measured work area, so other machines get
            // their own exact fit instead of a stale hardcoded value.
            .inner_size(DEFAULT_INNER_SIZE.0, DEFAULT_INNER_SIZE.1)
            .min_inner_size(MIN_INNER_SIZE.0, MIN_INNER_SIZE.1)
            .shadow(false)
            .center()
            .visible(!start_minimized)
            .background_color(Color(13, 14, 21, 255))
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            )
            // ── Enable WebView2 Translate-to-English bar on Windows ───────────────
            // --autoplay-policy=no-user-gesture-required: let the first video autoplay
            // WITH sound (WebView2 otherwise blocks audible autoplay until a user
            // gesture, so TikTok starts muted on every fresh launch).
            .additional_browser_args(
                "--enable-features=msTranslate \
                 --disable-features=msSmartScreenProtection \
                 --no-first-run \
                 --autoplay-policy=no-user-gesture-required",
            )
            .on_navigation(|url| {
                let scheme = url.scheme();

                if scheme != "http" && scheme != "https" {
                    return true;
                }

                if url.host_str().map(is_local_host).unwrap_or(false) {
                    return true;
                }

                if is_tiktok_host(url) {
                    return true;
                }

                diag!(
                    "[TikTok-Now] External link intercepted ({}), opening in OS default browser...",
                    url
                );
                let _ = open::that(url.as_str());
                false
            })
            // ── Titlebar Username & "Logged in" Toast Auto-Close Popup Sweeper ──
            .on_document_title_changed(move |window, page_title| {
                let app = window.app_handle().clone();
                if page_title.contains("LOGGED_IN") || page_title.starts_with("TIKTOKNOW:") {
                    let user = page_title
                        .trim_start_matches("TIKTOKNOW:")
                        .trim_start_matches("TIKTOKNOW_LOGGED_IN")
                        .to_string();
                    let is_pure_numeric = !user.is_empty() && user.chars().all(|c| c.is_ascii_digit());

                    if !user.is_empty() && !is_pure_numeric {
                        *cache_for_title.lock().unwrap() = Some(user.clone());
                    }

                    // Immediately close ALL open popup windows upon "Logged in" toast detection!
                    let mut closed_any = false;
                    for (label, popup_win) in app.webview_windows() {
                        if label.starts_with("popup-") {
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

                    let cached = cache_for_title.lock().unwrap().clone();
                    let new_title = if let Some(u) = cached {
                        format!("TikTok-Now (@{})", u)
                    } else if !user.is_empty() && !is_pure_numeric {
                        format!("TikTok-Now (@{})", user)
                    } else {
                        "TikTok-Now".to_string()
                    };
                    let _ = window.set_title(&new_title);
                } else if !page_title.starts_with("TIKTOKNOW:")
                    && !page_title.starts_with("TIKTOKNOW_LOGGED_IN")
                {
                    let cached = cache_for_title.lock().unwrap().clone();
                    if let Some(u) = cached {
                        let _ = window.set_title(&format!("TikTok-Now (@{})", u));
                    } else {
                        let _ = window.eval(TITLE_JS);
                    }
                }
            })
            // ── Injected Instant Dark Background & Desktop Engine ────────────────
            .initialization_script(INIT_JS)
            // ── Clean OAuth Popup Window Handler & External Hyperlink Router ─────
            .on_new_window(move |url, features| {
                let host = url.host_str().unwrap_or("");
                let path = url.path();
                let is_tiktok = is_tiktok_host(&url);
                // v2.1.0 bug fix: the old code matched `host.contains("live")`
                // — a SUBSTRING match that treated any hostname containing the
                // letters "live" (liveweb.example.com, alivereports.io, ...)
                // as an auth provider and hijacked it into a sign-in popup.
                // Only known OAuth providers are matched now, with exact
                // hostnames or subdomains of the real provider domains.
                // v2.1.0 audit round 2: the URL-PATH heuristics (login/auth/
                // sso/passport) are now TikTok-ONLY as well — previously any
                // third-party URL with "auth" or "sso" anywhere in its path
                // (e.g. an analytics site's /auth0-dashboard page) was
                // hijacked into an in-app popup instead of the OS browser.
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
                let is_auth_provider = is_oauth_provider_host || (is_tiktok && is_login_path);

                if is_auth_provider {
                    let label = format!("popup-{}", POPUP_COUNTER.fetch_add(1, Ordering::Relaxed));
                    diag!("[TikTok-Now] Intercepted OAuth login window: {}", url);

                    let (w, h) = features.size().map(|s| (s.width, s.height)).unwrap_or((540.0, 700.0));

                    #[cfg(windows)]
                    let env = features.opener().environment.clone();

                    let app_close = app_handle.clone();
                    let label_close = label.clone();
                    let flag_nav = Arc::new(AtomicBool::new(false));

                    let mut builder = tauri::WebviewWindowBuilder::new(
                        &app_handle,
                        label,
                        tauri::WebviewUrl::External(url.clone()),
                    )
                    .title("TikTok-Now — Sign in")
                    .inner_size(w.max(500.0), h.max(600.0))
                    .center()
                    .background_color(Color(13, 14, 21, 255))
                    .user_agent(
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                         (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
                    )
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
                                if let Some(main) = app.get_webview_window("main") {
                                    let _ = main.show();
                                    let _ = main.set_focus();
                                    let _ = main.eval("window.location.reload();");
                                }
                            });
                        }
                        true
                    });

                    #[cfg(windows)]
                    {
                        builder = builder.with_environment(env);
                    }

                    match builder.build() {
                        Ok(popup) => NewWindowResponse::Create { window: popup },
                        Err(_) => NewWindowResponse::Allow,
                    }
                } else if !is_tiktok {
                    diag!(
                        "[TikTok-Now] External hyperlink in new window ({}), launching OS default browser...",
                        url
                    );
                    let _ = open::that(url.as_str());
                    NewWindowResponse::Deny
                } else {
                    NewWindowResponse::Allow
                }
            })
            .build()
            .expect("Failed to build TikTok-Now main window");

            // v2.1.0: exact work-area fit (1326 × 1032 on the reference
            // machine) — applied after the window exists, measuring the REAL
            // work area instead of trusting the hardcoded values.
            // The fit waits for the window to be VISIBLE before measuring:
            // DWM frame bounds on never-shown windows (the `--minimized`
            // autostart path) are unreliable, so hidden starts are fitted the
            // moment the user restores from the tray. Normal launches fit
            // within the first ~100 ms — while the splash is still painting
            // its first frame — so the settle is imperceptible.
            {
                let w = _window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    for _ in 0..120 {
                        if w.is_visible().unwrap_or(false) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                    geometry::fit_window_to_work_area(&w);
                });
            }

            // ── Playback Watchdog ─────────────────────────────────────────────
            // Some WebView2/WebKit builds never fire `visibilitychange` (or a resize
            // event) for a minimized window, so the page would keep playing audio in
            // the background. This thread polls the REAL window state and drives the
            // page helpers on every hidden/visible transition. Cheap: two state reads
            // every 800 ms, and the page eval only runs on a state CHANGE.
            {
                let w = _window.clone();
                std::thread::spawn(move || {
                    // Fail-safe direction: if a state query errors (e.g. the window was
                    // destroyed), treat the window as HIDDEN so playback gets paused.
                    let state_hidden = || {
                        w.is_minimized().unwrap_or(true) || !w.is_visible().unwrap_or(false)
                    };
                    let mut last_hidden = state_hidden();
                    #[cfg(windows)]
                    let mut unmute_ticks = 0u32;
                    #[cfg(windows)]
                    let mut startup_unmuted = false;
                    let mut startup_ticks = 0u32;
                    let mut startup_reported = false;
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(800));
                        let hidden = state_hidden();
                        // Windows PERSISTS a session's mute state across app
                        // restarts (per-app Volume Mixer store). If the previous
                        // session exited while muted (pause-on-minimize), this
                        // cold start would begin OS-muted — with no hidden/visible
                        // transition ever firing, nothing would unmute it and the
                        // app would be silent while the page plays fine. Retry the
                        // unmute until a session actually exists (the WebView2
                        // session appears seconds after launch), capped at ~32 s.
                        // (Windows-only: audio.rs / Core Audio is cfg(windows).)
                        #[cfg(windows)]
                        {
                            if !startup_unmuted {
                                unmute_ticks += 1;
                                if unmute_ticks >= 3 && !hidden {
                                    if audio::set_app_audio_mute(false) {
                                        startup_unmuted = true;
                                        diag!("[TikTok-Now] Watchdog: startup unmute (cleared persisted session mute)");
                                        // Immediate evidence: session state right after clearing.
                                        #[cfg(debug_assertions)]
                                        audio::report_audio_state();
                                    } else if unmute_ticks >= 60 {
                                        startup_unmuted = true; // ~48 s: no session appeared — nothing to clear
                                        diag!("[TikTok-Now] Watchdog: startup unmute gave up (no session appeared)");
                                    }
                                }
                            }
                        }
                        if hidden && !last_hidden {
                            diag!("[TikTok-Now] Watchdog: window hidden -> pausing media");
                            pause_media_for_hidden(&w);
                        } else if !hidden && last_hidden {
                            diag!("[TikTok-Now] Watchdog: window visible -> resuming media");
                            resume_media_for_visible(&w);
                        }
                        last_hidden = hidden;
                        // Cold-start diagnostic: ~20 s after launch, with no
                        // hidden/visible transition yet, dump the TRUE fresh-launch
                        // page state (muted? volume?) — the E2E evidence that the
                        // startup sound defaults actually hold BEFORE any
                        // minimize/restore cycle. Debug builds only; release
                        // builds skip the whole block.
                        if !startup_reported {
                            if hidden {
                                startup_reported = true; // a transition happened first; skip
                            } else {
                                startup_ticks += 1;
                                #[cfg(debug_assertions)]
                                if startup_ticks >= 25 {
                                    startup_reported = true;
                                    let _ = w.eval_with_callback(REPORT_JS, |report| {
                                        diag!("[TikTok-Now] Watchdog startup report: {}", report);
                                    });
                                    // OS-level evidence: session mute + master volume.
                                    // (Windows-only module — MUST stay cfg-gated so
                                    // macOS/Linux debug builds keep compiling.)
                                    #[cfg(windows)]
                                    audio::report_audio_state();
                                }
                                #[cfg(not(debug_assertions))]
                                {
                                    if startup_ticks >= 25 {
                                        startup_reported = true;
                                    }
                                }
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if window.label() == "main" {
                    // Minimal, deterministic close-to-tray: prevent the close FIRST,
                    // pause via a plain eval (no callbacks/threads/COM — those were
                    // observed to race with the OS close processing and occasionally
                    // let the window be destroyed), then hide. The watchdog's
                    // hidden-transition applies the full layered pause within 800 ms.
                    api.prevent_close();
                    if let Some(main) = window.get_webview_window("main") {
                        let _ = main.eval("if (window.__onWindowHidden) window.__onWindowHidden();");
                        let _ = main.hide();
                    }
                }
            }
            // Fast path: tao emits Resized(0x0) the moment the window minimizes,
            // while the watchdog polls at 800 ms. Pause immediately here too.
            tauri::WindowEvent::Resized(_)
                if window.label() == "main" && window.is_minimized().unwrap_or(false) =>
            {
                if let Some(main) = window.get_webview_window("main") {
                    pause_media_for_hidden(&main);
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::jump_to_external,
        ])
        .run(tauri::generate_context!())
        .expect(concat!(
            "error while running TikTok-Now v",
            env!("CARGO_PKG_VERSION")
        ));
}

/// v2.1.0: local-origin recognition shared by the navigation interceptors.
fn is_local_host(host: &str) -> bool {
    host == "localhost"
        || host == "tauri.localhost"
        || host.ends_with(".localhost")
        || host == "127.0.0.1"
        || host == "[::1]"
        || host == "ipc.localhost"
}

/// Title-extractor script evaluated on title changes until a handle is known.
/// Mirrors `getTikTokUserHandle` in INIT_JS; sets `TIKTOKNOW:<handle>` only.
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

/// v2.1.0: OAuth popup engine — defensive version. The old inline script
/// appended to `document.head || document.documentElement` at injection time,
/// where BOTH can be null (pre-DOM); a thrown TypeError also killed the
/// auto-close interval that was registered after it, so sign-in popups could
/// never self-close. The interval is now registered unconditionally and the
/// dark style is applied only once the DOM exists.
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
        // v2.1.0 round 4 (F24): paint at true document-start. The
        // DOMContentLoaded wait let the popup's default white background show
        // for one or more frames before darkening.
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
        // Auto-close once login completes (navigated past the login picker).
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

/// The main TikTok page engine. Injected as an initialization script into
/// every page load of the main window (and any popup, via the same builder
/// default — sections that must not run on login popups guard themselves).
/// v2.1.0 changes are marked with `v2.1.0` comments:
///   - TikTok-only guards for the dark canvas and auto-scroll,
///   - Messages/DM-page awareness (typing guard, no synthetic clicks),
///   - modifier-key + text-field guards on the keyboard shortcuts,
///   - async-clipboard fix for the false "URL copied" toast.
const INIT_JS: &str = r##"
    (function() {
        'use strict';

        // WebView2 injects this script at DOCUMENT CREATION — before <html>
        // and <head> exist — so touching document.documentElement/head right
        // away throws a TypeError and silently kills the whole script (which
        // is why none of the page-side helpers ever appeared on TikTok pages).
        // Defer every section until the DOM actually exists.
        function start() {
        try {

        var isTikTokPage = location.hostname === 'tiktok.com' || location.hostname.endsWith('.tiktok.com');

        // 0. Force Immediate Dark Canvas on Page Navigation
        //    v2.1.0: TikTok-only — the splash/login pages served by the app
        //    define their own theme and must not be force-painted over.
        //
        //    FLASH ROOT CAUSE (v2.1.0 audit round 4, F24): this used to live
        //    inside start(), i.e. it first ran at DOMContentLoaded — AFTER the
        //    browser had already committed its first paint of the new
        //    document's default WHITE background. Every in-app navigation
        //    (splash→feed, tray feed jumps) therefore flashed white before
        //    darkening. The fix paints at true document-start: the observer
        //    appends the style the instant <html> exists, which is still
        //    before the first paint commit, so no white frame is ever shown.
        if (isTikTokPage) {
            var forceDarkCSS = document.createElement('style');
            forceDarkCSS.id = 'tiktok-now-dark-canvas';
            forceDarkCSS.textContent = 'html, body { background-color: #0d0e15 !important; color: #ffffff !important; }';
            var appendDarkCanvas = function () {
                var root = document.head || document.documentElement;
                if (root && !document.getElementById('tiktok-now-dark-canvas')) {
                    root.appendChild(forceDarkCSS);
                }
                if (document.documentElement) {
                    document.documentElement.style.backgroundColor = '#0d0e15';
                }
            };
            if (document.documentElement || document.head) {
                appendDarkCanvas();
            } else {
                var darkCanvasObserver = new MutationObserver(function () {
                    if (document.documentElement || document.head) {
                        darkCanvasObserver.disconnect();
                        appendDarkCanvas();
                    }
                });
                darkCanvasObserver.observe(document, { childList: true, subtree: true });
            }
            appendDarkCanvas();
        }

        // v2.1.0: Messages/DM-page detection. TikTok's DM view lives at
        // /messages (optionally /messages/<id>). On that page the desktop
        // engine must behave differently: no synthetic play/pause clicks,
        // no auto-scroll, and keyboard shortcuts stay out of the composer.
        window.__tiktoknow_is_dm = function() {
            return location.pathname === '/messages' || location.pathname.indexOf('/messages/') === 0;
        };

        // Helper: Extract valid logged-in TikTok user handle (rejecting feed authors & numeric IDs)
        function getTikTokUserHandle() {
            try {
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
            } catch(e) {}
            return null;
        }

        // 0b. DOM Toast Observer: Watches for "Logged in" Toast Notification & Triggers Instant Popup Close
        new MutationObserver(function() {
            try {
                var text = document.body ? document.body.innerText : '';
                if (text.indexOf('Logged in') !== -1 || text.indexOf('Login success') !== -1) {
                    var handle = getTikTokUserHandle();
                    if (handle) {
                        document.title = 'TIKTOKNOW:' + handle;
                    } else {
                        document.title = 'TIKTOKNOW_LOGGED_IN';
                    }
                }
            } catch(e) {}
        }).observe(document.documentElement, { childList: true, subtree: true, characterData: true });

        // 0c. Captcha & Slider Dragging Fix: Prevents native WebView HTML5 dragstart from cancelling pointermove/mousemove
        document.addEventListener('dragstart', function(e) {
            if (e.target) {
                // v2.1.0 round 6: text fields keep NATIVE drag behavior
                // (dragging selected text out of an input is standard) —
                // the captcha-family selectors below also match the
                // screen-time dialog's container.
                if (e.target.closest &&
                    e.target.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]')) return;
                var tag = e.target.tagName ? e.target.tagName.toUpperCase() : '';
                if (tag === 'IMG' || tag === 'SVG' || e.target.closest('[class*="captcha"]') || e.target.closest('[class*="sec-captcha"]') || e.target.closest('[class*="slider"]') || e.target.closest('[class*="verify"]') || e.target.closest('[class*="puzzle"]')) {
                    e.preventDefault();
                }
            }
        }, true);

        var captchaCSS = document.createElement('style');
        captchaCSS.id = 'tiktok-now-captcha-fix';
        captchaCSS.textContent = `
            /* v2.1.0 round 6: NATIVE TEXT ENTRY RESTORATION. The captcha rules
               below match broad substrings (verify, puzzle, ...) that also hit
               NON-captcha surfaces — notably TikTok's screen-time passcode
               dialog (its container class contains verify). user-select
               INHERITS into the dialog's input, and Chromium/WebView2 inputs
               with inherited user-select:none cannot reliably place the caret
               or accept typed text. Form controls always get native
               selection/typing behavior back. */
            input, textarea, select,
            [contenteditable=""], [contenteditable="true"] {
                -webkit-user-select: text !important;
                user-select: text !important;
                touch-action: auto !important;
                -webkit-user-drag: auto !important;
            }
            [class*="captcha"]:not(input):not(textarea):not(select),
            [class*="sec-captcha"]:not(input):not(textarea):not(select),
            [class*="slider"]:not(input):not(textarea):not(select),
            [class*="verify"]:not(input):not(textarea):not(select),
            [class*="puzzle"]:not(input):not(textarea):not(select) {
                -webkit-user-drag: none !important;
                user-select: none !important;
                -webkit-user-select: none !important;
                touch-action: none !important;
            }
            [class*="captcha"] img, [class*="sec-captcha"] img, [class*="captcha"] svg, [class*="sec-captcha"] svg {
                -webkit-user-drag: none !important;
                pointer-events: none !important;
            }
        `;
        (document.head || document.documentElement).appendChild(captchaCSS);

        window.autoScrollEnabled = false;
        // v2.1.0: persistent typing-guard flag — initialized here, BEFORE the
        // keydown listener below reads it (the listeners that maintain it are
        // registered further down, next to the keyboard engine).
        window.__tiktoknow_typing = false;

        // 1. Floating Toast Notification
        function showToast(msg) {
            var t = document.getElementById('tiktok-now-toast');
            if (!t) {
                t = document.createElement('div');
                t.id = 'tiktok-now-toast';
                t.style.cssText = `
                    position: fixed;
                    top: 24px;
                    right: 24px;
                    background: rgba(13, 14, 21, 0.95);
                    color: #00F2FE;
                    border: 1px solid #FF007F;
                    padding: 12px 22px;
                    border-radius: 14px;
                    font-family: sans-serif;
                    font-weight: bold;
                    font-size: 14px;
                    z-index: 99999999;
                    box-shadow: 0 12px 30px rgba(0,0,0,0.8);
                    transition: opacity 0.3s ease, transform 0.3s ease;
                    pointer-events: none;
                `;
                document.body.appendChild(t);
            }
            t.textContent = msg;
            t.style.opacity = '1';
            t.style.transform = 'translateY(0)';
            clearTimeout(window._toastTimeout);
            window._toastTimeout = setTimeout(function() {
                t.style.opacity = '0';
                t.style.transform = 'translateY(-10px)';
            }, 2200);
        }

        window.toggleAutoScroll = function() {
            // v2.1.0: auto-scroll is a feed feature; on the Messages page it
            // would fight the chat's own scrolling and break the composer.
            if (window.__tiktoknow_is_dm()) {
                showToast('💬 Auto-Scroll is not available in Messages');
                return;
            }
            window.autoScrollEnabled = !window.autoScrollEnabled;
            showToast(window.autoScrollEnabled ? '📜 Auto-Scroll: ENABLED' : '📜 Auto-Scroll: DISABLED');
        };

        // 3. Desktop CSS Clean-up
        var style = document.createElement('style');
        style.id = 'tiktok-now-desktop-styles';
        style.textContent = `
            [class*="DivBannerContainer"],
            [class*="DivAppDownload"],
            [class*="ButtonGetApp"],
            [data-e2e="open-app-btn"],
            .tiktok-banner-container,
            div[class*="tiktok-download"] {
                display: none !important;
            }

            ::-webkit-scrollbar {
                width: 8px;
                height: 8px;
            }
            ::-webkit-scrollbar-track {
                background: #0d0e15;
            }
            ::-webkit-scrollbar-thumb {
                background: #00F2FE;
                border-radius: 4px;
            }
        `;
        document.head.appendChild(style);

        // 5. Video Observer & Auto-Scroll
        var activeVideo = null;
        // Tracks whether the user intends the current video to play.
        // Drives pause-on-minimize and resume-on-restore.
        var _intendedPlaying = false;
        var _resumeOnVisible = false;
        var observer = new IntersectionObserver(function(entries) {
            entries.forEach(function(entry) {
                var vid = entry.target;
                if (entry.isIntersecting && entry.intersectionRatio >= 0.5) {
                    if (activeVideo && activeVideo !== vid) {
                        activeVideo.pause();
                    }
                    vid.play().catch(function(){});
                    activeVideo = vid;
                    _intendedPlaying = true;
                } else if (!entry.isIntersecting && activeVideo === vid) {
                    vid.pause();
                    activeVideo = null;
                    _intendedPlaying = false;
                }
            });
        }, { threshold: [0.5] });

        // Startup sound defaults. TikTok's player initializes the first
        // video muted on a fresh session (its "needs a gesture"
        // heuristic) and can RE-MUTE after its own init completes, so a
        // one-time flip is not enough. During a 60 s grace window we
        // re-enforce on EVERY 'play' event plus a 1 s interval:
        // unmuted + 50% default volume. Guarded so it never fights the
        // pause-on-minimize mute-backup; after grace, the user's own
        // mute/volume choices rule forever.
        var _startupGraceUntil = Date.now() + 60000;
        function maybeStartupUnmute(v) {
            if (_mutedByHide || document.hidden) return;   // never fight the hide-backup
            if (Date.now() > _startupGraceUntil) return;   // grace over — hands off
            if (v) {
                if (v.muted) v.muted = false;
                if (Math.abs((v.volume || 0) - 0.5) > 0.01) v.volume = 0.5;
            }
        }
        window.__tiktoknowStartupTick = function() {
            maybeStartupUnmute(activeVideo || document.querySelector('video'));
        };
        window.__tiktoknowActiveVideo = function() { return activeVideo || null; };
        setInterval(function() {
            maybeStartupUnmute(activeVideo || document.querySelector('video'));
        }, 1000);
        function setupVideoListeners(v) {
            observer.observe(v);
            if (!v.dataset.hasEndedListener) {
                v.dataset.hasEndedListener = 'true';
                v.addEventListener('ended', function() {
                    if (window.autoScrollEnabled) {
                        window.scrollBy({ top: window.innerHeight * 0.85, behavior: 'smooth' });
                    }
                });
                v.addEventListener('play', function() { maybeStartupUnmute(v); });
            }
        }

        function observeVideos(root) {
            var vids = root ? root.querySelectorAll('video') : [];
            vids.forEach(setupVideoListeners);
        }

        // 5c. Photo-post drag navigation: drag horizontally on a photo
        //     post (multi-image = next/previous slide; single-image =
        //     nothing but no accidental pause/selection). Philosophy:
        //     (1) once the pointer moves on a photo post, the release
        //     click is SUPPRESSED — TikTok otherwise toggles the
        //     slideshow play/pause and selects the image on every drag;
        //     (2) past the drag threshold we click TikTok's OWN arrow
        //     buttons (same state-machine approach as the pause fix),
        //     so the site's slide animation and counters stay consistent.
        var CAROUSEL_DRAG_THRESHOLD = 60;   // px of horizontal travel to navigate
        var CAROUSEL_CLAIM_PX = 6;          // px of movement before we claim the drag (kills native image-drag/selection early)
        var _carouselDrag = null;
        var _suppressClickUntil = 0;
        var _suppressClickWrap = null;

        // Returns { wrap, next, prev, track, single } when the pressed
        // element is inside a PHOTO post (video posts return null and are
        // left to TikTok). Multi-image carousels carry next/prev arrows
        // (TikTok's data-e2e buttons, or an edge-button fallback);
        // single-photo posts return next/prev = null so a drag only
        // suppresses pause/selection without navigating.
        function findPhotoContext(target) {
            var el = target;
            for (var depth = 0; el && depth < 8; depth++, el = el.parentElement) {
                if (!el.querySelectorAll || el === document.body || el === document.documentElement) continue;
                var next = el.querySelector('[data-e2e="arrow-next"]');
                var prev = el.querySelector('[data-e2e="arrow-prev"]');
                if (next && prev) {
                    return { wrap: el, next: next, prev: prev, track: findCarouselTrack(el), single: false };
                }
                if (el.querySelector('video')) return null;   // video post — leave TikTok alone
                var imgs = el.querySelectorAll('img');
                var big = 0;
                for (var i = 0; i < imgs.length; i++) {
                    if ((imgs[i].naturalWidth || imgs[i].width || 0) >= 200) big++;
                }
                var r = el.getBoundingClientRect();
                // jsdom reports zero rects; size checks only apply on real pages.
                if (r.width > 0 && (r.width < 200 || r.height < 150)) continue;
                if (imgs.length >= 2) {
                    // Multi-image container: the carousel track or a
                    // wrapper above it — keep walking until the level
                    // carrying the arrow buttons. If the images are
                    // actually small (avatar/thumbnail rows), skip it.
                    if (r.width > 0 && big < 2) continue;
                    var fb = findEdgeButtons(el, r);
                    if (fb) {
                        return { wrap: el, next: fb.next, prev: fb.prev, track: findCarouselTrack(el), single: false };
                    }
                    continue;
                }
                if (r.width > 0 && big < 1) continue;   // no photo-size image here (avatar etc.)
                if (imgs.length < 1) continue;          // nothing photo-like inside — keep walking
                // A large image with no video and no arrows: photo post.
                return { wrap: el, next: null, prev: null, track: null, single: true };
            }
            return null;
        }

        // Fallback arrow detection for carousels without data-e2e buttons:
        // clickable buttons at the left/right edges, vertically centered,
        // INSIDE the image span (the like/comment column sits outside it).
        function findEdgeButtons(wrap, r) {
            var buttons = wrap.querySelectorAll('button');
            var leftBtn = null, rightBtn = null;
            for (var j = 0; j < buttons.length; j++) {
                var br = buttons[j].getBoundingClientRect();
                if (br.width === 0 || br.height === 0) continue;
                var insideX = br.left >= r.left - 10 && br.right <= r.right + 10;
                var vCentered = Math.abs((br.top + br.height / 2) - (r.top + r.height / 2)) <= r.height / 2;
                if (!insideX || !vCentered) continue;
                if (br.right < r.left + r.width / 2 && br.left < r.left + r.width * 0.35) leftBtn = buttons[j];
                if (br.left > r.left + r.width / 2 && br.right > r.right - r.width * 0.35) rightBtn = buttons[j];
            }
            return (leftBtn && rightBtn) ? { next: rightBtn, prev: leftBtn } : null;
        }

        // The horizontally-translated track of images (for drag feedback).
        function findCarouselTrack(wrap) {
            var t = wrap.querySelector('[style*="transform"]');
            if (t && t.querySelectorAll('img').length >= 2) return t;
            var cands = wrap.querySelectorAll('div');
            for (var i = 0; i < cands.length; i++) {
                var c = cands[i];
                if (c.querySelectorAll('img').length >= 2 &&
                    (c.style.transform || c.style.transition)) return c;
            }
            return null;
        }

        document.addEventListener('pointerdown', function(e) {
            // v2.1.0: Messages page — chats can drag images/emoji grids; the
            // feed carousel logic must never claim those gestures.
            if (window.__tiktoknow_is_dm()) return;
            // v2.1.0 round 6: never begin a carousel claim from a text-entry
            // surface — a press inside an input (e.g. the screen-time
            // passcode box rendered over a post) must never lead to the
            // release-click suppression eating the input's own click/focus.
            if (e.target && e.target.closest &&
                e.target.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"]')) return;
            var ctx = findPhotoContext(e.target);
            if (!ctx) return;
            _carouselDrag = {
                ctx: ctx,
                startX: e.clientX, startY: e.clientY,
                dx: 0, dy: 0, active: false, bailed: false, moved: false,
                savedTrackStyle: ctx.track ? ctx.track.getAttribute('style') : null
            };
        }, true);

        document.addEventListener('pointermove', function(e) {
            if (!_carouselDrag) return;
            var d = _carouselDrag;
            d.dx = e.clientX - d.startX;
            d.dy = e.clientY - d.startY;
            if (Math.abs(d.dx) + Math.abs(d.dy) > 6) d.moved = true;
            if (!d.active) {
                if (Math.abs(d.dy) > 14 && Math.abs(d.dy) > Math.abs(d.dx)) {
                    d.bailed = true;          // vertical intent: feed scroll — do NOT preventDefault
                    return;
                }
                if (Math.abs(d.dx) <= CAROUSEL_CLAIM_PX || Math.abs(d.dx) <= Math.abs(d.dy)) return;
                d.active = true;              // horizontal intent: claim the drag
            }
            e.preventDefault();              // stop native image-drag / selection
            if (d.ctx.track) {
                d.ctx.track.style.transition = 'none';
                d.ctx.track.style.transform = 'translateX(' + d.dx + 'px)';
            }
        }, true);

        document.addEventListener('pointerup', function(e) {
            if (!_carouselDrag) return;
            var d = _carouselDrag;
            _carouselDrag = null;
            if (d.ctx.track) {
                d.ctx.track.setAttribute('style', d.savedTrackStyle || '');
            }
            if (d.bailed || !d.active || Math.abs(d.dx) < CAROUSEL_DRAG_THRESHOLD) {
                // No navigation (vertical scroll, small drag, or a single
                // photo): still suppress the release click so the post is
                // neither paused nor the image selected.
                if (d.moved) { _suppressClickUntil = Date.now() + 400; _suppressClickWrap = d.ctx.wrap; }
                return;
            }
            if (d.dx < 0 && d.ctx.next) d.ctx.next.click();     // dragged left  -> next image
            else if (d.dx > 0 && d.ctx.prev) d.ctx.prev.click(); // dragged right -> previous image
            // Suppress the user's own release click (the arrow clicks
            // above already fired BEFORE this, so they are not affected).
            if (d.moved) { _suppressClickUntil = Date.now() + 400; _suppressClickWrap = d.ctx.wrap; }
        }, true);

        // The "smart delay": a click landing inside the photo post shortly
        // after a drag is swallowed (no pause, no selection). Clicks
        // outside the post — like buttons, comments — always pass through.
        document.addEventListener('click', function(e) {
            // Arrow-button clicks (ours AND the user's) always pass —
            // the suppression exists to protect the PHOTO, not the controls.
            if (e.target && e.target.closest &&
                e.target.closest('[data-e2e="arrow-prev"], [data-e2e="arrow-next"]')) return;
            if (Date.now() < _suppressClickUntil && _suppressClickWrap &&
                e.target && _suppressClickWrap.contains(e.target)) {
                e.preventDefault();
                e.stopPropagation();
            }
        }, true);

        // Never let a photo-post press start a native HTML5 image drag
        // (this also covers the pre-claim window, where movement is
        // still below the claim threshold).
        document.addEventListener('dragstart', function(e) {
            if (_carouselDrag) e.preventDefault();
        }, true);

        new MutationObserver(function(mutations) {
            mutations.forEach(function(m) {
                m.addedNodes.forEach(function(node) {
                    if (node.nodeType === 1) observeVideos(node);
                });
            });
        }).observe(document.documentElement, { childList: true, subtree: true });

        if (document.readyState === 'complete') {
            observeVideos(document);
        } else {
            window.addEventListener('load', function() { observeVideos(document); });
        }

        // 5b. Pause-on-Minimize: when the window is minimized (or hidden to the
        //     tray), stop ALL playback so audio doesn't keep playing in the
        //     background. When the window comes back into view, resume the
        //     video that was playing before.
        // The Rust watchdog thread also calls these two helpers by polling the
        // real window state — so pausing works even on WebView2/WebKit builds
        // that never fire `visibilitychange` for a minimized window.
        //
        // Strategy (layered, so at least one always lands):
        //  1. Direct `pause()` on every <video>/<audio> element — instant silence.
        //  2. Synthetic center-click on the active video: TikTok's own player
        //     state machine flips to "paused", so its autoplay engine stops
        //     re-issuing play() (a raw element pause() desyncs their state and
        //     can be reverted by the engine).
        //  3. Mute backup: remember the video's mute state and mute it while
        //     hidden — even if some engine resumes playback, it stays silent.
        //  4. Rust side additionally mutes the app's Windows audio sessions.
        var _wasMutedBeforeHide = false;
        var _mutedByHide = false;
        var _playingAudios = [];   // audio elements playing at hide (audio-only feeds)
        window.__onWindowHidden = function() {
            var v = activeVideo || document.querySelector('video');
            var wasPlaying = !!(v && !v.paused);
            // TikTok can serve audio-only states (no <video>); remember
            // what was playing so the resume path can bring it back.
            _playingAudios = Array.from(document.querySelectorAll('audio'))
                .filter(function(a) { return !a.paused; });
            // Resume on restore if our observer intended playback OR the video
            // was actually playing (covers minimize right after page load,
            // before the IntersectionObserver has marked the video).
            _resumeOnVisible = _intendedPlaying || wasPlaying || _playingAudios.length > 0;
            window.pauseAllVideos();                       // layer 1
            // v2.1.0: the synthetic state-sync click is a FEED player fix —
            // on the Messages page it lands in the chat canvas (scrolling it,
            // or worse, activating a focused message action). Layer 1 + the
            // OS-level mute are enough there.
            if (wasPlaying && v && !window.__tiktoknow_is_dm()) {
                try {                                      // layer 2: TikTok state sync
                    var r = v.getBoundingClientRect();
                    var cx = (r && r.width) ? r.left + r.width / 2 : window.innerWidth / 2;
                    var cy = (r && r.height) ? r.top + r.height / 2 : window.innerHeight / 2;
                    v.dispatchEvent(new MouseEvent('click', {
                        bubbles: true, cancelable: true, clientX: cx, clientY: cy, view: window
                    }));
                    if (!v.paused) v.pause();              // click toggled the wrong way? force-pause
                } catch(e) {}
                _wasMutedBeforeHide = v.muted;             // layer 3: silence guarantee
                _mutedByHide = !v.muted;
                if (!v.muted) v.muted = true;
            }
        };
        window.__resumeIfNeeded = function() {
            if (!_resumeOnVisible) return;
            _resumeOnVisible = false;
            var v = activeVideo || document.querySelector('video');
            if (!v) {
                // Audio-only feed (no <video>): bring the audio back.
                _playingAudios.forEach(function(a) {
                    if (a.paused) {
                        var p = a.play();
                        if (p && p.catch) p.catch(function(){});
                    }
                });
                _playingAudios = [];
                return;
            }
            if (v) {
                if (_mutedByHide) {                        // restore mute state, then resume
                    v.muted = _wasMutedBeforeHide;
                    _mutedByHide = false;
                }
                if (v.paused) {
                    var p = v.play();
                    if (p && p.catch) p.catch(function(){});
                }
            }
        };
        window.__tiktoknowPauseReport = function() {
            var v = activeVideo || document.querySelector('video');
            var media = document.querySelectorAll('video, audio').length;
            return JSON.stringify({
                media: media,
                active: v ? {
                    paused: v.paused,
                    muted: v.muted,
                    time: Math.round((v.currentTime || 0) * 10) / 10
                } : null,
                intended: _intendedPlaying,
                resumeFlag: _resumeOnVisible,
                isDM: window.__tiktoknow_is_dm()
            });
        };
        document.addEventListener('visibilitychange', function() {
            if (document.hidden) {
                window.__onWindowHidden();
            } else {
                window.__resumeIfNeeded();
            }
        });

        // Shared playback helpers (used by the tray menu + keyboard shortcuts).
        // Pauses <video> AND <audio> so no background sound survives hiding.
        window.pauseAllVideos = function() {
            document.querySelectorAll('video, audio').forEach(function(m) { if (!m.paused) m.pause(); });
        };

        window.togglePlayPause = function() {
            // v2.1.0: on the Messages page there is no feed video to toggle;
            // an injected Space/click could open a focused conversation item.
            if (window.__tiktoknow_is_dm()) return;
            var v = activeVideo || document.querySelector('video');
            if (!v) return;
            if (v.paused) {
                var p = v.play();
                if (p && p.catch) p.catch(function(){});
                _intendedPlaying = true;
            } else {
                v.pause();
                _intendedPlaying = false;
            }
        };

        window.toggleMute = function() {
            var v = activeVideo || document.querySelector('video');
            if (v) {
                v.muted = !v.muted;
                showToast(v.muted ? '🔇 Muted' : '🔊 Unmuted');
            }
        };

        window.seekBy = function(delta) {
            // v2.1.0: Messages page — never synthesize carousel/seek clicks.
            if (window.__tiktoknow_is_dm()) return;
            var v = activeVideo || document.querySelector('video');
            if (!v) {
                // No video (photo post): ← / → navigate the carousel instead.
                var nextBtn = document.querySelector('[data-e2e="arrow-next"]');
                var prevBtn = document.querySelector('[data-e2e="arrow-prev"]');
                if (delta > 0 && nextBtn) nextBtn.click();
                else if (delta < 0 && prevBtn) prevBtn.click();
                return;
            }
            // `isFinite` guard keeps live streams (duration = Infinity) seek-safe
            if (isFinite(v.duration)) {
                v.currentTime = Math.min(Math.max(0, v.currentTime + delta), v.duration);
            }
        };

        window.togglePip = function() {
            var v = activeVideo || document.querySelector('video');
            if (!v) return;
            if (document.pictureInPictureElement) {
                document.exitPictureInPicture().catch(function(){});
            } else if (v.requestPictureInPicture) {
                v.requestPictureInPicture().catch(function(){});
            }
        };

        window.captureFrame = function() {
            var v = activeVideo || document.querySelector('video');
            if (!v || !v.videoWidth || !v.videoHeight) {
                showToast('⚠️ No video frame available');
                return;
            }
            try {
                var c = document.createElement('canvas');
                c.width = v.videoWidth;
                c.height = v.videoHeight;
                c.getContext('2d').drawImage(v, 0, 0);
                var a = document.createElement('a');
                a.href = c.toDataURL('image/png');
                a.download = 'tiktok-now-frame-' + Date.now() + '.png';
                document.body.appendChild(a);
                a.click();
                a.remove();
                showToast('📸 Frame captured');
            } catch(e) {
                showToast('⚠️ Frame capture blocked by TikTok CORS');
            }
        };

        window.copyCurrentUrl = function() {
            var url = window.location.href;
            function fallbackCopy(onDone) {
                var ta = document.createElement('textarea');
                ta.value = url;
                ta.style.position = 'fixed';
                ta.style.opacity = '0';
                document.body.appendChild(ta);
                ta.select();
                var ok = false;
                try { ok = document.execCommand('copy'); } catch(e) {}
                ta.remove();
                if (onDone) onDone(ok);
            }
            // v2.1.0 UX fix: the old code showed "📋 URL copied" even when
            // the promise rejected AND the textarea fallback failed. The
            // success toast now reflects the REAL outcome of the copy.
            if (navigator.clipboard && navigator.clipboard.writeText) {
                navigator.clipboard.writeText(url).then(function() {
                    showToast('📋 URL copied');
                }, function() {
                    fallbackCopy(function(ok) {
                        showToast(ok ? '📋 URL copied' : '⚠️ Copy blocked — select the address bar instead');
                    });
                });
            } else {
                fallbackCopy(function(ok) {
                    showToast(ok ? '📋 URL copied' : '⚠️ Copy blocked — select the address bar instead');
                });
            }
        };

        // 6. Native Desktop Navigation Engine (Arrow keys, Space, Auto-scroll A, Refresh R)
        window.addEventListener('keydown', function(e) {
            // v2.1.0: never hijack keys while a modifier is held (Ctrl+R,
            // Ctrl+F, browser/webview shortcuts must stay native), and skip
            // everything while the user types in TikTok's own inputs —
            // including contenteditable widgets and the Messages composer.
            // The persistent `__tiktoknow_typing` flag (set by the capture-phase
            // listener below until `focusout`) is belt-and-suspenders against
            // focus-timing races between the two listeners.
            if (e.ctrlKey || e.metaKey || e.altKey) return;
            if (window.__tiktoknow_typing) return;

            var ae = document.activeElement;
            var activeTag = ae ? ae.tagName : '';
            if (activeTag === 'INPUT' || activeTag === 'TEXTAREA' || activeTag === 'BUTTON' || activeTag === 'SELECT' || (ae && (ae.isContentEditable || ae.getAttribute && ae.getAttribute('contenteditable') === 'true'))) {
                // Inside a text field only Escape should blur the field — a
                // deliberate convenience, not a hijack.
                if (e.key === 'Escape') { try { ae.blur(); } catch (err) {} }
                return;
            }

            var key = e.key.toLowerCase();

            if (e.key === 'ArrowDown' || key === 'j') {
                window.scrollBy({ top: window.innerHeight * 0.85, behavior: 'smooth' });
            } else if (e.key === 'ArrowUp' || key === 'k') {
                window.scrollBy({ top: -window.innerHeight * 0.85, behavior: 'smooth' });
            } else if (e.code === 'Space') {
                e.preventDefault();
                window.togglePlayPause();
            } else if (e.key === 'ArrowLeft') {
                window.seekBy(-5);
            } else if (e.key === 'ArrowRight') {
                window.seekBy(5);
            } else if (key === 'm') {
                window.toggleMute();
            } else if (key === 'p') {
                window.togglePip();
            } else if (key === 's') {
                window.captureFrame();
            } else if (key === 'a') {
                window.toggleAutoScroll();
            } else if (key === 'r') {
                window.location.reload();
            }
        });

        // v2.1.0: guard the typing surface, not just the keydown moment. If the
        // user starts typing (composer, search box) mid-gesture, shortcuts are
        // suspended until the field loses focus. Typing detection is based on
        // focus + key events only — no clipboard snooping, no content access.
        function isTextField(el) {
            return !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable);
        }
        window.addEventListener('keydown', function(e) {
            if (isTextField(document.activeElement)) {
                window.__tiktoknow_typing = true;
            }
        }, true);
        window.addEventListener('focusout', function(e) {
            if (isTextField(e.target) && !isTextField(document.activeElement)) {
                window.__tiktoknow_typing = false;
            }
        }, true);

        // 7. Periodic Username Extractor (only rewrites the title when it changed —
        //    prevents constant on_document_title_changed churn in the Rust side)
        setInterval(function() {
            try {
                var handle = getTikTokUserHandle();
                if (handle && document.title.indexOf('TIKTOKNOW:') !== 0) {
                    document.title = 'TIKTOKNOW:' + handle;
                }
            } catch(e) {}
        }, 1000);

        // 8. Global External Link Opener — available to injected overlays (About modal, etc.)
        //    Uses Tauri IPC directly from init script context where __TAURI_INTERNALS__ is available.
        window.__tiktoknow_open = function(url) {
            try {
                if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                    window.__TAURI_INTERNALS__.invoke('jump_to_external', { url: url });
                } else if (window.__TAURI__ && window.__TAURI__.core) {
                    window.__TAURI__.core.invoke('jump_to_external', { url: url });
                } else {
                    window.open(url, '_blank');
                }
            } catch(err) {
                window.open(url, '_blank');
            }
        };

        // 9. Link-Aware External Handler (Non-TikTok links open in default OS browser; TikTok links load in-app)
        //    Intercepts ALL anchor clicks at capture phase — fires before TikTok's own handlers.
        document.addEventListener('click', function(e) {
            var a = e.target ? e.target.closest('a') : null;
            if (!a) return;
            var href = a.getAttribute('href') || a.href || '';
            if (!href || href.startsWith('#') || href.startsWith('javascript:')) return;

            var isExternal = false;
            try {
                var u = new URL(href, window.location.href);
                var host = u.hostname.toLowerCase();
                var isTikTok = host === 'tiktok.com' || host.endsWith('.tiktok.com');
                // v2.1.0 audit round 2: kept in lockstep with the Rust-side
                // `is_local_host` (single source of truth for local origins).
                var isLocal = host === 'localhost' || host === '127.0.0.1' || host === '[::1]' ||
                              host === 'tauri.localhost' || host === 'ipc.localhost' || host.endsWith('.localhost') ||
                              u.protocol === 'file:' || u.protocol === 'tauri:';
                if (!isTikTok && !isLocal && (u.protocol === 'http:' || u.protocol === 'https:')) {
                    isExternal = true;
                }
            } catch(err) {
                // relative links or unparseable — keep in-app
            }

            if (isExternal) {
                e.preventDefault();
                e.stopImmediatePropagation();
                var fullUrl = a.href || href;
                window.__tiktoknow_open(fullUrl);
            }
        }, true);

        } catch(e) {
            try { console.error('[TikTok-Now] init failed:', e); } catch(_) {}
        }
        } // end start()
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', start);
        } else {
            start();
        }
    })();
"##;
