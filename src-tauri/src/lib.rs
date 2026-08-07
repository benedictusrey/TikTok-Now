mod commands;
mod db;
mod error;
mod state;
mod tray;

use db::open_db;
use state::AppState;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex,
};
use tauri::{window::Color, webview::NewWindowResponse, Manager};

static POPUP_COUNTER: AtomicU32 = AtomicU32::new(0);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .to_string_lossy()
                .to_string();

            std::fs::create_dir_all(&app_data_dir).ok();

            let db = open_db(&app_data_dir).expect("failed to open database");
            app.manage(AppState::new(db));

            if let Err(e) = tray::setup_tray(app.handle()) {
                eprintln!("[TikTok-Now] Tray setup failed: {}", e);
            }

            let username_cache: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let cache_for_title = username_cache.clone();

            let app_handle = app.handle().clone();

            // ── Main Window with Native Dark Canvas Background (#0D0E15) ─────────
            let _window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("TikTok-Now")
            .inner_size(1250.0, 900.0)
            .min_inner_size(800.0, 600.0)
            .shadow(false)
            .center()
            .background_color(Color(13, 14, 21, 255))
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            )
            // ── Enable WebView2 Translate-to-English bar on Windows ───────────────
            .additional_browser_args(
                "--enable-features=msTranslate \
                 --disable-features=msSmartScreenProtection \
                 --no-first-run"
            )
            .on_navigation(|url| {
                let host = url.host_str().unwrap_or("");
                let scheme = url.scheme();

                if scheme != "http" && scheme != "https" {
                    return true;
                }

                if host == "localhost" || host == "tauri.localhost" || host.ends_with(".localhost") {
                    return true;
                }

                let is_tiktok = host == "tiktok.com" || host.ends_with(".tiktok.com");
                if is_tiktok {
                    return true;
                }

                eprintln!("[TikTok-Now] External link intercepted ({}), opening in OS default browser...", url);
                let _ = open::that(url.as_str());
                false
            })
            // ── Titlebar Username & "Logged in" Toast Auto-Close Popup Sweeper ──
            .on_document_title_changed(move |window, page_title| {
                let app = window.app_handle().clone();
                if page_title.contains("LOGGED_IN") || page_title.starts_with("TIKTOKNOW:") {
                    let user = page_title.trim_start_matches("TIKTOKNOW:").trim_start_matches("TIKTOKNOW_LOGGED_IN").to_string();
                    if !user.is_empty() {
                        *cache_for_title.lock().unwrap() = Some(user.clone());
                    }

                    // Immediately close ALL open popup windows upon "Logged in" toast detection!
                    let mut closed_any = false;
                    for (label, popup_win) in app.webview_windows() {
                        if label.starts_with("popup-") {
                            eprintln!("[TikTok-Now] Logged-in toast/title detected. Closing popup window '{}' immediately!", label);
                            let _ = popup_win.close();
                            closed_any = true;
                        }
                    }

                    if closed_any {
                        let _ = window.eval("window.location.reload();");
                    }

                    let new_title = if user.is_empty() {
                        "TikTok-Now".to_string()
                    } else {
                        format!("TikTok-Now (@{})", user)
                    };
                    let _ = window.set_title(&new_title);
                } else {
                    let cached = cache_for_title.lock().unwrap().clone();
                    if let Some(u) = cached {
                        let _ = window.set_title(&format!("TikTok-Now (@{})", u));

                        // Sweep popups for active cached user
                        for (label, popup_win) in app.webview_windows() {
                            if label.starts_with("popup-") {
                                eprintln!("[TikTok-Now] Active session (@{}). Closing popup window '{}'!", u, label);
                                let _ = popup_win.close();
                            }
                        }
                    } else {
                        let _ = window.eval(r#"
                            (function() {
                                try {
                                    var avatar = document.querySelector('a[href*="/@"]');
                                    if (avatar) {
                                        var href = avatar.getAttribute('href') || '';
                                        var m = href.match(/\/@([a-zA-Z0-9_\.]+)/);
                                        if (m && m[1]) {
                                            document.title = 'TIKTOKNOW:' + m[1];
                                        }
                                    }
                                } catch(e) {}
                            })();
                        "#);
                    }
                }
            })
            // ── Injected Instant Dark Background & Desktop Engine ────────────────
            .initialization_script(r##"
                (function() {
                    'use strict';

                    // 0. Force Immediate Dark Canvas on Page Navigation
                    var forceDarkCSS = document.createElement('style');
                    forceDarkCSS.id = 'tiktok-now-dark-canvas';
                    forceDarkCSS.textContent = 'html, body { background-color: #0d0e15 !important; color: #ffffff !important; }';
                    (document.head || document.documentElement).appendChild(forceDarkCSS);

                    // 0b. DOM Toast Observer: Watches for "Logged in" Toast Notification & Triggers Instant Popup Close
                    new MutationObserver(function() {
                        try {
                            var text = document.body ? document.body.innerText : '';
                            if (text.indexOf('Logged in') !== -1 || text.indexOf('Login success') !== -1) {
                                var avatar = document.querySelector('a[href*="/@"]');
                                if (avatar) {
                                    var href = avatar.getAttribute('href') || '';
                                    var m = href.match(/\/@([a-zA-Z0-9_\.]+)/);
                                    if (m && m[1]) {
                                        document.title = 'TIKTOKNOW:' + m[1];
                                    } else {
                                        document.title = 'TIKTOKNOW_LOGGED_IN';
                                    }
                                } else {
                                    document.title = 'TIKTOKNOW_LOGGED_IN';
                                }
                            }
                        } catch(e) {}
                    }).observe(document.documentElement, { childList: true, subtree: true, characterData: true });

                    // 0c. Captcha & Slider Dragging Fix: Prevents native WebView HTML5 dragstart from cancelling pointermove/mousemove
                    document.addEventListener('dragstart', function(e) {
                        if (e.target) {
                            var tag = e.target.tagName ? e.target.tagName.toUpperCase() : '';
                            if (tag === 'IMG' || tag === 'SVG' || e.target.closest('[class*="captcha"]') || e.target.closest('[class*="sec-captcha"]') || e.target.closest('[class*="slider"]') || e.target.closest('[class*="verify"]') || e.target.closest('[class*="puzzle"]')) {
                                e.preventDefault();
                            }
                        }
                    }, true);

                    var captchaCSS = document.createElement('style');
                    captchaCSS.id = 'tiktok-now-captcha-fix';
                    captchaCSS.textContent = `
                        [class*="captcha"], [class*="sec-captcha"], [class*="slider"], [class*="verify"], [class*="puzzle"] {
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
                    var observer = new IntersectionObserver(function(entries) {
                        entries.forEach(function(entry) {
                            var vid = entry.target;
                            if (entry.isIntersecting && entry.intersectionRatio >= 0.5) {
                                if (activeVideo && activeVideo !== vid) {
                                    activeVideo.pause();
                                }
                                vid.play().catch(function(){});
                                activeVideo = vid;
                            } else if (!entry.isIntersecting && activeVideo === vid) {
                                vid.pause();
                                activeVideo = null;
                            }
                        });
                    }, { threshold: [0.5] });

                    function setupVideoListeners(v) {
                        observer.observe(v);
                        if (!v.dataset.hasEndedListener) {
                            v.dataset.hasEndedListener = 'true';
                            v.addEventListener('ended', function() {
                                if (window.autoScrollEnabled) {
                                    window.scrollBy({ top: window.innerHeight * 0.85, behavior: 'smooth' });
                                }
                            });
                        }
                    }

                    function observeVideos(root) {
                        var vids = root ? root.querySelectorAll('video') : [];
                        vids.forEach(setupVideoListeners);
                    }

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

                    // 6. Native Desktop Navigation Engine (Arrow keys, Space, Auto-scroll A, Refresh R)
                    window.addEventListener('keydown', function(e) {
                        if (['INPUT', 'TEXTAREA'].includes(document.activeElement.tagName) || document.activeElement.isContentEditable) {
                            return;
                        }

                        var key = e.key.toLowerCase();
                        var v = activeVideo || document.querySelector('video');

                        if (e.key === 'ArrowDown' || key === 'j') {
                            window.scrollBy({ top: window.innerHeight * 0.85, behavior: 'smooth' });
                        } else if (e.key === 'ArrowUp' || key === 'k') {
                            window.scrollBy({ top: -window.innerHeight * 0.85, behavior: 'smooth' });
                        } else if (e.code === 'Space') {
                            e.preventDefault();
                            if (v) { v.paused ? v.play() : v.pause(); }
                        } else if (key === 'a') {
                            window.toggleAutoScroll();
                        } else if (key === 'r') {
                            window.location.reload();
                        }
                    });

                    // 7. Periodic Username Extractor
                    setInterval(function() {
                        try {
                            var avatar = document.querySelector('a[href*="/@"]');
                            if (avatar) {
                                var href = avatar.getAttribute('href') || '';
                                var m = href.match(/\/@([a-zA-Z0-9_\.]+)/);
                                if (m && m[1]) {
                                    document.title = 'TIKTOKNOW:' + m[1];
                                }
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
                            var isLocal = host === 'localhost' || host === 'tauri.localhost' || host.endsWith('.localhost') || u.protocol === 'file:' || u.protocol === 'tauri:';
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
                })();
            "##)
            // ── Clean OAuth Popup Window Handler & External Hyperlink Router ─────
            .on_new_window(move |url, features| {
                let host = url.host_str().unwrap_or("");
                let path = url.path();
                let is_tiktok = host == "tiktok.com" || host.ends_with(".tiktok.com");
                let is_auth_provider = host.contains("google")
                    || host.contains("apple")
                    || host.contains("facebook")
                    || host.contains("twitter")
                    || host.contains("live")
                    || host.contains("microsoft")
                    || path.contains("login")
                    || path.contains("auth")
                    || path.contains("sso")
                    || path.contains("passport");

                if is_auth_provider || (is_tiktok && (path.contains("login") || path.contains("passport"))) {
                    let label = format!(
                        "popup-{}",
                        POPUP_COUNTER.fetch_add(1, Ordering::Relaxed)
                    );
                    eprintln!("[TikTok-Now] Intercepted OAuth login window: {}", url);

                    let (w, h) = features
                        .size()
                        .map(|s| (s.width as f64, s.height as f64))
                        .unwrap_or((540.0, 700.0));

                    #[cfg(windows)]
                    let env = features.opener().environment.clone();

                    let app_close = app_handle.clone();
                    let label_close = label.clone();
                    let close_once = Arc::new(AtomicBool::new(false));
                    let flag_nav = close_once.clone();

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
                    .initialization_script(r#"
                        (function() {
                            var darkStyle = document.createElement('style');
                            darkStyle.textContent = 'html, body { background-color: #0d0e15 !important; color: #fff !important; }';
                            (document.head || document.documentElement).appendChild(darkStyle);

                            setInterval(function() {
                                var isTikTok = location.hostname.endsWith('tiktok.com');
                                var isLoginSelection = location.pathname.endsWith('/login') || location.pathname.endsWith('/login/');
                                if (isTikTok && !isLoginSelection) {
                                    window.close();
                                }
                            }, 300);
                        })();
                    "#)
                    .on_navigation(move |nav_url| {
                        let host = nav_url.host_str().unwrap_or("");
                        let path = nav_url.path();
                        let is_tiktok = host == "tiktok.com" || host.ends_with(".tiktok.com");
                        let is_login_selection = path == "/login" || path == "/login/";

                        if is_tiktok && !is_login_selection && !flag_nav.load(Ordering::Relaxed) {
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
                    eprintln!("[TikTok-Now] External hyperlink in new window ({}), launching OS default browser...", url);
                    let _ = open::that(url.as_str());
                    NewWindowResponse::Deny
                } else {
                    NewWindowResponse::Allow
                }
            })
            .build()
            .expect("Failed to build TikTok-Now main window");

            let _ = _window.set_size(tauri::Size::Logical(tauri::LogicalSize { width: 1250.0, height: 900.0 }));
            let _ = _window.center();

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if window.label() == "main" {
                    if let Some(main) = window.get_webview_window("main") {
                        let _ = main.eval(
                            "var vids = document.querySelectorAll('video'); vids.forEach(function(v) { v.pause(); });"
                        );
                        let _ = main.hide();
                    }
                    api.prevent_close();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::check_auth,
            commands::auth::jump_to_external,
            commands::navigation::navigate_to,
            commands::system::get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TikTok-Now v1.0.0");
}
