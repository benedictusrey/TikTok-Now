# Contributing to TikTok-Now 🎬

Thank you for your interest in contributing to **TikTok-Now**! We welcome contributions from developers of all skill levels — whether you are fixing a bug, improving documentation, adding keybindings, or proposing new features.

---

## 🧭 Table of Contents

- [Code of Conduct](#-code-of-conduct)
- [How to Contribute](#-how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Features](#suggesting-features)
  - [Pull Requests](#pull-requests)
- [Local Development Setup](#-local-development-setup)
- [Project Structure Overview](#-project-structure-overview)
- [Coding Standards](#-coding-standards)

---

## 🤝 Code of Conduct

We aim to foster an inclusive, respectful, and welcoming community. Please treat all contributors with kindness, empathy, and constructive feedback.

---

## 💡 How to Contribute

### Reporting Bugs

Before creating a bug report, please check existing [GitHub Issues](https://github.com/benedictusrey/TikTok-Now/issues) to avoid duplicates.

When filing a bug report, please include:
- **Operating System** (e.g. Windows 11, macOS Sequoia, Ubuntu 24.04).
- **App Version** (e.g. `v1.0.0`).
- **Steps to Reproduce** the issue clearly.
- **Expected vs Actual Behavior**.
- Screenshots or console logs if applicable.

### Suggesting Features

We love new ideas! To suggest a feature:
1. Open a new issue under [Feature Request](https://github.com/benedictusrey/TikTok-Now/issues/new).
2. Explain **why** the feature is valuable and **how** it should work.

### Pull Requests

1. **Fork** the repository on GitHub.
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/TikTok-Now.git
   cd TikTok-Now
   ```
3. Create a feature branch:
   ```bash
   git checkout -b feature/my-cool-feature
   ```
4. Make your changes, testing locally (see [Local Setup](#-local-development-setup)).
5. Commit with clean, descriptive messages:
   ```bash
   git commit -m "feat: add double-click to toggle fullscreen"
   ```
6. Push to your branch and open a **Pull Request** on GitHub!

---

## 🛠️ Local Development Setup

### Prerequisites

- **Rust**: Installed via [rustup.rs](https://rustup.rs/) (edition 2021).
- **Node.js / npm** (optional): Only needed if modifying frontend tooling.
- **OS Dependencies**:
  - **Windows**: [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (pre-installed on Windows 10/11).
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
  - **Linux**: GTK3 and WebKit2GTK:
    ```bash
    sudo apt update && sudo apt install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
    ```

### Building & Running Locally

Navigate to the `src-tauri` directory:

```bash
cd src-tauri

# Run in debug mode
cargo build

# Run the release build
cargo build --release
```

The compiled binary will be in `src-tauri/target/release/TikTok-Now.exe` (or `TikTok-Now` on macOS/Linux).

---

## 📁 Project Structure Overview

```
TikTok-Now/
├── .github/workflows/       # Multi-platform CI/CD release workflow
├── docs/assets/             # Project graphics, icon, and showcase artwork
├── frontend/                # Application static assets & About modal markup
├── src-tauri/               # Native Rust application backend
│   ├── capabilities/        # Tauri v2 security capabilities configuration
│   ├── src/
│   │   ├── commands/        # Custom Tauri IPC command handlers
│   │   ├── db/              # Local SQLite database initialization
│   │   ├── error.rs         # Error types & handling
│   │   ├── lib.rs           # Core WebviewWindow builder, init script, navigation filters
│   │   ├── main.rs          # Application entrypoint
│   │   ├── state.rs         # AppState container
│   │   └── tray.rs          # System tray menu setup & About modal script injection
│   ├── Cargo.toml           # Rust package definitions
│   └── tauri.conf.json      # Tauri v2 configuration
├── CHANGELOG.md             # Version history
├── LICENSE                  # MIT License
└── README.md                # Main repository documentation
```

---

## 🎨 Coding Standards

- **Rust**: Format code using `cargo fmt` before submitting. Verify no clippy warnings via `cargo clippy`.
- **JavaScript**: Keep initialization scripts modular, vanilla (zero dependencies), and memory-efficient.
- **Security**: Maintain strict domain validation for all link navigation to ensure user privacy and security.

---

*Authored and maintained with ❤️ by [@benedictusrey](https://github.com/benedictusrey)*
