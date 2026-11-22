# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.0.4] - 2026-11-22

### Added

- Watchlist for films and series, synced through the server, with a new-episode reminder.

### Fixed

- End-of-episode Play now / Cancel buttons did nothing.
- macOS: the full-screen player header no longer sits under the window buttons.
- Artists: multi-artist entities are split; no letters under cards without a picture.

## [2.0.3] - 2026-11-19

### Changed

- Full-screen player: the large album cover with a deep blur is back (computed once per album).
- Film and series banners no longer show the overview (episodes still do); the Synopsis section keeps it.
- The app icon shows in the Start menu, taskbar and Alt+Tab; the React/Tauri code is gone from the repository.

## [2.0.2] - 2026-11-19

### Changed

- Film and series banners (video home and item pages) no longer show the overview, which stays in the Synopsis section.

## [2.0.1] - 2026-11-19

### Fixed

- Windows: no command window opens beside the app any more (closing it closed Orphyn).
- What's new did not show after updating to 2.0.

## [2.0.0] - Unreleased

### Changed

- Orphyn is now 100 % native: the app is written in Rust and draws its own interface, with no web engine. It starts faster and uses far less memory.
- Video is played by libmpv, bundled in the Windows installer, the macOS app and the Linux AppImage (the `.deb` depends on `libmpv2`).
- Installers and updates: same file names, same update signing key. Orphyn 1.5 updates to this version by itself.

### Added

- First start after an update from 1.5: accounts (same keychain entries), settings and window size and position are imported automatically; the 1.5 files are left untouched.

## [1.5.0] - 2026-11-18

### Added

- Rich artist page: popular songs, discography by year and release type, biography and similar artists.
- Online artist images (Deezer, TheAudioDB) for artists without a server image, with one switch in Settings > Music for all online artist info.
- Top songs of the year: playable from Your year and the mixes, savable as a Jellyfin playlist.

### Changed

- Your year: series show their poster, every entry opens its page, the top artist block is better spaced.

## [1.4.1] - 2026-11-18

### Fixed

- macOS reported the app as damaged and refused to open it: the bundle is now signed ad hoc, and the release checks the signature.

## [1.4.0] - 2026-11-17

### Added

- Year in review: an animated page with your top artists, songs, albums, genres, films and series, per year.
- Online lyrics from LRCLIB when the server has none (synced when available), can be turned off in Settings > Music.
- Artist grouping: "feat.", "&" and spelling variants are merged into one artist, can be turned off in Settings > Music.
- Audio output device choice for music and video in Settings (where the platform supports it).

### Fixed

- The "…" menu of the full-screen Now Playing opened behind it.
- The full-screen button of the music Picture-in-Picture window did nothing.

## [1.3.0] - 2026-11-17

### Added

- Watch and listen together with Jellyfin SyncPlay (rooms, synchronised playback, members and their state).
- Smart mixes: For You, Discovery, On Repeat, Rediscover and genre mixes, refreshed daily.
- Radio from a song, an album or an artist, with an endless queue.
- Crossfade between songs, adjustable (1-12 s) or off, optionally not within an album.

## [1.2.1] - 2026-11-17

### Changed

- Home customisation moved to Settings > Home; the home screens no longer show an Edit button.

### Fixed

- Music Picture-in-Picture window is sharp at every size and display scale.

## [1.2.0] - 2026-11-17

### Added

- Appearance personalisation: accent (custom or from the artwork), transparency, corners, density, text size, sidebar mode, page backdrop, motion, card shape; live preview.
- Skip Intro / Recap / Credits and Next Episode, with optional automatic intro skipping (Jellyfin media segments).
- Command palette (Cmd/Ctrl+K).
- Customisable video and music home screens.

### Fixed

- Trailers: server trailers play in the player, YouTube trailers in a built-in window.
- Music Picture-in-Picture window: drag from anywhere, resize from every edge, edge snapping.
- The five interface styles refined on every screen, light and dark.

## [1.1.0] - 2026-11-14

### Changed

- Lumen is now **Orphyn**: new name, window titles, installer, artifact names, website (orphyn.dyskolos.fr) and repository (Dyskolos-Dev/Orphyn). The window bridge is `window.orphyn` and the end-to-end variables are `ORPHYN_*`.
- Rebuilt on a native core (Tauri v2, Rust) in place of Electron: a much smaller download and install, faster start and less memory. The interface is the same React app, rendered by the system webview (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux); the Jellyfin device profile follows what each webview can play.
- Access tokens are kept in the OS keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service); `accounts.json` no longer holds them.
- Casting (Google Cast, UPnP/DLNA/WiiM, AirPlay), Discord Rich Presence, the OS media controls (Windows SMTC, macOS Now Playing, Linux MPRIS) and the updater run in the native core.
- Updates install themselves on macOS too (Windows, macOS and Linux AppImage), verified with the release signing key.
- Installer files are now `Orphyn_<version>_x64-setup.exe`, `Orphyn_<version>_aarch64.dmg` / `_x64.dmg`, `Orphyn_<version>_amd64.AppImage` and `.deb`. The Windows installer keeps its dark custom pages (Welcome, Options, progress, Finish) and its uninstaller.

### Added

- Picture-in-Picture for music (mini player window) and video.
- Playlists (create, add songs and albums, remove, delete) and favourites, with hearts in the players.
- Five interface styles: Classic, Glass, Cinema, Minimal and Compact, in Settings > General.
- End-to-end suite on the native app (Playwright over the WebView2 DevTools protocol), with new scenarios for playlists, favourites, interface styles and Picture-in-Picture.

### Removed

- The Electron shell, its build (electron-vite, electron-builder) and the native application menu (its shortcuts remain: Ctrl/Cmd+R refreshes, Ctrl/Cmd+, opens Settings).

### Migration

- Updating from 1.0.x happens in the app as usual: the release also carries the `latest.yml` / `latest-linux.yml` / `latest-mac.yml` files the 1.0.x updater reads. On Windows the new installer removes the previous install and its shortcuts; the Linux AppImage is replaced in place; on macOS the update opens the download page once.
- Saved servers, users, names, the active server and the device id are imported from the previous version; each server asks to sign in again once (its token could not be carried over). Preferences (theme, accent, language, player settings) and the window size start from their defaults.

## [1.0.5] - 2026-11-13

### Fixed

- Windows release build: the custom installer art is generated in CI before packaging.

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

- Mini player rebuilt: transport on the left, a 52 px "LCD" capsule with the artwork, title, artist and an inset progress line (times on hover), volume and actions on the right. It no longer overflows between 1000 and 2560 px.
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
