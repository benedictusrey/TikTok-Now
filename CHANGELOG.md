# Changelog — TikTok-Now

All notable changes to **TikTok-Now** are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

---

## [2.1.0] — 2026-09-18

> **v2.1.0** is the audit release: a full pass over the v2.0.0 code — faster cold launch, removal of every dead dependency and code path, closure of real security/leakage findings, and a first-class, DM-safe **Messages** experience.

### 🚀 Performance — initial loading page
- **Near-instant launch** — the splash now redirects after **350 ms** (was 2,200 ms) via `location.replace`, so the splash page never pollutes history (Back can never strand you on a dead splash loop).
- **Connection pre-warming** — `preconnect`/`dns-prefetch` to `www.tiktok.com` open the TLS+DNS path while the splash animates, so the real navigation reuses a warm socket.
- **Offline auto-recovery** — an offline start now auto-redirects the moment the OS reports connectivity back (`online` event); the retry button is still there and reacts instantly instead of waiting a blind second.
- **Compositor-only splash animations** — the equalizer bars animate via `transform: scaleY` instead of `height`, eliminating per-frame layout work on low-end machines.

### 💬 Messages (Direct Messages) — audited & first-class
- **"💬 Direct Messages" tray entry** navigates to TikTok's DM page (`/messages`) with the same audited whitelist path as every other feed.
- **DM-aware playback engine** — on the Messages page the app no longer fires synthetic play/pause center-clicks (they could scroll the chat or activate a focused message action), never auto-scrolls, and Play/Next/Prev/seek/speed tray actions are safely inert.
- **No more keyboard leakage into the composer** — shortcuts are skipped while a modifier key is held (Ctrl+R / Ctrl+F stay native), while typing in ANY of TikTok's inputs — `INPUT`, `TEXTAREA`, `SELECT`, `contenteditable` widgets — including a new persistent typing guard. `Esc` deliberately blurs the field; nothing else is hijacked.
- **Carousel drag logic never claims DM gestures** — dragging images/emoji in a chat is left entirely to TikTok.

### 🔒 Security & privacy fixes (audit findings)
- **`jump_to_external` hardened** — the command used to forward ANY string to the OS shell opener. It now parses the URL and enforces an allowlist (`http`, `https`, `mailto` only) — `javascript:`, `data:`, `file:` etc. are refused.
- **OAuth host matching fixed** — the popup interceptor matched any hostname *containing* the substring `live` (so `liveweb.example.com` became a "sign-in window"). It now matches only the real provider domains (Google, Apple, Facebook, Twitter/X, Microsoft/Live) plus TikTok's own login/passport routes.
- **Release builds no longer emit diagnostics** — v2.0.0 streamed a forensic stderr log (current URLs, page titles with your handle, playback states) in RELEASE builds. All watchdog/audio tracing is now debug-build-only.
- **CSP enforced on local pages** — the splash and About pages previously had `csp: null`; a strict Content-Security-Policy now applies (scripts/styles from `self` only, no remote connections), and their inline code was externalized to comply.
- **Dead attack/maintenance surface removed** — the never-registered `tauri-plugin-shell` and three registered-but-unused plugins (`http`, `store`, `notification`) are gone, along with `rusqlite`+`moka`, the unused `db`/`state` modules, and the dead `check_auth` command and its ACL permission. Remote capability grants are now TikTok-only (was: Google/Facebook/Twitter appleid included regardless of use).

### ✨ UX fixes (audit findings)
- **No more fake "📋 URL copied"** — the toast now reflects the REAL clipboard outcome (async API result + fallback verification) instead of claiming success on failure.
- **Restoring from the tray no longer resizes your window** — v2.0.0 forced 1250×900 + re-center on every restore; your size and position are now preserved.
- **Honest, stateful tray menu** — "Pin Always-On-Top" and "Launch on Startup" labels now stay in sync with reality, and both show an in-page toast on change (the pin used to be a silent toggle).
- **Feed jumps from the tray now reveal the window** — previously they navigated a hidden window and nothing appeared to happen.
- **"Clear Web Cache" actually clears everything** — Cache Storage + service workers are now cleared along with local/session storage, then the page reloads.
- **About dialog version is compile-time generated** — the hardcoded `v2.0.0` string can never drift from the real version again.
- **For You tray entry lands on the actual For You feed** — the menu ID had drifted from the internal URL table, so it silently fell back to the generic root page.
- **Smallest possible remote API** — two IPC commands that existed but were never called (`navigate_to`, `get_app_info`) were removed along with their ACL grants; TikTok pages can now reach exactly one audited command (`jump_to_external`).

### 🪟 Window geometry
- **New default window: 1326 × 1032** — the visible frame (title bar included) fits **exactly** between the screen's top edge and the Windows taskbar on the reference machine (1920×1080 @ 100 % DPI), horizontally centered as before. The work area is **measured at runtime** (`Monitor::work_area` + DWM extended frame bounds), so every other setup — 125 %/150 % DPI, different taskbar sizes, multi-monitor — gets its own exact fit instead of a stale hardcoded height. Hidden `--minimized` autostarts are fitted on first restore (never-shown windows give unreliable DWM measurements), and the fit settles while the splash is still painting its first frame, so it is imperceptible. The fit is applied exactly **once** per launch (an internal latch — restoring from the tray never overrides a size you chose yourself), and a bounded verify loop re-applies both size and position from fresh DWM measurements until the visible frame is exactly on target.
- **No more white/black flashes on navigation and launch** — two root causes found and fixed: (1) the dark-canvas style was applied at DOMContentLoaded, i.e. *after* the first white paint of every new document (splash→feed, tray feed jumps, sign-in popups); it now paints at true document-start via a MutationObserver, before any frame is committed. (2) the geometry verify loop could fire corrective resizes while the webview was crossing documents; its first check now waits until after the splash redirect commits, so in the normal case **zero** extra resizes ever occur.

### 🧹 Code health
- The 1,262-line `lib.rs` window-builder monolith was refactored into auditable constants (`INIT_JS`, `TITLE_JS`, `POPUP_INIT_JS`) plus shared helpers; `eval_quiet` documents the callback-less path; local-origin detection (`127.0.0.1`, `[::1]`, `ipc.localhost` added) and TikTok-host detection are single functions used everywhere.
- The OAuth popup init script no longer dies pre-DOM (head/documentElement can both be null at injection time — the old script threw and silently disabled its own auto-close interval).
- Six dead dependencies removed; binary and build get leaner again.
- `cargo clippy` is now **zero-warning** across the codebase.

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

*Authored and maintained with ❤️ by **Benedictus Reynaldo Hartanto** ([@benedictusrey](https://github.com/benedictusrey))*

