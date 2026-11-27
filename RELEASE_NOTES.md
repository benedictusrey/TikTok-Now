# 🎬 Release Notes — TikTok-Now 2.2.0

<p align="center">
  <img src="docs/assets/icon.png" width="96" height="96" alt="TikTok-Now Icon"><br>
  <strong>TikTok-Now v2.2.0</strong><br>
  <em>Fixes Windows 403 denials, adds a built-in Media Downloader, brands the Windows Volume Mixer, and tunes Windows, macOS, and Linux.</em>
</p>

<p align="center">
  <img src="docs/assets/TikTok-Now%20Hero.png" width="860" alt="TikTok-Now v2.2.0 Hero Showcase">
</p>

---

🎉 **TikTok-Now 2.2.0** resolves Issue #1 (`"Access to www.tiktok.com was denied"` HTTP 403 on Windows), introduces a standalone **Media Downloader** with a stage-anchored Download button, fixes the double-click-to-like blue selection overlay, properly brands the **Windows Volume Mixer**, and brings targeted runtime optimizations across Windows, macOS, and Linux.

---

## 🚀 What's New in 2.2.0

### 🌐 Issue #1 Fix — No More Windows HTTP 403 Denials
- **Restored genuine WebView2 identity** — removed the stale hardcoded `Chrome/126.0.0.0` user-agent override from main and popup webviews, restored `wry` default WebView2 features (`msWebOOUI,msPdfOOUI,msSmartScreenProtection`) alongside occlusion/overlay/autoplay flags, and removed `<meta name="referrer" content="no-referrer">` from the splash screen (`frontend/index.html`).

### ⬇️ Standalone Media Downloader (`downloader.html`)
- **Stage-anchored Download button** — sits at the top-right of the active video/photo stage (never `position: fixed`), shifts automatically around native top-right controls, is exempt from carousel drag gestures, stays clear when the Comments drawer opens, and tracks scrolled For You videos with Comments open or closed. Also accessible from the **"⬇️ Media Downloader"** tray entry.
- **Authentic original-resolution downloads** — supports **Videos** (`H.264`/`HEVC`/`AAC`, DASH/MP4/HLS `.m3u8` with byte-range param stripping, CDN fallback URLs, and automatic `music.playUrl` audio muxing for video-only streams), **Photo Carousels** (`JPG`/`PNG`/`GIF` verbatim, `WebP`/`AVIF`/`HEIC` converted when needed), **Audio `.m4a`** (`AAC 192k`), **Animated GIF** export (256-color diff palette + Floyd-Steinberg dithering), and **Full Post Bundles** (`caption.txt`, `metadata.json`, and media into `Downloads/TikTok-Now/<stem>/` with live progress).
- **Cookie-isolated native transfers** — uses system `curl` (`--config -` stdin) and optional `ffmpeg`. Cookies are strictly confined to `tiktok.com` / `*.tiktok.com` (userinfo `@` bypasses rejected), downloads are magic-byte validated against HTML error pages, and files are written atomically.

### 🖱️ Double-Click Selection Fix
- **No more full-viewport blue highlight on double-click-to-like** — `user-select: none` is scoped to video/player surfaces with capture-phase `selectstart` suppression and `dblclick` range clearing, while inputs, comments, captions, and Direct Messages keep native text selection.

### 🔊 Windows Volume Mixer Branding & Foreground Audio
- **Branded Windows Volume Mixer entry** — audio sessions display `"TikTok-Now"` with the executable's icon and a shared grouping GUID across all active render endpoints (`EnumAudioEndpoints` + hotplug refresh).
- **Crash-safe mute recovery & foreground readiness** — an `audio-muted.flag` marker clears stale mute state if the app previously exited abruptly, `Mode::Release` cleans up on quit/exit, and foreground focus transitions trigger `ensure_foreground_audio_ready()` with a non-fighting `maybeStartupUnmute`.

### ⚡ Performance & Cross-Platform Refinements
- **Scrolling, animations & DOM** — added targeted `scrollFeedBy` container scrolling, `prefers-reduced-motion` support, a 1,500 ms video mount guard, adaptive idle polling, `captureFrame` CORS fallback, native `TTNOWBG:r,g,b` background sync, and reflow-free `MutationObserver` checks; toast notifications now sit cleanly at bottom-center.
- **Windows WebView2 hide-sleep** — `webview_sleep.rs` suspends WebView2 (`TrySuspend`/`Resume`) and lowers memory usage when hidden in the tray, paired with a condvar watchdog that exits cleanly when `main` closes.
- **Linux WebKitGTK & Wayland** — auto-configures `GDK_BACKEND`, targeted `WEBKIT_DISABLE_DMABUF_RENDERER` (on WSLg/NVIDIA/missing DRI3), Skia/GStreamer settings, `CacheModel::WebBrowser`, Wayland-safe hidden-state detection, and strips heavy CSS blur filters on WebKitGTK.
- **macOS** — restores the window on Dock icon clicks (`RunEvent::Reopen`) and preserves `WKWebView` `window.opener` for OAuth popups.
- **Universal work-area clamping** — clamps the `1326 × 1032` default frame to the monitor work area on Windows, macOS, and Linux alike.

---

## 🛤️ Everything Changed Since v1.0.0

| Since v1.0.0 | What changed |
|---|---|
| 🌐 **No 403 blocks (2.2.0)** | Removed stale `Chrome/126` UA override, restored WebView2 defaults, and removed `no-referrer` on splash so TikTok loads reliably on Windows. |
| ⬇️ **Media Downloader (2.2.0)** | Stage-anchored Download button + standalone `downloader.html` for original Videos, Photo Carousels, `.m4a` Audio, `.gif`, and Full Post bundles via `curl` + optional `ffmpeg` with TikTok-only cookie isolation. |
| 🔊 **Volume Mixer & Audio (2.2.0)** | `"TikTok-Now"` + exe icon in Windows Volume Mixer, active endpoint keepalive, crash-safe mute flag, and foreground audio readiness. |
| 🖱️ **Clean double-click & cross-OS polish (2.2.0)** | Fixed double-click selection overlay; added WebView2 hide-sleep, Linux WebKitGTK/Wayland tuning, macOS Dock reopen/OAuth fixes, and cross-OS work-area clamping. |
| 🔇 **Pause on minimize & close** | Audio stops the instant the window is hidden; resumes where you left off on restore. Backed by a Rust watchdog + OS-level audio-session mute — silence is guaranteed even if the site misbehaves. |
| 🖱️ **Tray-icon restore** | Clicking the tray icon always restores the app to the front as the active window (and since 2.1.0, without resizing your window). |
| 🪟 **Exact-fit window (2.1.0)** | The default 1326 × 1032 visible frame fits precisely between screen top and taskbar — work-area measured at runtime, clamped across all OSes in 2.2.0. |
| 🌑 **Flash-free (2.1.0)** | Dark canvas painted before first paint + navigation-safe geometry corrections — no white/black flashes on launch or feed jumps. |
| 🔉 **Autoplay with sound** | The first video of a fresh launch plays **unmuted at 50% volume** (startup grace enforcement + persisted Windows session mute cleared automatically). |
| 🖼️ **Photo-post drag** | Swipe through multi-image carousels; single photos are drag-safe (no pause, no selection); `←`/`→` keys flip photo slides too. |
| 💬 **Messages (2.1.0)** | Direct tray entry + a DM-aware engine: no synthetic clicks in chat, no auto-scroll, no shortcut leakage into the composer. |
| ⌨️ **Shortcuts that work** | `M` mute · `P` picture-in-picture · `S` capture frame · `←`/`→` seek ±5 s · `A` auto-scroll · `R` refresh — and since 2.1.0 they never fire while typing or with modifier keys held. |
| 🚀 **Launch on Startup** | The tray toggle really enables/disables OS autostart; autostart opens hidden to the tray; the label stays truthful. |
| 🧩 **Reliable close-to-tray** | Closing always parks the app in the tray (paused) — never quits by accident. |
| 🔐 **Security hardening (2.1.0)** | Scheme-allowlisted external opener, fixed OAuth host matching, release-build silence, strict local CSP, dead plugins/commands removed, and split per-window capabilities in 2.2.0. |
| ⚡ **Leaner binary** | 9 dependencies removed in 1.5.0, 6 more in 2.1.0; `devtools` gated behind an optional Cargo feature in 2.2.0. |
| 📦 **Real publishing** | GitHub Actions builds & publishes Windows, macOS (universal), and Linux assets. |

---

## 📦 Download Matrix

| Platform | Binary | Architecture | Notes |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | `TikTok-Now_v2.2.0_windows-x86_64.exe` | x86_64 | Portable standalone binary. No setup required. |
| 🍎 **macOS** | `TikTok-Now_v2.2.0_macos-universal` | Universal (Intel + M1/M2/M3/M4) | Single universal binary for all Macs. |
| 🐧 **Linux** | `TikTok-Now_v2.2.0_linux-x86_64` | x86_64 | Native executable targeting GTK3 + WebKit2GTK 4.1. |

> 💡 **Upgrading from any older version?** Just replace the executable — your login, cookies, and settings are stored by the OS webview and carry over automatically.

---

## 🔑 Verification & Checksums

Check release integrity using SHA-256 hashes provided in `checksums.txt`:

```powershell
# Windows PowerShell
Get-FileHash TikTok-Now_v2.2.0_windows-x86_64.exe -Algorithm SHA256
```

---

## 📜 History

- **v2.2.0** — Fixed Windows 403 denial (Issue #1), added Media Downloader (`downloader.html`), fixed double-click selection & Windows Volume Mixer branding, and optimized Windows/macOS/Linux runtime behavior.
- **v2.1.0** — The audit release: instant launch, security fixes, Messages experience, honest UX.
- **v2.0.0** — Photo drag perfected; sound on every fresh launch; CI fixed for macOS/Linux.
- **v1.6.0** — Drag to browse photo carousels; arrow keys navigate photo posts.
- **v1.5.0** — Pause on minimize/close, guaranteed silence, tray restore, working autostart & shortcuts.
- **v1.0.0** — Initial public release (see [CHANGELOG.md](CHANGELOG.md) for the full history).

---

*Authored and maintained with ❤️ by **Benedictus Reynaldo Hartanto** ([@benedictusrey](https://github.com/benedictusrey))*
