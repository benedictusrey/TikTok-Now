<p align="center">
  <a href="https://github.com/benedictusrey/TikTok-Now">
    <img src="docs/assets/icon.png" width="128" height="128" alt="TikTok-Now Icon">
  </a>
</p>

<h1 align="center">TikTok-Now</h1>

<p align="center">
  <strong>A high-performance, ultra-lightweight desktop experience for TikTok</strong><br>
  Built with <code>Tauri v2</code> + <code>Rust</code> + <code>Native WebView</code>
</p>

<p align="center">
  🎉 <strong>TikTok-Now 1.0.0 is now available</strong> 🎉<br>
  Personalized desktop feed control, RAM-conscious autoplay engine, clean window geometry, and seamless system tray integration.
</p>

<p align="center">
  <a href="https://github.com/benedictusrey/TikTok-Now/releases/latest"><img src="https://img.shields.io/badge/version-1.0.0-213547?style=flat-square" alt="Version 1.0.0"></a>
  <a href="https://github.com/benedictusrey/TikTok-Now/releases/latest"><img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-168B72?style=flat-square" alt="Platform"></a>
  <a href="https://tauri.app/"><img src="https://img.shields.io/badge/built%20with-Tauri%20v2-24A6D8?style=flat-square" alt="Built with Tauri v2"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/built%20with-Rust-B7410E?style=flat-square" alt="Built with Rust"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License MIT"></a>
  <a href="https://github.com/benedictusrey"><img src="https://img.shields.io/badge/author-@benedictusrey-FF007F?style=flat-square" alt="Author @benedictusrey"></a>
</p>

---

**TikTok-Now** is an independent desktop application that combines the official TikTok web service with focused desktop workflow controls. Enjoy your favorite video feeds while adding the essential details that make a desktop experience feel native, responsive, and efficient: smart video pausing, direct keyboard navigation, isolated OAuth login popups, and quiet tray management.

> ⚠️ **Disclaimer**: TikTok-Now is an independent, unofficial open-source desktop client by [@benedictusrey](https://github.com/benedictusrey). It is not affiliated with, endorsed by, or maintained by TikTok Ltd. or ByteDance Ltd. TikTok is a registered trademark of its respective owner.

---

## 🌟 What TikTok-Now adds

<p align="center">
  <img src="docs/assets/tiktoknow-features.png" width="880" alt="TikTok-Now Features Suite"><br>
  <em>Figure 1: Concept artwork illustrating TikTok-Now's desktop productivity suite — featuring quick feed switching (For You, Following, Friends, Live, Explore), smart autoplay video pausing, system tray integration, and isolated OAuth sign-in.</em>
</p>

<br>

<p align="center">
  <img src="docs/assets/tiktoknow-showcase.png" width="880" alt="TikTok-Now Interface & Ease of Use"><br>
  <em>Figure 2: Product showcase demonstrating TikTok-Now's dark-mode viewing canvas, seamless integration with official TikTok video controls (like, comment, bookmark, share), background system tray behavior, and sub-10 MB binary size built on native Rust + Tauri v2 architecture.</em>
</p>

<br>

| Area | TikTok-Now 1.0.0 Capabilities |
|---|---|
| **Feeds Navigation** | Submenu tray shortcuts for **🔥 For You**, **👥 Following**, **🤝 Friends**, **🔍 Explore**, **🔴 Live**, and **➕ Upload** |
| **Autoplay Engine** | Smart `IntersectionObserver` video engine autoplays focused videos and immediately pauses off-screen content to conserve CPU & memory |
| **OAuth Sign-In** | Clean, isolated popup engine for third-party sign-in options; automatically closes upon successful authentication |
| **System Tray** | Native system tray integration with background minimize-to-tray, single-click toggle, feed switcher, and playback controls |
| **Playback Control** | Play/Pause, Next/Previous video, Rewind/Fast-Forward 5s, Mute, Volume adjust, Like video, Fullscreen, and Picture-in-Picture |
| **Auto-Scroll Mode** | Toggleable continuous scrolling mode automatically advancing to the next clip upon video end |
| **Window Geometry** | Perfectly proportioned **1250 × 900** default window, pre-centered on display launch |
| **Clean Desktop CSS** | Injected styles stripping intrusive mobile app download prompts (*"Get App"*, *"Open App"*) for distraction-free viewing |

---

## ⚖️ TikTok-Now and the official web experience

| Feature | TikTok-Now | Standard Web Browser |
|---|---|---|
| **Maintainer** | Independent project by [@benedictusrey](https://github.com/benedictusrey) | Third-party browsers (Chrome, Edge, Safari) |
| **Service Used** | Official TikTok (`https://www.tiktok.com/`) | Official TikTok (`https://www.tiktok.com/`) |
| **Footprint** | Sub-10 MB native binary; zero bundled Chromium | 200 MB–500 MB+ browser process trees |
| **Tray Behavior** | Native tray icon with full background toggle & feed menu | None (closing window terminates session) |
| **OAuth Cleanup** | Automatic popup intercept & auto-closing login window | Manual popup closing required |
| **Distraction Removal** | Automatic CSS removal of mobile download popups | Stock web banners present |
| **Updates** | GitHub Releases | Browser updates |

---

## ⚡ Observed resource efficiency

Unlike traditional Electron-based desktop wrappers that bundle full Chromium instances (consuming 300MB–800MB RAM), **TikTok-Now** leverages **Tauri v2** and **Rust** to render via the native operating system web engine (WebView2 on Windows, WebKit on macOS/Linux).

- **Binary Size**: Sub-10 MB standalone executable.
- **Resource Management**: The built-in `IntersectionObserver` constantly monitors off-screen DOM nodes and pauses background video streams, preventing memory bloat during extended scrolling sessions.
- **Background Mode**: Minimizing to the system tray lowers window graphics overhead while keeping your feed state instant and responsive.

---

## 📦 Download & Platform Support

Download pre-built release binaries from the [latest GitHub Release](https://github.com/benedictusrey/TikTok-Now/releases/latest).

| Platform | Recommended Asset | Notes |
|---|---|---|
| **🪟 Windows (x86_64)** | `TikTok-Now_1.0.0_windows-x86_64.exe` | Portable standalone binary. No installation required. |
| **🍎 macOS (Universal)** | `TikTok-Now_1.0.0_macos-universal` | Combined binary for Apple Silicon (M1/M2/M3/M4) & Intel Macs. |
| **🐧 Linux (x86_64)** | `TikTok-Now_1.0.0_linux-x86_64` | Native Linux executable targeting GTK3 & WebKit2GTK 4.1. |

### Quick Execution
- **Windows**: Double-click `TikTok-Now_1.0.0_windows-x86_64.exe`.
- **macOS**:
  ```bash
  chmod +x TikTok-Now_1.0.0_macos-universal
  ./TikTok-Now_1.0.0_macos-universal
  ```
- **Linux**:
  ```bash
  chmod +x TikTok-Now_1.0.0_linux-x86_64
  ./TikTok-Now_1.0.0_linux-x86_64
  ```

---

## 🔒 Security & Privacy Architecture

TikTok-Now is engineered with security and data privacy as core architectural tenets:

- **Direct First-Party Communication**: All HTTPS connections are established directly between your local device and official TikTok servers (`https://www.tiktok.com/`). TikTok-Now uses zero telemetry, zero analytics trackers, and zero project-operated proxy servers.
- **Native OS Storage Isolation**: Account session cookies, local storage, and session tokens remain strictly contained within your operating system's default webview sandbox:
  - **Windows**: `%LOCALAPPDATA%\com.tiktoknow.desktop\EBWebView`
  - **macOS**: `~/Library/Application Support/com.tiktoknow.desktop`
  - **Linux**: `~/.config/com.tiktoknow.desktop`
- **Isolated OAuth Login Popup Handling**: Third-party login modals (Google, Apple, TikTok QR) run inside an isolated secondary window instance that self-destructs upon login completion, preventing cookie leaks.
- **Cryptographic Verification**: Every release asset attached to [GitHub Releases](https://github.com/benedictusrey/TikTok-Now/releases) is published alongside a verified SHA-256 `checksums.txt` file.

---

## 🚀 Publishing with GitHub Desktop & GitHub Cloud CI/CD

Publishing **TikTok-Now** to GitHub is streamlined using **GitHub Desktop** and automated GitHub Actions.

### 1. Publishing via GitHub Desktop
1. Open **GitHub Desktop**.
2. Click **File** › **Add Local Repository...** and select `C:\Users\Benedictus\Desktop\TikTok-Now`.
3. Type a summary (e.g. `feat: initial release v1.0.0 of TikTok-Now`) in the bottom left commit panel.
4. Click **Commit to main**.
5. Click **Publish repository** (or **Push origin**) to send the code to your GitHub account (`benedictusrey/TikTok-Now`).

### 2. Automated Multi-Platform Releases via GitHub Actions
You do **not** need a physical Mac or Linux machine to build macOS and Linux binaries! The included workflow [`.github/workflows/release.yml`](.github/workflows/release.yml) handles cross-platform compilation automatically in GitHub Cloud.

- **Triggering a Cloud Build**:
  - **Option A (Tag Push)**: In Git / GitHub Desktop, create tag `v1.0.0` and push tags:
    ```bash
    git tag v1.0.0
    git push origin v1.0.0
    ```
  - **Option B (Manual Dispatch)**: Go to your repository on GitHub.com › **Actions** tab › select **Release TikTok-Now** › click **Run workflow**.

- **What GitHub Cloud Does Automatically**:
  1. **🪟 Windows Runner (`windows-latest`)**: Compiles `TikTok-Now_v1.0.0_windows-x86_64.exe`.
  2. **🍎 macOS Runner (`macos-latest`)**: Compiles both Intel (`x86_64`) and Apple Silicon (`aarch64`) targets, then joins them using `lipo` into a single `TikTok-Now_v1.0.0_macos-universal` binary.
  3. **🐧 Linux Runner (`ubuntu-22.04`)**: Installs GTK3/WebKit2GTK 4.1 headers and compiles `TikTok-Now_v1.0.0_linux-x86_64`.
  4. **🚀 GitHub Release Job**: Combines all three platform binaries + `checksums.txt` and publishes them to your repository's **Releases** page automatically.

---

## 📚 Documentation

- [Release Notes](RELEASE_NOTES.md)
- [Changelog](CHANGELOG.md)
- [GitHub Actions Release Workflow](.github/workflows/release.yml)

---

## 👤 Author and license

TikTok-Now is authored and maintained with ❤️ by **[@benedictusrey](https://github.com/benedictusrey)**.

Copyright © 2026 [@benedictusrey](https://github.com/benedictusrey).  
Released under the [MIT License](LICENSE).
