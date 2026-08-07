# Security & Privacy Policy 🔒

The security, privacy, and integrity of **TikTok-Now** and its users are our top priorities.

---

## 🛡️ Supported Versions

| Version | Supported | Security Maintenance |
| :--- | :---: | :--- |
| **`1.0.x`** | ✅ | Active security support & bug fixes |
| `< 1.0.0` | ❌ | Pre-release development versions |

---

## 🔐 Core Security & Privacy Guarantees

TikTok-Now is designed with strict security principles to safeguard your account and data:

1. **Direct First-Party Communication**:
   All web traffic connects directly to official TikTok servers (`https://www.tiktok.com/`). TikTok-Now uses **zero third-party tracking**, **zero analytics/telemetry**, and **zero middleman proxy servers**.

2. **Native OS Web Sandbox Isolation**:
   Account session credentials, cookies, and local storage remain strictly contained within your operating system's native WebEngine sandbox:
   - **Windows**: `%LOCALAPPDATA%\com.tiktoknow.desktop\EBWebView`
   - **macOS**: `~/Library/Application Support/com.tiktoknow.desktop`
   - **Linux**: `~/.config/com.tiktoknow.desktop`

3. **Isolated OAuth Popup Architecture**:
   Third-party sign-in options (Google, Apple, TikTok QR) run inside an isolated secondary window instance that self-destructs upon login completion, preventing cookie leaks or cross-site script access.

4. **Cryptographic Release Verification**:
   All release binaries published on [GitHub Releases](https://github.com/benedictusrey/TikTok-Now/releases) are cryptographically hashed and accompanied by an official `checksums.txt` file (SHA-256).

---

## 🚨 Reporting a Vulnerability

If you discover a potential security vulnerability in TikTok-Now, please report it responsibly:

- **Email**: Send security advisories directly to **[@benedictusrey](https://github.com/benedictusrey)** via [GitHub Profile](https://github.com/benedictusrey).
- **Response Time**: We aim to acknowledge receipt within **24 hours** and provide a patch timeline within **72 hours**.
- **Public Disclosure**: Please allow reasonable time for a fix to be published before making public disclosures.

Thank you for helping keep TikTok-Now secure!

---

*Authored and maintained with ❤️ by [@benedictusrey](https://github.com/benedictusrey)*
