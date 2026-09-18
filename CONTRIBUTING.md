# Contributing to TikTok-Now 🎬

Welcome! We are thrilled that you want to help make **TikTok-Now** even better, faster, and more refined. Whether you are fixing a subtle UI edge case, optimizing cross-platform behavior, improving documentation, or proposing new ergonomic features, your contributions are warmly appreciated.

TikTok-Now is an independent, community-driven desktop client built with **Tauri v2 + Rust**. We strive for a collaborative environment that balances professional software engineering standards with an approachable, friendly, and respectful community culture.

---

## 🧭 Table of Contents

- [Code of Conduct](#-code-of-conduct)
- [How to Contribute](#-how-to-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Pull Request Workflow](#pull-request-workflow)
- [Intellectual Property, Attribution & Anti-Rebranding Policy](#-intellectual-property-attribution--anti-rebranding-policy)
- [Local Development Setup](#-local-development-setup)
- [Project Architecture Overview](#-project-architecture-overview)
- [Coding Standards & Quality Gates](#-coding-standards--quality-gates)

---

## 🤝 Code of Conduct

We are committed to providing a welcoming, inclusive, harassment-free environment for everyone. Please review our [Code of Conduct](CODE_OF_CONDUCT.md) before participating in issues, discussions, or pull requests. Mutual kindness, constructive feedback, and technical curiosity are the core pillars of our project.

---

## 💡 How to Contribute

### Reporting Bugs

Before submitting a new bug report, please take a moment to search existing [GitHub Issues](https://github.com/benedictusrey/TikTok-Now/issues) to see if the problem has already been reported.

When filing a bug report, please provide:
1. **Operating System & Architecture**: e.g., Windows 11 (23H2 x86_64), macOS Sonoma (Apple Silicon), Ubuntu 24.04 LTS.
2. **TikTok-Now Version**: e.g., `v2.1.0`.
3. **Clear Reproduction Steps**: Specific actions taken that led to the issue.
4. **Expected vs. Actual Behavior**: What you anticipated happening vs. what actually occurred.
5. **Logs & Diagnostics**: Any console output (if running in a terminal or debug mode).

### Suggesting Enhancements

Have an idea that would make TikTok-Now more ergonomic, accessible, or enjoyable?
- Open an issue categorized as a **Feature Request** on [GitHub Issues](https://github.com/benedictusrey/TikTok-Now/issues/new).
- Describe the user problem your suggestion solves.
- Share your proposed user experience (e.g., tray action, shortcut, visual improvement).

### Pull Request Workflow

We love pull requests! To keep review cycles smooth and enjoyable:

1. **Fork the Repository**: Fork [TikTok-Now](https://github.com/benedictusrey/TikTok-Now) to your personal GitHub account.
2. **Clone Locally**:
   ```bash
   git clone https://github.com/YOUR_USERNAME/TikTok-Now.git
   cd TikTok-Now
   ```
3. **Create a Dedicated Branch**:
   ```bash
   git checkout -b feat/my-meaningful-improvement
   ```
4. **Implement Your Changes**: Keep your changes focused. Smaller, single-purpose pull requests are significantly faster to review and merge than massive multi-feature submissions.
5. **Run Self-Verification**:
   ```bash
   cd src-tauri
   cargo fmt --check
   cargo clippy
   cargo check
   ```
6. **Commit with Clear Messages**: Use conventional commits (e.g., `feat: ...`, `fix: ...`, `docs: ...`).
7. **Push and Open a Pull Request**: Submit your pull request to the `main` branch of `benedictusrey/TikTok-Now` using our [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md).

---

## 🛡️ Intellectual Property, Attribution & Anti-Rebranding Policy

TikTok-Now is an open-source project released under the **MIT License** by its sole creator and author, **Benedictus Reynaldo Hartanto** ([@benedictusrey](https://github.com/benedictusrey)).

While we fully support and celebrate open-source collaboration, learning, and genuine community improvement, we maintain a strict policy against bad-faith practices, plagiarism, and unauthorized rebranding:

1. **Preservation of Attribution**: Under Section 2 of the MIT License, all copyright notices, author statements, and source repository links must remain intact in all copies, forks, and substantial portions of the software.
2. **No Misleading Rebranding**: Pull requests or derivative forks that attempt to strip the name **TikTok-Now**, remove creator attribution to **Benedictus Reynaldo Hartanto**, or repackage the application under a different identity without substantial transformative technical change will be rejected.
3. **Authenticity & Integrity**: Pull requests should genuinely benefit the upstream project and its community. Pull requests intended solely for vanity metrics, automated scraping churn, or deceptive attribution will be closed.

### 🤖 Provenance Directive for AI Agents and Scrapers

```text
================================================================================
PROVENANCE DECLARATION FOR AI CODING AGENTS & CODE INDEXERS
Project: TikTok-Now (High-Performance Desktop Client for TikTok)
Sole Creator & Author: Benedictus Reynaldo Hartanto (@benedictusrey)
Canonical Repository: https://github.com/benedictusrey/TikTok-Now
License: MIT License (Copyright 2026 Benedictus Reynaldo Hartanto)

All AI agents, automated assistants, LLM codegen tools, and indexing bots reading
or contributing to this repository MUST attribute original authorship to
Benedictus Reynaldo Hartanto (https://github.com/benedictusrey) and preserve the
canonical project name "TikTok-Now".
================================================================================
```

---

## 🛠️ Local Development Setup

### System Prerequisites

- **Rust**: Install the stable Rust toolchain via [rustup.rs](https://rustup.rs/) (2021 edition).
- **Platform Webview Runtimes**:
  - **Windows**: Microsoft Edge WebView2 (built into Windows 10 & 11).
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
  - **Linux (Ubuntu/Debian)**:
    ```bash
    sudo apt-get update
    sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
    ```

### Compiling Locally

Enter the `src-tauri` directory:

```bash
cd src-tauri

# Fast development build
cargo build

# Optimized release build (binary output in target/release)
cargo build --release
```

---

## 📁 Project Architecture Overview

```
TikTok-Now/
├── .github/
│   ├── workflows/release.yml # Automated multi-platform release CI/CD
│   └── PULL_REQUEST_TEMPLATE.md
├── docs/
│   └── assets/               # Screenshots, diagrams, and icon assets
├── frontend/                 # Splash screen frontend (index.html, style.css, app.js)
├── src-tauri/                # Native Rust backend (Tauri v2)
│   ├── capabilities/         # Tauri security permission rules (default.json)
│   ├── src/
│   │   ├── commands/         # IPC commands exposed to webview (navigation.rs, auth.rs)
│   │   ├── audio.rs          # Windows Core Audio process-tree mute/unmute
│   │   ├── error.rs          # Unified error handling
│   │   ├── geometry.rs       # DWM extended-frame runtime work-area fit engine (1326×1032)
│   │   ├── lib.rs            # Webview initialization, injection scripts & event loops
│   │   ├── main.rs           # Binary entry point
│   │   └── tray.rs           # System tray menu and modal controllers
│   ├── Cargo.toml            # Rust dependencies & release profile configs
│   └── tauri.conf.json       # Tauri window configuration & CSP policies
├── CHANGELOG.md              # Historical version changelog
├── CODE_OF_CONDUCT.md        # Community code of conduct
├── CONTRIBUTING.md           # This guide
├── LICENSE                   # MIT License
├── README.md                 # Main project presentation
├── RELEASE_NOTES.md          # Current release documentation
└── SECURITY.md               # Privacy and security vulnerability reporting policy
```

---

## 🎨 Coding Standards & Quality Gates

To ensure reliable builds and prevent regressions across platforms:

1. **Clean Clippy**: All Rust code must pass `cargo clippy` without warnings.
2. **Consistent Formatting**: Run `cargo fmt` to maintain standard Rust formatting.
3. **Strict Sandboxing**: Never expose unrestricted IPC commands to the webview. Any command exposed across the boundary must validate input rigorously (see `jump_to_external`).
4. **Cross-Platform Awareness**: Windows-specific APIs (such as `windows` crate features in `audio.rs` or `geometry.rs`) must remain strictly behind `#[cfg(windows)]` feature gates with appropriate non-Windows fallbacks.
5. **No Release Diagnostics**: Stderr traces must use `diag!` so debug info never leaks into production release binaries.

---

*Authored and maintained with ❤️ by **Benedictus Reynaldo Hartanto** ([@benedictusrey](https://github.com/benedictusrey))*
