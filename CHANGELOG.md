# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-11-13

### Added

- Auto-update from GitHub Releases: background download and "Restart to update" prompt on Windows and AppImage; "available" notice with a Download button on macOS and `.deb`. Settings > About shows the version, a "Check for updates" button with its status and an "Automatic updates" toggle.
- Refresh button in the sidebar, Cmd+R / Ctrl+R / F5 shortcut, and automatic refetch after the window was in the background for more than 5 minutes.
- New app icon.
- "Made with ❤️ by Dyskolos_" credit in Settings > About.
- macOS application menu (Settings, Edit, View, Window, Help), Linux desktop entry metadata and a keyring hint when sign-in cannot be encrypted.
- macOS zip target (needed by the updater) next to the DMG.

### Changed

- The app is renamed from Jellyra to Lumen: product name, app id (`app.lumen.desktop`), preload API (`window.lumen`), storage keys, DOM events and `LUMEN_*` environment variables.
- The release workflow now publishes the GitHub release directly, with the update feeds and blockmaps.

### Added

- Electron + React 19 + TypeScript application foundation, with electron-vite, ESLint, Prettier and Vitest.
- Project documentation: README in English and French, contributing guide, architecture overview.
- Continuous integration on Linux, Windows and macOS (typecheck, lint, tests, build).
- Release workflow that builds installers for each operating system on `v*` tags and attaches them to a draft GitHub Release.
- Issue templates (bug report, feature request) and a pull request template.
- MIT license (provisional).
