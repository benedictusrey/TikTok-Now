# Release Notes — TikTok-Now 1.0.0

🎉 **TikTok-Now 1.0.0** is the initial public release of the high-performance desktop client for TikTok (`https://www.tiktok.com/`).

---

## Highlights

- ⚡ **Sub-10 MB Footprint**: Powered by **Tauri v2 + Rust**. Renders natively without bundling Chromium or Node.js.
- 🎯 **Optimized Geometry**: Starts cleanly at **1250 × 900** resolution, centered automatically on launch.
- 🎥 **Smart Autoplay Engine**: Uses `IntersectionObserver` to automatically pause videos when scrolled out of view, reducing CPU and memory consumption.
- 🔐 **Isolated OAuth Login Popup**: Dedicated window handler for Google, Apple, and TikTok sign-in popups; automatically detects authentication success and closes cleanly.
- 🔔 **System Tray Integration**: Full background tray support with single-click show/hide, feed switching (For You, Following, Friends, Explore, Live, Upload), mute toggles, and cache clearing.
- ℹ️ **Seamless In-Page About Modal**: Modern backdrop-filtered overlay with project details and quick links, launched directly from the tray.
- 🚀 **GitHub Actions CI/CD**: Workflow building binaries for Windows (x86_64), macOS (Universal Binary), and Linux (x86_64) on tag push.

---

## Published Assets

- `TikTok-Now_1.0.0_windows-x86_64.exe`
- `TikTok-Now_1.0.0_macos-universal`
- `TikTok-Now_1.0.0_linux-x86_64`
- `checksums.txt` (SHA-256)

---
*Authored and maintained with ❤️ by [@benedictusrey](https://github.com/benedictusrey)*
