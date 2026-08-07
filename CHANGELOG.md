# Changelog — TikTok-Now

All notable changes to **TikTok-Now** are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [1.0.0] — 2026-08-07

### Added
- Initial public release of **TikTok-Now** desktop application for Windows, macOS, and Linux.
- **Tauri v2 + Rust architecture**: Sub-10 MB standalone executable binary targeting `https://www.tiktok.com/`.
- **IntersectionObserver Autoplay Engine**: Automatic playback of focused feed videos; background videos pause to conserve resources.
- **Full Keyboard Shortcuts Suite**:
  | Key | Action |
  |---|---|
  | `Space` | Play / Pause |
  | `J` / `↓` | Next Video |
  | `K` / `↑` | Previous Video |
  | `←` / `→` | Rewind / Fast-Forward 5s |
  | `M` | Mute / Unmute |
  | `[` / `]` | Volume Down / Up |
  | `L` | Like Video |
  | `F` | Toggle Fullscreen |
  | `P` | Picture-in-Picture |
  | `A` | Toggle Auto-Scroll |
  | `R` | Refresh / Reload Feed |
- **Floating Toast Notifications**: Keyboard action feedback overlaid on-screen.
- **System Tray**: Always-running tray icon with full feed navigation submenu (For You, Following, Friends, Explore, Live, Upload), playback controls, and settings.
- **OAuth Popup Auto-Close**: Login flow opens a slim popup that closes automatically once the user is authenticated; main window reloads.
- **Seamless About Modal**: In-page overlay (not a separate window) with app info and keyboard cheatsheet, launched from the tray.
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
