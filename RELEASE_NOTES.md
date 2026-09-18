# 🎬 Release Notes — TikTok-Now 2.1.0

<p align="center">
  <img src="docs/assets/icon.png" width="96" height="96" alt="TikTok-Now Icon"><br>
  <strong>TikTok-Now v2.1.0 — The Audit Release</strong><br>
  <em>Faster launch, hardened security, honest UX — and Messages, done right.</em>
</p>

<p align="center">
  <img src="docs/assets/TikTok-Now%20Hero.png" width="860" alt="TikTok-Now v2.1.0 Hero Showcase">
</p>

---

🎉 **TikTok-Now 2.1.0** is the result of a full audit of the v2.0.0 codebase. Every dead code path was removed, every leak closed, the initial loading page was made near-instant, and the Direct Messages experience inside the app is now first-class and DM-aware. Nothing that worked in v2.0.0 was broken: photo drag, sound-on-launch, pause-on-minimize, and tray restore all behave exactly as before.

---

## 🚀 What's New in 2.1.0

### Near-instant initial loading page
- **350 ms splash** (was 2,200 ms) — and `location.replace` means Back can never strand you on a dead splash loop.
- **Connection pre-warming** — DNS + TLS to `www.tiktok.com` start heating up while the splash animates.
- **Offline auto-recovery** — if you launch offline, the app redirects the moment connectivity returns (the Retry button still works, instantly).
- **Compositor-only animations** — the equalizer bars no longer force layout on every frame.

### 🪟 Exact-fit default window (1326 × 1032)
- The visible frame (title bar included) now lands **precisely** between the screen's top edge and the Windows taskbar — pixel-exact on the reference machine (1920×1080 @ 100 % DPI), and still exact everywhere else because the work area is **measured at runtime** (DWM extended frame bounds + `Monitor::work_area`), covering 125 %/150 % DPI, different taskbar sizes, and multi-monitor.
- The fit settles during the splash's first paint — imperceptible — and hidden autostarts are fitted on first restore.

### 🌑 Flash-free navigation
- **No white/black flashing** when the splash hands over to TikTok, when jumping feeds from the tray, or when sign-in popups open: the dark canvas is painted at true document-start (before the first frame commits), and window geometry corrections never land mid-navigation.

### 💬 Messages (Direct Messages)
- **💬 Direct Messages** tray entry jumps straight to TikTok's DM page via the same audited whitelist as every other feed — and reveals the window.
- **DM-aware engine** — no synthetic play/pause clicks in chat (they could scroll conversations or trigger focused message actions), no auto-scroll, and feed-only tray actions are safely inert.
- **Zero keyboard leakage** — shortcuts are ignored while Ctrl/Alt/⌘ are held and while you type in any input, textarea, select, or contenteditable — including the Messages composer. `Esc` blurs the field; nothing else is hijacked.
- **Drag safety** — the photo-carousel drag logic never claims gestures inside chats.

### 🔒 Security & privacy fixes
- **`jump_to_external` is now scheme-allowlisted** (`http`/`https`/`mailto`) — previously it forwarded any string it received to the OS shell.
- **OAuth interceptor fixed** — the substring `live` matched any hostname containing those letters; only real provider domains are matched now.
- **Release builds are silent** — v2.0.0 logged URLs, titles (with your handle), and playback states to stderr even in release builds. All diagnostics are debug-only now.
- **Strict CSP on local pages** — the splash and About pages can only load their own scripts/styles; no remote connections, and inline code was externalized to comply.
- **Dead surface removed** — 4 unused Tauri plugins, the SQLite/moka stack with its `db`/`state` modules, and the dead `check_auth` command (with its ACL grant) are gone; the remote capability is TikTok-only.

### ✨ UX honesty pass
- **Screen-time passcode box is typeable again** — TikTok's "Ready to close TikTok?" dialog inherits `user-select: none` from the captcha-slider CSS (its container carries a `verify`-family class). Form controls (`input`, `textarea`, `select`, `contenteditable`) now strictly retain native text selection, drag, and typing; captcha styles are scoped away from form controls; and carousel release-click suppression never triggers on a text field.
- **No fake confirmations** — "📋 URL copied" now means the clipboard actually received the URL.
- **Restore keeps your window** — un-minimizing from the tray no longer resizes/re-centers it.
- **Truthful tray menu** — Always-On-Top and Launch-on-Startup labels stay in sync and toast on change.
- **"Clear Web Cache" clears everything** — Cache Storage + service workers included, then reloads.
- **About is version-accurate forever** — the dialog version is generated from the build itself.

---

## 🛤️ Everything Changed Since v1.0.0

| Since v1.0.0 | What changed |
|---|---|
| 🔇 **Pause on minimize & close** | Audio stops the instant the window is hidden; resumes where you left off on restore. Backed by a Rust watchdog + OS-level audio-session mute — silence is guaranteed even if the site misbehaves. |
| 🖱️ **Tray-icon restore** | Clicking the tray icon always restores the app to the front as the active window (and since 2.1.0, without resizing your window). |
| 🪟 **Exact-fit window (2.1.0)** | The default 1326 × 1032 visible frame fits precisely between screen top and taskbar — work-area measured at runtime, so it holds at any DPI/taskbar. |
| 🌑 **Flash-free (2.1.0)** | Dark canvas painted before first paint + navigation-safe geometry corrections — no white/black flashes on launch or feed jumps. |
| 🔉 **Autoplay with sound** | The first video of a fresh launch plays **unmuted at 50% volume** (startup grace enforcement + persisted Windows session mute cleared automatically). |
| 🖼️ **Photo-post drag** | Swipe through multi-image carousels; single photos are drag-safe (no pause, no selection); `←`/`→` keys flip photo slides too. |
| 💬 **Messages (2.1.0)** | Direct tray entry + a DM-aware engine: no synthetic clicks in chat, no auto-scroll, no shortcut leakage into the composer. |
| ⌨️ **Shortcuts that work** | `M` mute · `P` picture-in-picture · `S` capture frame · `←`/`→` seek ±5 s · `A` auto-scroll · `R` refresh — and since 2.1.0 they never fire while typing or with modifier keys held. |
| 🚀 **Launch on Startup** | The tray toggle really enables/disables OS autostart; autostart opens hidden to the tray; the label stays truthful. |
| 🧩 **Reliable close-to-tray** | Closing always parks the app in the tray (paused) — never quits by accident. |
| 🔐 **Security hardening (2.1.0)** | Scheme-allowlisted external opener, fixed OAuth host matching, release-build silence, strict local-page CSP, dead plugins/commands removed. |
| ⚡ **Leaner binary** | 9 dependencies removed in 1.5.0, 6 more in 2.1.0 — smaller executable, faster builds. |
| 📦 **Real publishing** | GitHub Actions builds & publishes Windows, macOS (universal), and Linux assets. |

---

## 📦 Download Matrix

| Platform | Binary | Architecture | Notes |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | `TikTok-Now_v2.1.0_windows-x86_64.exe` | x86_64 | Portable standalone binary. No setup required. |
| 🍎 **macOS** | `TikTok-Now_v2.1.0_macos-universal` | Universal (Intel + M1/M2/M3/M4) | Single universal binary for all Macs. |
| 🐧 **Linux** | `TikTok-Now_v2.1.0_linux-x86_64` | x86_64 | Native executable targeting GTK3 + WebKit2GTK 4.1. |

> 💡 **Upgrading from any older version?** Just replace the executable — your login, cookies, and settings are stored by the OS webview and carry over automatically.

---

## 🔑 Verification & Checksums

Check release integrity using SHA-256 hashes provided in `checksums.txt`:

```powershell
# Windows PowerShell
Get-FileHash TikTok-Now_v2.1.0_windows-x86_64.exe -Algorithm SHA256
```

---

## 📜 History

- **v2.1.0** — The audit release: instant launch, security fixes, Messages experience, honest UX.
- **v2.0.0** — Photo drag perfected; sound on every fresh launch; CI fixed for macOS/Linux.
- **v1.6.0** — Drag to browse photo carousels; arrow keys navigate photo posts.
- **v1.5.0** — Pause on minimize/close, guaranteed silence, tray restore, working autostart & shortcuts.
- **v1.0.0** — Initial public release (see [CHANGELOG.md](CHANGELOG.md) for the full history).

---

*Authored and maintained with ❤️ by **Benedictus Reynaldo Hartanto** ([@benedictusrey](https://github.com/benedictusrey))*
