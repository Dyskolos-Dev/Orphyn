# Native release (2.0.0)

The release ships the native app (`native/`, Rust, no web engine) under the identity of the 1.5 Tauri
app: product name `Orphyn`, identifier `app.lumen.desktop`, same data folders, same keychain service
(`Orphyn`), same update key and same asset names. 1.5 installs update to it through `latest.json`.

## Version

`package.json` is the source. `native/crates/orphyn-native/Cargo.toml` carries the same number and
`scripts/native-package.mjs` refuses to build when they differ. Bump both, then tag `v<version>`.

## Building the installers

`npm run dist` (or `node scripts/native-package.mjs`) writes to `dist/native/`:

| OS | Files | Notes |
|---|---|---|
| Windows | `Orphyn_<v>_x64-setup.exe` + `.sig` | `build/windows/installer.nsi` + `hooks.nsh` (the dark custom pages, `npm run installer-art` first); `libmpv-2.dll` is installed next to `Orphyn.exe`. Needs NSIS (`makensis`, or `MAKENSIS`) and `ORPHYN_MPV_DIR` (folder with `libmpv-2.dll`). |
| macOS | `Orphyn_<v>_<arch>.dmg`, `Orphyn_<arch>.app.tar.gz` + `.sig` | `.app` built by hand, `libmpv.2.dylib` in `Contents/MacOS` and its dependencies in `Contents/Frameworks`, everything signed ad hoc, then `codesign --verify --deep --strict`. Needs `brew install mpv create-dmg`. |
| Linux | `Orphyn_<v>_amd64.AppImage` + `.sig`, `Orphyn_<v>_amd64.deb` + `.sig` | AppImage by linuxdeploy with `libmpv.so.2` bundled; `.deb` with `Depends: libmpv2, libasound2 | libasound2t64, ...`. Built on Ubuntu 24.04 (the first with `libmpv2`). |

The `.sig` files are minisign signatures made with `tauri signer sign` and the key in
`TAURI_SIGNING_PRIVATE_KEY` (CI secret) or `~/.orphyn/updater.key`. The installer template is Tauri's
NSIS template with Orphyn's pages: `scripts/native-package.mjs` fills it in itself (no Tauri bundler);
the files Tauri used to generate are in `build/windows/include/` (`utils.nsh`, `FileAssociation.nsh`),
`build/windows/plugins/` (`nsis_tauri_utils.dll`) and `build/windows/languages/`.

## Workflow

`.github/workflows/release.yml`: `prepare` (draft release) -> `build` on windows, macos-arm64, macos-x64
(an Intel runner) and linux -> `publish` (1.0.x bridge files `latest*.yml`, `latest.json` rebuilt from
the `.sig` files, asset check, then undraft).

Dry run: Actions > Release > Run workflow, any tag name (default `v2.0.0-dryrun`), "publish" off. The
release stays a draft, nothing is tagged, and the job summary lists the assets. A tag push publishes.

## Updating from 1.5

The 1.5 updater downloads `latest.json`, picks its platform entry, checks the minisign signature
against the public key in `tauri.conf.json` (unchanged) and runs the NSIS setup (Windows, passive),
replaces the `.app` from the `.app.tar.gz` (macOS) or the AppImage (Linux). The native app then reads
the same `latest.json` (`orphyn-core/src/updater`). The installer's `hooks.nsh` keeps removing 1.0.x
Electron installs.

## Migration of a 1.5 profile

`native/crates/orphyn-native/src/migration.rs`, run once at start (`migration::run_if_needed`, then
the marker `native-migration.json`): accounts and tokens need nothing (same file, same keychain
entries); `orphyn.*` entries of the old webview's localStorage (WebView2 LevelDB, WebKit SQLite) are
copied into `local-storage.json` without overwriting existing keys; `.window-state.json` becomes
`native-window-state.json`. The old files are only read.

## Linux build packages

`build-essential pkg-config libasound2-dev libxkbcommon-dev libwayland-dev libx11-dev libvulkan-dev
libssl-dev libdbus-1-dev libfontconfig1-dev libmpv-dev libmpv2`.
