# Changelog — TikTok-Now

All notable changes to **TikTok-Now** are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

---

## [2.0.0] — 2026-08-07

> **v2.0.0** is the milestone release: everything since the v1.0.0 initial commit — three rounds of fixes and improvements — consolidated and polished. The headline fix below completes the photo-post experience.

### Fixed
- **Photo-post dragging no longer pauses the post or selects the image** (the "weird" behavior) —
  - **Multi-image posts**: dragging left/right still flips slides with visual feedback, but the release click that used to toggle TikTok's slideshow pause is now **suppressed** — dragging never pauses anymore.
  - **Single-photo posts (smart handling)**: a drag attempt is detected within 6 px of movement, so the image is never selected or dragged as a native HTML element, and no pause click fires. A clean tap (no movement) still behaves exactly like TikTok intended — the "delay" the fix needs is built in.
  - **Vertical drags** that start on a photo (normal feed scrolling) also no longer pause the post on release.
  - **Video posts are completely untouched** — every previously fixed behavior is preserved.
- **Sound on every fresh launch — the real fix.** Two root causes, both fixed:
  1. **TikTok's player re-mutes after its own init** — during a **60-second startup grace window**, every `play` event plus a 1-second interval re-enforces unmuted + 50% volume on the *active* video (guarded so it never fights the pause-on-minimize mute-backup).
  2. **Windows persists the app's audio-session mute across restarts** (per-app Volume Mixer store). If the previous session exited while muted (pause-on-minimize), the next cold start began OS-muted — and with no hidden→visible transition ever firing, nothing unmuted it: the video played fine while the OS muted the sound. The watchdog now **retries an OS-level unmute every 800 ms from ~2.4 s after launch** until the WebView2 session exists and is cleared (capped at ~32 s; skipped when autostarting hidden).
  Diagnostics prove audio actually flows on cold start: the playing video reports `mediaMuted:false` / `mediaVolume:0.5` and the Windows session reports `muted=no` with a **peak meter above zero** (`peak=0.18` observed).
- **Minimize → restore also recovers audio-only feeds** — TikTok sometimes serves a state with an `<audio>` element and no `<video>`; the resume path now brings the audio back on restore (previously the app could come back silent).
- **macOS/Linux CI builds fixed** — the `windows` dependency (Core Audio session mute, `audio.rs`) was declared unconditionally, so its transitive `windows-future 0.2.1` compiled on macOS/Linux where it fails (`windows_core::imp::IMarshal` only exists on Windows). The dependency and `audio.rs` are now properly `cfg(windows)`-gated — identical to how Tauri itself gates it — and the Windows audio features are unchanged.

### All changes since v1.0.0 (cumulative)
- 🔇 **Pause on minimize & close** — audio stops the instant the window is hidden and resumes where you left off on restore (page pause + TikTok state-machine sync + Rust watchdog + OS-level Core Audio session mute = guaranteed silence).
- 🖱️ **Tray-icon restore** — clicking the tray icon always restores the app to the front as the active window, even from minimized (was: taskbar button vanished).
- 🔉 **Autoplay with sound** — the first video of a fresh launch plays **unmuted at 50% volume** (WebView2 autoplay policy + startup grace enforcement + OS-session mute cleared at launch).
- 🖼️ **Photo-post drag navigation** — swipe through multi-image carousels, smart single-photo handling, `←`/`→` keys flip photo slides too.
- 🧩 **Close-to-tray reliability** — closing always parks the app in the tray (paused) instead of occasionally quitting.
- 🚀 **Launch on Startup actually works** — the tray toggle really enables/disables OS autostart; autostart opens hidden to the tray.
- ⌨️ **All tray-advertised shortcuts now work** — `M` mute, `P` picture-in-picture, `S` capture frame, `←`/`→` seek ±5 s, `A` auto-scroll, `R` refresh, Space with button-focus guard.
- 📋 **Capture Video Frame & Copy Video Link** implemented (previously dead menu entries).
- ⚡ **Engineering fixes** — init-script root cause (WebView2 injects before the DOM exists — now deferred with a safety net); watchdog state polling; OAuth popup no longer killed by title churn; mute only affects the active video; live streams seek-safe.
- 🪶 **Leaner binary** — removed 9 unused dependencies (`tokio`, `reqwest`, `chrono`, `uuid`, `anyhow`, `log`, `env_logger`, `serde_json`, `url`), trimming size and build time.
- 📦 **Cross-platform publishing** — GitHub Actions builds and publishes Windows, macOS (universal), and Linux assets; manual-dispatch tag bug fixed.

---

## [1.6.0] — 2026-08-07

### Added
- **Drag to browse photo carousels** — on multi-image posts, drag horizontally (left = next image, right = previous) instead of aiming for TikTok's small arrow buttons. The images follow your cursor while dragging; releasing past the threshold snaps to the next/previous slide by clicking TikTok's own arrows, so the site's animation and counters stay consistent. Vertical drags are untouched (feed scrolling preserved), small drags and drags on video posts do nothing.
- **`←` / `→` keys navigate photo posts too** — when the current post is a carousel (no video to seek), the arrow keys flip between images.

### Fixed
- *(No fixes in this release — see 1.5.0.)*

---

## [1.5.0] — 2026-08-07

### Fixed
- **Autoplay with sound on launch** — the first video now autoplays **unmuted** (previously every fresh launch started muted because WebView2 blocks audible autoplay until a user gesture, and TikTok's own player initializes the first video muted). Two layers: `--autoplay-policy=no-user-gesture-required` in the webview launch args, plus a one-time startup unmute on the session's first `play` event (after that, the user's own mute/volume choices are never touched).
- **Pause on minimize & close** — the playing video/post pauses instantly when the window is minimized or closed-to-tray, and resumes where it left off when the window is restored. Previously the clip kept playing (with audio) in the background. Root cause: the page-side script was dying at injection time (WebView2 runs it before the DOM exists), so none of the playback helpers ever loaded on TikTok pages — the script now defers to `DOMContentLoaded` with a safety net, and a Rust watchdog polls the real window state, a synthetic center-click syncs TikTok's own player state machine, and `<audio>` elements are covered too.
- **Guaranteed silence** — while hidden, the app also mutes its own Windows audio sessions (Core Audio, covering the WebView2 child processes), so no sound can escape even if every page-side mechanism fails; sessions unmute on restore.
- **Tray icon restore** — clicking the tray icon while the window is minimized now restores it to the front as the active window (unminimize + show + focus). Previously `is_visible()` reported a minimized window as "visible", so the toggle hid it and removed its taskbar button. Rule: restore when minimized/hidden/unfocused; hide only when visible **and** focused.
- **Close-to-tray reliability** — closing the window always parks it in the system tray (paused) instead of occasionally quitting the app entirely.
- **Launch on Startup** — the tray toggle now actually enables/disables OS autostart (was a no-op); autostart launches the app hidden to the tray (`--minimized`).
- **Keyboard shortcuts** — tray-advertised keys now work: `M` mute/unmute, `P` picture-in-picture, `S` capture frame, `←` / `→` rewind/forward 5 s.
- **Capture Video Frame & Copy Video Link** — implemented (previously dead menu entries that called undefined functions).
- **Mute & seek refinements** — mute affects only the active video instead of flipping every `<video>` on the page; live streams (infinite duration) are no longer seeked into invalid positions; Space no longer hijacks focused buttons (Like/Comment keep native keyboard activation).
- **OAuth re-login** — in-progress login popups are no longer closed by background title changes; the title extractor no longer rewrites the page title every second.
- **Leaner binary** — removed unused dependencies (`tokio`, `reqwest`, `chrono`, `uuid`, `anyhow`, `log`, `env_logger`, `serde_json`, `url`), trimming binary size and build time.

---

## [1.0.0] — 2026-08-07

### Added
- Initial public release of **TikTok-Now** desktop application for Windows, macOS, and Linux.
- **Tauri v2 + Rust architecture**: Sub-10 MB standalone executable binary targeting `https://www.tiktok.com/`.
- **IntersectionObserver Autoplay Engine**: Automatic playback of focused feed videos; background videos pause to conserve resources.
- **Official TikTok Buttons Integration**: Native integration with official TikTok video controls (Like, Comment, Bookmark, Share).
- **Floating Toast Notifications**: Action feedback overlaid on-screen.
- **System Tray**: Always-running tray icon with full feed navigation submenu (For You, Following, Friends, Explore, Live, Upload), playback controls, and settings.
- **OAuth Popup Auto-Close**: Login flow opens a slim popup that closes automatically once the user is authenticated; main window reloads.
- **Seamless About Modal**: In-page overlay (not a separate window) with app info and author credits, launched from the tray.
- **Default Window Size**: 1250 × 900, centered on launch.
- **Always-on-Top Pin**: Toggle from tray menu.
- **Picture-in-Picture**: Via tray menu or `P` key.
- **Auto-Scroll Mode**: Automatically scrolls to next video; toggle via `A` key or tray.
- **Playback Speed Control**: 1.0×, 1.5×, 2.0× from tray submenu.
- **URL Copy**: Copy current video URL from tray.
- **Cache Clear**: Reset localStorage + sessionStorage from tray.
- **Minimize-to-Tray on Close**: Window hides on close (stays alive in background); left-click tray icon to restore.
- **Clean Desktop CSS**: Injects styles to remove mobile download banners and app-install overlays.
- **Cross-platform Builds**: Supports Windows (x86_64), macOS (Universal — Intel + Apple Silicon), and Linux (x86_64).
- **GitHub Actions CI/CD**: `release.yml` workflow builds and publishes release assets for all 3 platforms on version tag push.

---
