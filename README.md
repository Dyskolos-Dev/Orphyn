# Jellyra

**Jellyra** is a desktop client for [Jellyfin](https://jellyfin.org), the free and open-source media server. It plays your music and your movies and series on Windows, macOS and Linux, with an interface inspired by the Apple TV and Apple Music apps.

> **Status: in development.** Jellyra is not released yet. The features below are the planned v1 scope and are being built now.

Language: **English** · [Français](README.fr.md)

## Features

Planned for v1 (all **in progress**):

- **Music**: browse albums, artists, playlists and songs; a full music player.
- **Movies and series**: browse libraries, item details and a video player.
- **Search** across your libraries.
- **Multiple servers**: connect to several Jellyfin servers and switch between them.
- **Bilingual interface**: French and English, following the system language by default.
- **Casting**: Google Cast, UPnP/DLNA (including WiiM devices), and AirPlay on a best-effort basis.
- **Apple TV / Apple Music look**: a consistent, polished design system.

## Requirements

- [Node.js](https://nodejs.org) **22 or newer**
- npm (ships with Node.js)
- A Jellyfin server **10.9 or newer**, reachable from your computer

## Development setup

```bash
git clone <repository-url> jellyra
cd jellyra
npm install
npm run dev
```

`npm run dev` starts the app with electron-vite: the renderer has hot module replacement and the main process restarts on change.

## Scripts

| Script                 | What it does                                                         |
| ---------------------- | -------------------------------------------------------------------- |
| `npm run dev`          | Run the app in development mode (electron-vite dev)                  |
| `npm run build`        | Build the main, preload and renderer bundles into `out/`             |
| `npm run preview`      | Preview the production build (electron-vite preview)                 |
| `npm run typecheck`    | TypeScript check for the main/preload/shared code, then the renderer |
| `npm run lint`         | Lint the whole project with ESLint                                   |
| `npm run format`       | Format the project with Prettier (writes files)                      |
| `npm run format:check` | Check formatting with Prettier without writing files                 |
| `npm test`             | Run the unit and component tests once with Vitest (jsdom)            |
| `npm run check`        | Typecheck, lint, test and build: run this before opening a PR        |
| `npm run dist`         | Build, then package installers for the current operating system      |

## Building installers

`npm run dist` builds the app and packages it with electron-builder into the `release/` folder. Packaging only produces installers for the operating system you run it on:

| Run on  | Installers produced                                       |
| ------- | --------------------------------------------------------- |
| Windows | NSIS installer (`.exe`)                                   |
| macOS   | DMG disk image, for Intel (x64) and Apple Silicon (arm64) |
| Linux   | AppImage and Debian package (`.deb`)                      |

```bash
npm run dist
```

The installers are not code-signed yet. Signing for Windows and macOS (certificates, notarization) is a TODO; see the release workflow in `.github/workflows/release.yml`. Official builds for all three systems are produced by the GitHub Actions release workflow when a `v*` tag is pushed.

## Project layout

Jellyra is an Electron + React 19 + TypeScript (strict) application. The source is split into a main process (`src/main`), a preload bridge (`src/preload`), shared IPC contracts (`src/shared`) and the renderer (`src/renderer/src`), organised by feature. Read these documents before making changes:

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): folders, composition points, IPC and i18n conventions.
- [docs/DESIGN.md](docs/DESIGN.md): design system and visual guidelines.
- [docs/JELLYFIN.md](docs/JELLYFIN.md): how Jellyra talks to Jellyfin servers.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for branch naming, commit messages, the i18n rule and the checks to run before a pull request.

## License

Jellyra is released under the MIT License (see [LICENSE](LICENSE)). The license is not final yet and may change before the first release.

## Disclaimer

Jellyra is an independent project. It is **not affiliated with, endorsed by or sponsored by** the Jellyfin project or Apple Inc. "Jellyfin" is a trademark of the Jellyfin project; "Apple TV", "Apple Music", "AirPlay" and "Apple" are trademarks of Apple Inc. They are used only to describe compatibility and design inspiration.
