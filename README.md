<div align="center">

<img src="build/icon.png" alt="Orphyn" width="160" height="160" />

# Orphyn

**A beautiful desktop client for Jellyfin: your movies, shows and music, at its best.**

[![Latest release](https://img.shields.io/github/v/release/Dyskolos-Dev/Orphyn?style=for-the-badge&label=version&color=8b8cf8)](https://github.com/Dyskolos-Dev/Orphyn/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-5ec8d8?style=for-the-badge)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-b9a7e8?style=for-the-badge)](https://github.com/Dyskolos-Dev/Orphyn/releases/latest)
[![Jellyfin](https://img.shields.io/badge/Jellyfin-10.9%2B-00a4dc?style=for-the-badge&logo=jellyfin&logoColor=white)](https://jellyfin.org)

[**Download**](https://github.com/Dyskolos-Dev/Orphyn/releases/latest) · [Website](https://orphyn.dyskolos.fr) · [Français](README.fr.md)

</div>

---

## ✨ Features

| | |
|---|---|
| 🎬 **Movies & TV shows** | Cinematic home with a hero banner, Continue Watching, Next Up and Recently Added; rich detail pages, seasons and episodes. |
| ▶️ **Built-in player** | Direct play or server transcoding (HLS), resume, audio and subtitle tracks, Up Next with auto-play, keyboard shortcuts. |
| 🎵 **Music** | Refined library: albums, artists, playlists, queue, shuffle and repeat, full-screen Now Playing. |
| 🎤 **Synced lyrics** | Time-synced lyrics that scroll with the song; click a line to jump to it. |
| 📡 **Casting** | Google Cast (music and video), UPnP/DLNA renderers including WiiM, AirPlay on Apple TV (best effort). |
| 🗂️ **Multiple servers** | Add a server with its address, your account and a name; switch in one click. Only a token is kept, in the system keychain, never your password. |
| 🔎 **Global search** | Movies, shows, episodes, artists, albums and songs in one place. |
| 🌗 **Light & dark** | Follows your system, with an accent color of your choice. FR and EN. |
| 🎮 **Discord Rich Presence** | "Listening to" / "Watching" with artwork, toggle in Settings. |
| ⌨️ **Native feel** | Media keys and OS media controls, ⌘/Ctrl+, for Settings, ⌘/Ctrl+R to refresh the library. |
| 🪟 **Picture-in-Picture** | Keep the music mini player or a video in a small window above everything else. |
| 🔄 **Automatic updates** | New versions download and install themselves on Windows, macOS and Linux (AppImage). |

## 📥 Install

Download the installer for your system from the [latest release](https://github.com/Dyskolos-Dev/Orphyn/releases/latest):

| System | File |
|---|---|
| Windows 10/11 | `Orphyn_x.y.z_x64-setup.exe` |
| macOS Apple silicon (M1 and later) | `Orphyn_x.y.z_aarch64.dmg` |
| macOS Intel | `Orphyn_x.y.z_x64.dmg` |
| Linux | `Orphyn_x.y.z_amd64.AppImage` or `Orphyn_x.y.z_amd64.deb` |

> [!NOTE]
> The builds are not code-signed yet. On Windows, click **More info → Run anyway** in SmartScreen.
>
> On macOS, drag Orphyn to Applications and open it. If macOS says it cannot verify the app, open **System Settings → Privacy & Security** and click **Open Anyway** (once). If it says the app is **damaged**, run this in Terminal, then open it again:
>
> ```bash
> xattr -cr /Applications/Orphyn.app
> ```
>
> Updates then install from inside the app, with no further step.

Orphyn 2 is a fully native app (Rust, no web engine). Video is played by [libmpv](https://mpv.io), which the installers carry: nothing to install on Windows, macOS or with the AppImage. The `.deb` depends on the `libmpv2` package (Ubuntu 24.04 / Debian 13 and later), which `apt` installs for you. If you already use Orphyn 1.5, it updates itself to the native build and keeps your accounts and settings.

You need a [Jellyfin](https://jellyfin.org) server, version 10.9 or later.

## 🛠️ Development

Requires Node.js 22+, npm and Rust stable (1.85+). The native app lives in [`native/`](native/README.md) (Dioxus Native / Blitz, wgpu, libmpv); the React app is only its design reference. On Linux you need the packages listed in [docs/NATIVE-RELEASE.md](docs/NATIVE-RELEASE.md), and libmpv at runtime (`ORPHYN_MPV_DIR` on Windows).

```bash
npm install
npm run dev
```

| Script | What it does |
|---|---|
| `npm run dev` | Starts the app with hot reload |
| `npm run check` | Typecheck, lint, unit tests, renderer build, and the Rust format, lint and tests |
| `npm run e2e` | End-to-end tests against a real server (see [docs/QA.md](docs/QA.md)) |
| `cd native && cargo run -p orphyn-native` | Runs the native app |
| `npm run dist` | Builds the native installers for the current system into `dist/native/` (see [docs/NATIVE-RELEASE.md](docs/NATIVE-RELEASE.md)) |

Releases are built by GitHub Actions when a `v*` tag is pushed (see [CONTRIBUTING.md](CONTRIBUTING.md)).

Learn more in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/TAURI.md](docs/TAURI.md), [docs/DESIGN.md](docs/DESIGN.md), [docs/JELLYFIN.md](docs/JELLYFIN.md), [docs/CASTING.md](docs/CASTING.md) and [docs/DISCORD.md](docs/DISCORD.md).

## 📄 License

Orphyn is released under the [MIT License](LICENSE).

Orphyn is an independent project. It is not affiliated with or endorsed by Jellyfin.

<div align="center">

Made with ❤️ by [Dyskolos_](https://dyskolos.fr)

</div>
