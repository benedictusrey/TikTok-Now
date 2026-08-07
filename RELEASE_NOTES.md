# 🎬 Release Notes — TikTok-Now 2.0.0

<p align="center">
  <img src="docs/assets/icon.png" width="96" height="96" alt="TikTok-Now Icon"><br>
  <strong>TikTok-Now v2.0.0 — The Daily-Scrolling Milestone</strong><br>
  <em>Everything since v1.0.0, polished into the TikTok desktop experience it should have been.</em>
</p>

---

🎉 **TikTok-Now 2.0.0** completes the journey from the v1.0.0 initial commit to a truly *daily-scrolling-ready* desktop app. The headline fix: **dragging photo posts no longer pauses them or selects the image** — multi-image posts swipe like a native app, and single-photo posts are smart about it. And the sound is there from the very first second: every fresh launch starts **unmuted at 50% volume**.

---

## 🖼️ What's New in 2.0.0

- **Photo-post dragging fixed.** The previous drag feature had a flaw: releasing a drag *also* delivered a click to TikTok, which toggled the slideshow pause — and slow drags could select/drag the image itself. In 2.0.0:
  - **Multi-image posts** — drag left/right to flip slides with cursor-follow feedback; the release click is suppressed, so the post never pauses.
  - **Single-photo posts** — drag detection kicks in within 6 px of movement, so the image is never selected and no pause fires. A clean tap (no movement) still behaves exactly like TikTok's own click.
  - **Feed scrolling that starts on a photo** — no longer pauses the post on release.
  - **Video posts** — completely untouched; every previously fixed behavior is preserved.
- **Sound on every fresh launch.** Cold starts now come up **unmuted at 50% volume** — no more launching into silence until you minimize and restore:
  - TikTok's player re-mutes during its own init → a 60-second startup grace window re-enforces the unmute on the active video.
  - Windows keeps an app's old audio-session mute across restarts → the watchdog clears it seconds after launch (proven by an OS audio peak meter).
- **Minimize → restore is bulletproof.** Even when TikTok serves an audio-only feed (no video element), the audio is paused on minimize and comes back on restore.

---

## 🛤️ Everything Changed Since v1.0.0

| Since v1.0.0 | What changed |
|---|---|
| 🔇 **Pause on minimize & close** | Audio stops the instant the window is hidden; resumes where you left off on restore. Backed by a Rust watchdog + OS-level audio-session mute — silence is guaranteed even if the site misbehaves. |
| 🖱️ **Tray-icon restore** | Clicking the tray icon always restores the app to the front as the active window, even from minimized (it used to make the taskbar button vanish). |
| 🔉 **Autoplay with sound** | The first video of a fresh launch plays **unmuted at 50% volume** (was: always muted until you clicked — two causes: TikTok re-muting after its own init, and Windows persisting the app's audio-session mute from a hidden exit; both are now cleared automatically at startup, proven by an audio peak meter). |
| 🖼️ **Photo-post drag** | Swipe through multi-image carousels; single photos are drag-safe (no pause, no selection); `←`/`→` keys flip photo slides too. |
| ⌨️ **Shortcuts that work** | `M` mute · `P` picture-in-picture · `S` capture frame · `←`/`→` seek ±5 s (or flip photo slides) · `A` auto-scroll · `R` refresh — all tray-advertised keys now actually work. |
| 🚀 **Launch on Startup** | The tray toggle really enables/disables OS autostart (was a no-op); autostart opens hidden to the tray. |
| 🧩 **Reliable close-to-tray** | Closing always parks the app in the tray (paused) — never quits by accident. |
| 📋 **Dead menu entries revived** | Capture Video Frame & Copy Video Link now work. |
| 🔐 **Login & title handling** | OAuth popups no longer closed by background title churn; the title extractor no longer rewrites the page title every second. |
| ⚡ **Engineering under the hood** | Fixed the root cause of silently-dead page scripts (WebView2 injects before the DOM exists — now deferred with a safety net); mute only affects the active video; live streams are seek-safe. |
| 🪶 **Leaner binary** | Removed 9 unused dependencies (`tokio`, `reqwest`, `chrono`, `uuid`, `anyhow`, `log`, `env_logger`, `serde_json`, `url`) — smaller executable, faster builds. |
| 📦 **Real publishing** | GitHub Actions builds & publishes Windows, macOS (universal), and Linux assets — with a manual-dispatch tag bug fixed. |

---

## 📦 Download Matrix

| Platform | Binary | Architecture | Notes |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | `TikTok-Now_v2.0.0_windows-x86_64.exe` | x86_64 | Portable standalone binary. No setup required. |
| 🍎 **macOS** | `TikTok-Now_v2.0.0_macos-universal` | Universal (Intel + M1/M2/M3/M4) | Single universal binary for all Macs. |
| 🐧 **Linux** | `TikTok-Now_v2.0.0_linux-x86_64` | x86_64 | Native executable targeting GTK3 + WebKit2GTK 4.1. |

> 💡 **Upgrading from any older version?** Just replace the executable — your login, cookies, and settings are stored by the OS webview and carry over automatically.

---

## 🔑 Verification & Checksums

Check release integrity using SHA-256 hashes provided in `checksums.txt`:

```powershell
# Windows PowerShell
Get-FileHash TikTok-Now_v2.0.0_windows-x86_64.exe -Algorithm SHA256
```

---

## 📜 History

- **v1.6.0** — Drag to browse photo carousels; arrow keys navigate photo posts.
- **v1.5.0** — Daily-scrolling optimization: pause on minimize/close, guaranteed silence, tray restore, working autostart & shortcuts, autoplay with sound.
- **v1.0.0** — Initial public release (see [CHANGELOG.md](CHANGELOG.md) for the full history).

---

*Authored and maintained with ❤️ by [@benedictusrey](https://github.com/benedictusrey)*
