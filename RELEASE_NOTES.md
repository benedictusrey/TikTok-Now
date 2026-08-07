# 🎬 Release Notes — TikTok-Now 1.0.0

<p align="center">
  <img src="docs/assets/icon.png" width="96" height="96" alt="TikTok-Now Icon"><br>
  <strong>TikTok-Now v1.0.0 — Initial Public Release</strong><br>
  <em>Sub-10 MB Standalone Desktop Experience for TikTok</em>
</p>

---

🎉 **TikTok-Now 1.0.0** is officially available! Enjoy a personalized desktop feed, RAM-conscious autoplay engine, clean window geometry, and seamless system tray integration.

---

## 🌟 Release Highlights

- ⚡ **Sub-10 MB Standalone Executable**: Powered by **Tauri v2 + Rust**. Renders natively via OS WebEngine without bundling Chromium or Node.js.
- 🎯 **Optimized Geometry**: Starts cleanly at **1250 × 900** resolution, centered automatically on launch.
- 🎥 **Smart Autoplay Engine**: Uses `IntersectionObserver` to automatically pause videos when scrolled out of view, reducing CPU and memory consumption.
- 🔐 **Isolated OAuth Login Popup**: Dedicated window handler for Google, Apple, and TikTok sign-in popups; automatically detects authentication success and closes cleanly.
- 🔔 **System Tray Integration**: Always-running tray icon with single-click toggle, feed switcher (For You, Following, Friends, Explore, Live, Upload), mute controls, and cache clearing.
- ℹ️ **Seamless In-Page About Modal**: Modern backdrop-filtered overlay with project details and quick links, launched directly from the tray.
- 🌐 **WebView2 Translate-to-English**: Native browser translation bar support for international content on Windows.
- 🚀 **Multi-Platform CI/CD**: Automated GitHub Actions workflow building binaries for Windows (x86_64), macOS (Universal Binary), and Linux (x86_64).

---

## 📦 Download Matrix

| Platform | Binary | Architecture | Notes |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | `TikTok-Now_1.0.0_windows-x86_64.exe` | x86_64 | Portable standalone binary. No setup required. |
| 🍎 **macOS** | `TikTok-Now_1.0.0_macos-universal` | Universal (Intel + M1/M2/M3/M4) | Single universal binary for all Macs. |
| 🐧 **Linux** | `TikTok-Now_1.0.0_linux-x86_64` | x86_64 | Native executable targeting GTK3 + WebKit2GTK 4.1. |

---

## 🔑 Verification & Checksums

Check release integrity using SHA-256 hashes provided in `checksums.txt`:

```powershell
# Windows PowerShell
Get-FileHash TikTok-Now_1.0.0_windows-x86_64.exe -Algorithm SHA256
```

---

*Authored and maintained with ❤️ by [@benedictusrey](https://github.com/benedictusrey)*
