use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // ── Primary Window Controls ──
    let show_hide = MenuItemBuilder::with_id("show_hide", "Show / Hide TikTok-Now").build(app)?;
    let always_top = MenuItemBuilder::with_id("always_top", "📌 Pin Always-On-Top").build(app)?;
    let reload_feed = MenuItemBuilder::with_id("reload_feed", "🔄 Refresh Feed (R)").build(app)?;
    let sep0 = tauri::menu::PredefinedMenuItem::separator(app)?;

    // ── TikTok Feeds Submenu ──
    let for_you = MenuItemBuilder::with_id("for_you", "🔥 For You Feed").build(app)?;
    let following = MenuItemBuilder::with_id("following", "👥 Following Feed").build(app)?;
    let friends = MenuItemBuilder::with_id("friends", "🤝 Friends Feed").build(app)?;
    let explore = MenuItemBuilder::with_id("explore", "🔍 Explore / Search").build(app)?;
    let live = MenuItemBuilder::with_id("live", "🔴 Live Stream Feed").build(app)?;
    let upload = MenuItemBuilder::with_id("upload", "➕ Upload Video").build(app)?;

    let feeds_submenu = SubmenuBuilder::new(app, "🎵 TikTok Feeds")
        .item(&for_you)
        .item(&following)
        .item(&friends)
        .item(&explore)
        .item(&live)
        .item(&upload)
        .build()?;

    // ── Playback Controls Submenu ──
    let toggle_play = MenuItemBuilder::with_id("toggle_play", "⏯️ Play / Pause (Space)").build(app)?;
    let next_video = MenuItemBuilder::with_id("next_video", "⬇️ Next Video (J / Down)").build(app)?;
    let prev_video = MenuItemBuilder::with_id("prev_video", "⬆️ Previous Video (K / Up)").build(app)?;
    let seek_back = MenuItemBuilder::with_id("seek_back", "⏪ Rewind 5s (Left)").build(app)?;
    let seek_forward = MenuItemBuilder::with_id("seek_forward", "⏩ Fast-Forward 5s (Right)").build(app)?;
    let toggle_mute = MenuItemBuilder::with_id("toggle_mute", "🔇 Mute / Unmute (M)").build(app)?;
    let toggle_autoscroll = MenuItemBuilder::with_id("toggle_autoscroll", "📜 Toggle Auto-Scroll (A)").build(app)?;
    let toggle_pip = MenuItemBuilder::with_id("toggle_pip", "🖼️ Picture-in-Picture (P)").build(app)?;

    let speed_10 = MenuItemBuilder::with_id("speed_10", "⚡ 1.0x Normal Speed").build(app)?;
    let speed_15 = MenuItemBuilder::with_id("speed_15", "⚡ 1.5x Speed").build(app)?;
    let speed_20 = MenuItemBuilder::with_id("speed_20", "⚡ 2.0x Speed").build(app)?;

    let playback_submenu = SubmenuBuilder::new(app, "⏯️ Playback Controls")
        .item(&toggle_play)
        .item(&next_video)
        .item(&prev_video)
        .item(&seek_back)
        .item(&seek_forward)
        .item(&toggle_mute)
        .item(&toggle_autoscroll)
        .item(&toggle_pip)
        .item(&speed_10)
        .item(&speed_15)
        .item(&speed_20)
        .build()?;

    // ── App Settings & About ──
    let copy_url = MenuItemBuilder::with_id("copy_url", "📋 Copy Current Video Link").build(app)?;
    let capture_frame = MenuItemBuilder::with_id("capture_frame", "📸 Capture Video Frame (S)").build(app)?;
    let autostart = MenuItemBuilder::with_id("autostart", "🚀 Launch on Startup").build(app)?;
    let clear_cache = MenuItemBuilder::with_id("clear_cache", "🧹 Clear Web Cache").build(app)?;
    let about = MenuItemBuilder::with_id("about", "ℹ️ About TikTok-Now").build(app)?;
    let sep1 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "❌ Quit TikTok-Now").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_hide)
        .item(&always_top)
        .item(&reload_feed)
        .item(&sep0)
        .item(&feeds_submenu)
        .item(&playback_submenu)
        .item(&copy_url)
        .item(&capture_frame)
        .item(&autostart)
        .item(&clear_cache)
        .item(&about)
        .item(&sep1)
        .item(&quit)
        .build()?;

    let icon = app.default_window_icon().cloned().unwrap();

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("TikTok-Now Desktop")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show_hide" => {
                if let Some(main) = app.get_webview_window("main") {
                    if main.is_visible().unwrap_or(false) {
                        let _ = main.eval("var vids = document.querySelectorAll('video'); vids.forEach(function(v) { v.pause(); });");
                        let _ = main.hide();
                    } else {
                        let _ = main.set_size(tauri::Size::Logical(tauri::LogicalSize { width: 1250.0, height: 900.0 }));
                        let _ = main.center();
                        let _ = main.show();
                        let _ = main.set_focus();
                    }
                }
            }
            "always_top" => {
                if let Some(main) = app.get_webview_window("main") {
                    let current = main.is_always_on_top().unwrap_or(false);
                    let _ = main.set_always_on_top(!current);
                }
            }
            "reload_feed" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("window.location.reload();");
                }
            }
            // Feeds
            "for_you" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.eval("window.location.href = 'https://www.tiktok.com/foryou';");
                }
            }
            "following" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.eval("window.location.href = 'https://www.tiktok.com/following';");
                }
            }
            "friends" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.eval("window.location.href = 'https://www.tiktok.com/friends';");
                }
            }
            "explore" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.eval("window.location.href = 'https://www.tiktok.com/explore';");
                }
            }
            "live" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.eval("window.location.href = 'https://www.tiktok.com/live';");
                }
            }
            "upload" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.eval("window.location.href = 'https://www.tiktok.com/upload';");
                }
            }
            // Playback
            "toggle_play" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval(
                        "var v = document.querySelector('video'); if (v) { v.paused ? v.play() : v.pause(); }",
                    );
                }
            }
            "next_video" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("window.scrollBy({ top: window.innerHeight * 0.85, behavior: 'smooth' });");
                }
            }
            "prev_video" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("window.scrollBy({ top: -window.innerHeight * 0.85, behavior: 'smooth' });");
                }
            }
            "seek_back" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("var v = document.querySelector('video'); if(v) v.currentTime = Math.max(0, v.currentTime - 5);");
                }
            }
            "seek_forward" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("var v = document.querySelector('video'); if(v) v.currentTime = Math.min(v.duration || 9999, v.currentTime + 5);");
                }
            }
            "toggle_mute" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval(
                        "var vids = document.querySelectorAll('video'); vids.forEach(v => v.muted = !v.muted);",
                    );
                }
            }
            "toggle_autoscroll" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("if (window.toggleAutoScroll) window.toggleAutoScroll();");
                }
            }
            "toggle_pip" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval(r#"
                        (function() {
                            var v = document.querySelector('video');
                            if (v) {
                                if (document.pictureInPictureElement) {
                                    document.exitPictureInPicture().catch(function(){});
                                } else {
                                    v.requestPictureInPicture().catch(function(){});
                                }
                            }
                        })();
                    "#);
                }
            }
            "speed_10" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("var vids = document.querySelectorAll('video'); vids.forEach(v => v.playbackRate = 1.0);");
                }
            }
            "speed_15" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("var vids = document.querySelectorAll('video'); vids.forEach(v => v.playbackRate = 1.5);");
                }
            }
            "speed_20" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("var vids = document.querySelectorAll('video'); vids.forEach(v => v.playbackRate = 2.0);");
                }
            }
            "copy_url" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("navigator.clipboard.writeText(window.location.href);");
                }
            }
            "capture_frame" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("if(window.captureFrame) window.captureFrame();");
                }
            }
            "autostart" => {
                eprintln!("[TikTok-Now] Autostart menu clicked");
            }
            "clear_cache" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.eval("localStorage.clear(); sessionStorage.clear(); window.location.reload();");
                }
            }
            "about" => {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                    let _ = main.eval(r##"
(function() {
  var ID = '__tiktoknow_about';
  var old = document.getElementById(ID);
  if (old) { old.remove(); return; }

  function el(tag, css, extra) {
    var e = document.createElement(tag);
    if (css) e.style.cssText = css;
    if (extra) Object.assign(e, extra);
    return e;
  }
  function btn(label, cssTxt) {
    var b = el('button', cssTxt);
    b.textContent = label;
    b.onclick = function() { document.getElementById(ID).remove(); };
    return b;
  }

  var overlay = el('div',
    'position:fixed;top:0;left:0;width:100vw;height:100vh;' +
    'background:rgba(0,0,0,0.82);backdrop-filter:blur(12px);' +
    '-webkit-backdrop-filter:blur(12px);display:flex;align-items:center;' +
    'justify-content:center;z-index:2147483647;' +
    'font-family:system-ui,-apple-system,Segoe UI,sans-serif;');
  overlay.id = ID;
  overlay.onclick = function(e) { if(e.target===overlay) overlay.remove(); };

  var card = el('div',
    'background:#0d0e15;border:1px solid #FF007F;border-radius:24px;' +
    'padding:2rem 1.8rem;width:460px;max-width:92vw;max-height:90vh;overflow-y:auto;text-align:center;' +
    'position:relative;box-shadow:0 20px 60px rgba(0,0,0,0.9),0 0 40px rgba(255,0,127,0.25);' +
    'color:#fff;');

  // Close X
  var closeX = btn('\u00d7',
    'position:absolute;top:14px;right:18px;background:none;border:none;' +
    'color:#8e8ea0;font-size:26px;cursor:pointer;line-height:1;');
  card.appendChild(closeX);

  // Logo SVG
  var svgNS = 'http://www.w3.org/2000/svg';
  var svg = document.createElementNS(svgNS, 'svg');
  svg.setAttribute('width', '68'); svg.setAttribute('height', '68');
  svg.setAttribute('viewBox', '0 0 512 512');
  svg.style.cssText = 'filter:drop-shadow(0 0 14px rgba(0,242,254,0.5));margin:0 auto 1rem;display:block;';
  svg.innerHTML = '<rect x="25" y="25" width="462" height="462" rx="80" fill="#0D0E15" stroke="url(#acg18)" stroke-width="18"/>' +
    '<path d="M275 120 h50 v230 h-50 z M325 110 h85 v90 h-85 z M150 275 a65 65 0 1 0 130 0 a65 65 0 1 0 -130 0 Z M110 250 h18 v100 h-18 z M142 190 h18 v160 h-18 z M360 230 h18 v120 h-18 z M392 280 h18 v70 h-18 z" fill="#00F2FE"/>' +
    '<path d="M265 110 h50 v230 h-50 z M315 100 h85 v90 h-85 z M140 265 a65 65 0 1 0 130 0 a65 65 0 1 0 -130 0 Z M100 240 h18 v100 h-18 z M132 180 h18 v160 h-18 z M350 220 h18 v120 h-18 z M382 270 h18 v70 h-18 z" fill="#FF007F"/>' +
    '<path d="M270 115 h50 v230 h-50 z M320 105 h85 v90 h-85 z M145 270 a65 65 0 1 0 130 0 a65 65 0 1 0 -130 0 Z M105 245 h18 v100 h-18 z M137 185 h18 v160 h-18 z M355 225 h18 v120 h-18 z M387 275 h18 v70 h-18 z" fill="#FFF"/>' +
    '<defs><linearGradient id="acg18" x1="0%" y1="0%" x2="100%" y2="100%">' +
    '<stop offset="0%" stop-color="#00F2FE"/><stop offset="50%" stop-color="#9A4EF6"/>' +
    '<stop offset="100%" stop-color="#FF007F"/></linearGradient></defs>';
  card.appendChild(svg);

  // Title
  var h2 = el('h2', 'font-size:1.8rem;font-weight:800;margin-bottom:0.25rem;' +
    'background:linear-gradient(135deg,#00F2FE,#FF007F);' +
    '-webkit-background-clip:text;-webkit-text-fill-color:transparent;');
  h2.textContent = 'TikTok-Now Desktop';
  card.appendChild(h2);

  var sub = el('p', 'color:#00F2FE;font-size:0.88rem;font-weight:bold;margin-bottom:0.2rem;');
  sub.textContent = 'Your Ultimate Desktop Entertainment Hub \ud83c\udfac';
  card.appendChild(sub);

  var ver = el('p', 'color:#8e8ea0;font-size:0.78rem;margin-bottom:1.2rem;');
  ver.textContent = 'v1.0.0 \u2022 Powered by Rust & Tauri v2';
  card.appendChild(ver);

  // Author
  var author = el('div', 'font-size:0.78rem;color:#8e8ea0;margin-bottom:1.5rem;');
  var authorLink = document.createElement('a');
  authorLink.textContent = '@benedictusrey';
  authorLink.href = 'https://github.com/benedictusrey';
  authorLink.style.cssText = 'color:#00F2FE;text-decoration:none;font-weight:bold;cursor:pointer;';
  authorLink.onclick = function(e) {
    e.preventDefault();
    e.stopImmediatePropagation();
    if (typeof window.__tiktoknow_open === 'function') {
      window.__tiktoknow_open('https://github.com/benedictusrey');
    } else {
      window.open('https://github.com/benedictusrey', '_blank');
    }
  };
  author.appendChild(document.createTextNode('Authored and maintained with \u2764\ufe0f by '));
  author.appendChild(authorLink);
  card.appendChild(author);

  // Got It button
  var gotit = btn('Got It!',
    'background:linear-gradient(135deg,#00F2FE,#FF007F);color:#000;font-weight:bold;' +
    'border:none;padding:0.7rem 2.4rem;border-radius:12px;cursor:pointer;' +
    'font-size:0.92rem;box-shadow:0 4px 15px rgba(0,242,254,0.3);');
  card.appendChild(gotit);

  overlay.appendChild(card);
  document.body.appendChild(overlay);
})();
                    "##);
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, .. } = event {
                if button == MouseButton::Left {
                    let app = tray.app_handle();
                    if let Some(main) = app.get_webview_window("main") {
                        if main.is_visible().unwrap_or(false) {
                            let _ = main.eval("var vids = document.querySelectorAll('video'); vids.forEach(function(v) { v.pause(); });");
                            let _ = main.hide();
                        } else {
                            let _ = main.show();
                            let _ = main.set_focus();
                        }
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
