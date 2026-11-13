# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.4] - 2026-11-13

### Changed

- Windows installer and uninstaller fully custom (nsDialogs): dark navy pages with the logo, flat buttons, a thin progress bar, a Welcome page (Install / Options), an Options page (folder, desktop and start-menu shortcuts, launch at the end) and a Finish page, in French or English following the system language. Silent updates () are unchanged.
- Now Playing: the Now Playing / Lyrics / Up Next control is centred on the window, title and artist are centred, and artwork, title, progress, transport and volume form one column centred at any window size and display scale.
- Mini player rebuilt as a three-part flex row (transport, capsule, actions) that cannot overlap, with container queries: volume slider hidden below 1000 px, shuffle and repeat below 820 px. Times show only on hover.

### Added

- End-to-end checks for mini player overlap (800 to 2600 px at 1x, 1.25x, 1.5x) and for Now Playing and onboarding centring.

## [1.0.3] - 2026-11-13

### Added

- "What's new" sheet shown once after an update (every skipped version, newest first), reopenable from Settings > About. Release notes live in `src/renderer/src/features/whatsnew/notes/<version>.json`.
- Styled installers: Windows NSIS wizard with a dark navy sidebar, header and icons, a macOS disk image background with an arrow to Applications. The Windows installer is now named `Lumen-Setup-<version>.exe`.
- Mini player "Not Playing" state: the bar stays visible with the dimmed logo while the queue is empty.

### Changed

- Mini player rebuilt after Apple Music: transport on the left, a 52 px "LCD" capsule with the artwork, title, artist and an inset progress line (times on hover), volume and actions on the right. It no longer overflows between 1000 and 2560 px.
- First-launch onboarding keeps its logo, title and fields on one centred axis, including during step transitions.

### Fixed

- Segmented controls (Now Playing tabs): equal-width segments, centred labels, a thumb that exactly covers the selected one, and no focus ring after a mouse click (keyboard focus still shows it).

## [1.0.2] - 2026-11-13

### Changed

- Release rebuilt for Windows, macOS and Linux.

## [1.0.1] - 2026-11-13

### Changed

- New README with the logo, badges and an install guide.
- First release built and published by GitHub Actions for Windows, macOS and Linux.

## [1.0.0] - 2026-11-13

### Added

- Auto-update from GitHub Releases: background download and "Restart to update" prompt on Windows and AppImage; "available" notice with a Download button on macOS and `.deb`. Settings > About shows the version, a "Check for updates" button with its status and an "Automatic updates" toggle.
- Refresh button in the sidebar, Cmd+R / Ctrl+R / F5 shortcut, and automatic refetch after the window was in the background for more than 5 minutes.
- The official Lumen logo as app icon and in the app (onboarding, Settings > About, update notice).
- Discord Rich Presence: music shows as "Listening to Lumen", videos as "Watching Lumen", with title, progress bar and artwork (only when the server is reachable over public HTTPS, otherwise the Lumen logo). Settings > Discord has the on/off, artwork and status controls. Setup notes in `docs/DISCORD.md`.
- Animated first-launch onboarding: aurora background, spring logo, word-by-word welcome title and a success checkmark after signing in. Honours reduced motion.
- "Made with ❤️ by Dyskolos_" credit in Settings > About.
- macOS application menu (Settings, Edit, View, Window, Help), Linux desktop entry metadata and a keyring hint when sign-in cannot be encrypted.
- macOS zip target (needed by the updater) next to the DMG.

### Fixed

- A restored, not yet loaded track showed "--:--" as its duration; it now uses the length from the track metadata.

### Changed

- Redesigned mini player: taller translucent bar, evenly spaced transport, an "LCD" capsule with artwork, title, artist and a thin progress line (times on hover), and volume / cast / lyrics / queue on the right. The volume slider, then shuffle and repeat, hide on narrow windows.
- The video home no longer shows a back button, and an album or playlist title only appears in the toolbar once its large title has scrolled away.
- The app is renamed from Jellyra to Lumen: product name, app id (`app.lumen.desktop`), preload API (`window.lumen`), storage keys, DOM events and `LUMEN_*` environment variables.
- The release workflow now publishes the GitHub release directly, with the update feeds and blockmaps.

### Added

- Electron + React 19 + TypeScript application foundation, with electron-vite, ESLint, Prettier and Vitest.
- Project documentation: README in English and French, contributing guide, architecture overview.
- Continuous integration on Linux, Windows and macOS (typecheck, lint, tests, build).
- Release workflow that builds installers for each operating system on `v*` tags and attaches them to a draft GitHub Release.
- Issue templates (bug report, feature request) and a pull request template.
- MIT license (provisional).
