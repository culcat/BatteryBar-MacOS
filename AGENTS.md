# AGENTS.md — AI Agent Architecture & Development Guide for BatteryBar

> **Scope:** Repository-wide rules and architectural context for AI coding agents working on **BatteryBar** (`battery-bar`). This file takes precedence over global defaults.

---

## 1. Communication & Workflow Rules

- **Language:** Communicate with the user in **Russian**; write all source code, code comments, docstrings, commit messages, and PR descriptions in **English**.
- **Commits:** Use **Conventional Commits** without scope on every major feature or fix:
  - `feat: description`
  - `fix: description`
  - `refactor: description`
  - `chore: description`
  - `docs: description`
- **Documentation Sync:** Keep [`DOCUMENTATION.md`](./DOCUMENTATION.md) and [`README.md`](./README.md) synchronized whenever CLI behavior, UI layout, telemetry fields, or build scripts change.

---

## 2. Project Overview & Core Philosophy

**BatteryBar** is an ultra-lightweight, native macOS Menu Bar (`NSStatusBar`) application written in pure Rust using direct Apple AppKit / Cocoa bindings (`objc2`, `objc2-app-kit`, `objc2-foundation`).

### Non-Negotiable Constraints
1. **Zero WebView / Electron / Tauri Bloat:** Never introduce web-based UI frameworks. All UI must be built directly with native AppKit views (`NSStatusBarButton`, `NSMenu`, `NSMenuItem`, `NSView`, `NSTextField`, `NSBezierPath`, `NSImage`).
2. **Near-Zero Battery & CPU Impact (`Energy Impact = 0.0`):**
   - Never use busy loops or high-frequency timers (`< 10s`).
   - Default polling interval is `30s` (`UPDATE_INTERVAL_SECS` in `src/main.rs`).
   - All subprocess calls (`pmset`, `ioreg`, `curl`) MUST execute off the main thread (or with strict timeouts) so the Cocoa RunLoop never stutters.
   - When calling `ioreg`, ALWAYS pass `-d 1` (`ioreg -rc AppleSmartBattery -d 1`) to avoid deep IORegistry traversal and sandbox violations.
3. **Accessory Activation Policy:** The app runs with `NSApplicationActivationPolicy::Accessory` and `LSUIElement = true` in `assets/Info.plist` — it must never show a Dock icon or steal window focus.

---

## 3. Architecture & Module Map

```text
battery-bar/
├── Cargo.toml                  # Rust crate manifest & AppKit/Foundation feature flags
├── AGENTS.md                   # This agent guide & project rules
├── DOCUMENTATION.md            # Technical & architectural documentation (Russian)
├── README.md                   # Repository landing page (English default)
├── README.ru.md                # Russian translation of README
├── README.fr.md                # French translation of README
├── README.es.md                # Spanish translation of README
├── README.zh.md                # Simplified Chinese translation of README
├── .github/workflows/
│   └── build-dmg.yml           # CI/CD: Universal 2 binary (lipo) + .dmg + GitHub Releases
├── assets/
│   ├── Info.plist              # macOS bundle metadata (LSUIElement=true, AppIcon)
│   ├── AppIcon.icns            # Native multi-resolution macOS icon bundle
│   ├── logo.png / logo.svg     # 1024x1024 master icon & vector source
│   └── screenshot.jpg          # README preview image
├── scripts/
│   ├── build_dmg.sh            # Packages BatteryBar.app and creates .dmg via create-dmg/hdiutil
│   └── generate_icon.py        # Supersampled Pillow generator for assets/logo.png
└── src/
    ├── main.rs                 # AppKit lifecycle, NSMenu wiring, MenuActionHandler, threads
    ├── i18n.rs                 # System locale detection (AppleLanguages/LANG) & translations (en, ru, fr, es, zh)
    ├── ui.rs                   # Native Sonoma/Sequoia UI: HeaderCardView, BatteryProgressBarView, SF Symbols
    ├── battery.rs              # pmset & ioreg parsers, DisplayMode formatting, unit tests
    └── updater.rs              # GitHub Releases semver update checker & browser launcher
```

### Module Responsibilities

| Module | Key Types & Functions | Responsibility |
| :--- | :--- | :--- |
| [`src/main.rs`](./src/main.rs) | `AppState`, `MenuRefs`, `MenuActionHandler`, `update_battery_display`, `update_static_menu_labels` | Initializes `NSApplication`, builds the `NSMenu` hierarchy, handles Objective-C action selectors (`refreshBattery:`, `checkUpdates:`, `selectDisplayMode:`, `selectLanguage:`, `openBatterySettings:`), and dispatches UI updates onto the main thread via `run_on_main`. |
| [`src/i18n.rs`](./src/i18n.rs) | `Language`, `Language::detect_system`, `Language::parse_apple_languages`, `Language::from_locale_str` | Detects the user's macOS language via `defaults read -g AppleLanguages` (with `LC_ALL`/`LC_MESSAGES`/`LANG` fallback, defaulting to `Language::En`) and provides localized strings for English, Russian, French, Spanish, and Simplified Chinese. |
| [`src/ui.rs`](./src/ui.rs) | `HeaderCardView`, `BatteryProgressBarView`, `load_sf_symbol` | Implements the macOS Control Center–style top card (`276×96 pt`) and custom capsule progress bar (`drawRect:` with `NSBezierPath`), plus adaptive monochrome SF Symbols (`setTemplate(true)`). |
| [`src/battery.rs`](./src/battery.rs) | `BatteryInfo`, `BatteryState`, `DisplayMode`, `fetch_battery_info`, `parse_pmset_output`, `enrich_with_ioreg` | Runs `pmset -g batt` and `ioreg -rc AppleSmartBattery -d 1`, parses percentage, state, remaining time, cycle count, health (`NominalChargeCapacity / DesignCapacity`), voltage, and wattage. |
| [`src/updater.rs`](./src/updater.rs) | `UpdateInfo`, `check_for_updates`, `parse_github_release`, `is_newer_version` | Queries `https://api.github.com/repos/culcat/BatteryBar-MacOS/releases/latest` via `curl --max-time 5`, compares semantic versions, and opens the release URL via `open`. |

---

## 4. Thread-Safety & AppKit (`objc2`) Guidelines

AppKit UI objects (`NSStatusItem`, `NSMenu`, `NSMenuItem`, `NSView`, `NSTextField`) are **`!Send` and `!Sync`** and must **only** be accessed on the main thread.

1. **`MainThreadMarker` & `MainThreadBound`:**
   - Store main-thread-only references inside `objc2_foundation::MainThreadBound<MenuRefs>`.
   - Obtain `MainThreadMarker` in `main()` via `MainThreadMarker::new().expect(...)` or inside background callbacks via `objc2_foundation::run_on_main(|mtm| { ... })`.
2. **Dynamic Colors & Appearance (Dark / Light Mode):**
   - Never hardcode RGB colors for text or track backgrounds.
   - Always use semantic AppKit colors:
     - `NSColor::labelColor()` for primary text
     - `NSColor::secondaryLabelColor()` for subtitles
     - `NSColor::quaternaryLabelColor()` for capsule track backgrounds
     - `NSColor::systemGreenColor()`, `systemOrangeColor()`, `systemRedColor()` for battery fill states
   - Resolve `NSColor::setFill()` inside `drawRect:` so macOS automatically re-evaluates the active appearance when the user switches between Light and Dark Mode.
3. **SF Symbols Compatibility:**
   - Always provide a fallback symbol name in `load_sf_symbol(primary, fallback)` (e.g., `"battery.100percent"` with fallback `"battery.100"`) so the app renders icons on both macOS 13–15+ and macOS 11–12.
   - Always call `img.setTemplate(true)` on menu bar and menu item icons.
4. **Adding New `objc2-app-kit` APIs:**
   - `objc2-app-kit` gates every class and category behind Cargo feature flags in [`Cargo.toml`](./Cargo.toml).
   - Note that inheriting classes often require parent features enabled (e.g., `NSApplication` and `NSStatusItem::button` require `"NSResponder"`, `"NSView"`, `"NSControl"`, `"NSButton"`).
   - When building in an offline/sandboxed environment, use `--offline` with Cargo (`cargo check --offline`, `cargo test --offline`) to use cached crates in `~/.cargo/registry/cache`.

---

## 5. Rust Coding & Safety Standards (`rust-skills`)

Route Rust design and safety questions through the installed `rust-skills` plugin (`.agents/skills/`):
- **Ownership & Lifetimes:** `m01-ownership`
- **Smart Pointers & RAII (`Retained<T>`, `Arc<T>`):** `m02-resource`
- **Interior Mutability (`Cell`, `Mutex`, `MainThreadBound`):** `m03-mutability`
- **Error Handling:** `m06-error-handling` — never `unwrap()` on subprocess I/O or regex captures in production paths; return `Result<T, String>` and degrade gracefully in the UI.
- **Concurrency:** `m07-concurrency` — never hold a `MutexGuard` across `run_on_main` closures (which execute synchronously on the main queue and can deadlock if the main thread also locks the same mutex). Clone or extract the data before calling `run_on_main`.
- **Unsafe Code (`unsafe-checker`):**
  - Keep `unsafe` blocks as small and localized as possible around FFI / Objective-C message sends.
  - Ensure all Objective-C selectors (`sel!(...)`) match the exact method signature declared in `declare_class!` or Apple's headers (including trailing colons for 1-argument actions like `refreshBattery:`).

---

## 6. Build, Test & Packaging Commands

```bash
# 1. Type-check (offline-friendly)
cargo check --offline

# 2. Run all unit tests (battery parser, ioreg enrichment, semver updater)
cargo test --offline

# 3. Build optimized release binary
cargo build --release --offline

# 4. Package macOS .app bundle and .dmg installer locally
./scripts/build_dmg.sh
```

### Version Bump Checklist
When releasing a new version `vX.Y.Z`:
1. Update `version = "X.Y.Z"` in [`Cargo.toml`](./Cargo.toml).
2. Update `CFBundleShortVersionString` in [`assets/Info.plist`](./assets/Info.plist) (`scripts/build_dmg.sh` reads the version from `Cargo.toml` automatically).
3. Run `cargo test --offline`.
4. Commit with `chore: bump version to X.Y.Z` and tag `git tag -a vX.Y.Z -m "Release vX.Y.Z"`.
