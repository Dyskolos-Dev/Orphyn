# Orphyn on Tauri v2

Orphyn is moving from Electron to Tauri v2: a Rust core (`src-tauri/`) and the same React
renderer. During the migration both shells live side by side: `npm run dev` / `npm run dist`
still build the Electron app, and `npm run tauri:dev` / `npm run tauri:build` build the Tauri one.
The renderer does not know which shell it runs in: both install the same `window.orphyn`.

## How to run

| Script                   | What it does                                                                 |
| ------------------------ | ---------------------------------------------------------------------------- |
| `npm run tauri:dev`      | Vite dev server for the renderer + `cargo run` of the shell, with HMR         |
| `npm run tauri:build`    | Release build + installers (`dist/tauri-target/release/bundle/`)            |
| `npm run dev:renderer`   | Renderer alone with Vite (`vite.renderer.config.ts`), port `ORPHYN_DEV_PORT` |
| `npm run build:renderer` | Renderer into `dist/renderer/` (the Tauri `frontendDist`)                    |
| `npm run check:rust`     | Renderer build, `cargo fmt --check`, `clippy -D warnings`, `cargo test`      |

`npm run check` ends with `check:rust`. Requirements: Rust stable (1.85+), and on Linux the Tauri
system packages (`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
libsoup-3.0-dev libjavascriptcoregtk-4.1-dev patchelf`) plus `libdbus-1-dev` (keyring, souvlaki)
and the GStreamer plugins (`gstreamer1.0-plugins-good gstreamer1.0-libav`) for playback.

- The dev server port defaults to 1420 (`devUrl` in `src-tauri/tauri.conf.json`). To use another
  one: `ORPHYN_DEV_PORT=21000 npx tauri dev --config '{"build":{"devUrl":"http://localhost:21000"}}'`.
- Debug builds on Windows accept `ORPHYN_CDP_PORT=<port>`: WebView2 then opens the DevTools
  protocol, so Playwright can attach with `chromium.connectOverCDP('http://127.0.0.1:<port>')`.
  (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` is ignored because wry passes its own arguments.)
- The renderer is built into `dist/renderer/` rather than a new top-level folder, because `dist/`
  is already ignored by git and ESLint.
- `tauri:build` signs the updater artifacts (`createUpdaterArtifacts`): set
  `TAURI_SIGNING_PRIVATE_KEY` (the key or its path) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
  (empty for the current key) before running it. The public key is in `tauri.conf.json`; the
  private key is never committed.

## Layout

```
src-tauri/
  .cargo/config.toml   cargo target dir = dist/tauri-target (cargo emits .js files ESLint must not see)
  tauri.conf.json      app "Orphyn", identifier app.lumen.desktop, bundles, updater endpoint + pubkey
  capabilities/        default.json: core events, window dragging, opener
  icons/               generated with `npx tauri icon build/icon.png -o src-tauri/icons`
  src/
    main.rs            calls orphyn_lib::run()
    lib.rs             plugins, setup (each module's init), every command registered
    error.rs           Error (serializes as a string), Result
    window.rs          main window, material, window commands and events
    accounts/          store.rs (model, logic), keychain.rs, migrate.rs (Electron import)
    updater/           state.rs (state machine), mod.rs (plugin-updater, timers, commands)
    cast/ discord/ media_controls/ pip/   stubs: commands return "<command>: not implemented"
src/renderer/src/bridge/   window.orphyn for the Tauri shell (imported first by main.tsx)
src/shared/webview/        webview engine detection + media policy per engine
```

Every module exposes `pub fn init(app: &AppHandle) -> Result<()>` and its `#[tauri::command]`s.
All crates the next migration tasks need are already in `Cargo.toml` (mdns-sd, rust_cast,
quick-xml, discord-rich-presence, souvlaki, reqwest with rustls, tokio...), so filling a stub only
touches its own folder: no edit to `Cargo.toml` or `lib.rs` is needed unless a command is added.

Plugins: single-instance (a second launch focuses the window), log, window-state (bounds,
maximized, fullscreen; restored before the window is shown), updater, process, opener, os.

## Commands and events

Naming: command `<domain>_<action>` (snake_case), arguments in camelCase on the JS side (Tauri maps
`deviceId` to `device_id`). Events reuse the Electron channel names of `src/shared/ipc/*`.

| Domain         | Commands                                                                                                                                                                                                    | Events (Rust -> renderer)                                                    |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| window         | `window_minimize`, `window_toggle_maximize`, `window_close`, `window_get_state`, `window_set_fullscreen {fullscreen}`, `window_set_appearance {appearance}`                                                   | `window:state-changed` (`WindowState`)                                       |
| accounts       | `accounts_list`, `accounts_add {input}`, `accounts_rename {id,name}`, `accounts_remove {id}`, `accounts_set_active {id}`, `accounts_get_active`, `accounts_get_token {id}`, `accounts_get_device_id`, `accounts_get_status` | -                                                                            |
| updater        | `updater_get_state`, `updater_check`, `updater_install`, `updater_open_release`, `updater_set_auto_update {enabled}`                                                                                         | `orphyn:updater:state-changed` (`UpdaterState`)                              |
| cast (stub)    | `cast_start_discovery`, `cast_stop_discovery`, `cast_list_devices`, `cast_get_session {deviceId}`, `cast_load {deviceId,media}`, `cast_play/pause/stop/disconnect {deviceId}`, `cast_seek {deviceId,positionSec}`, `cast_set_volume {deviceId,volume}`, `cast_set_muted {deviceId,muted}` | `cast:devices-changed`, `cast:session-changed`, `cast:ended`, `cast:error` |
| discord (stub) | `discord_get_state`, `discord_set_presence {presence}`, `discord_set_settings {settings}`                                                                                                                    | `orphyn:discord:state-changed`                                               |
| media controls (stub) | `media_controls_set_metadata {metadata}`, `media_controls_set_playback {playback}`, `media_controls_clear`                                                                                            | `orphyn:media-controls:action` (`MediaControlAction`)                        |
| pip (stub)     | `pip_get_state`, `pip_open {kind}`, `pip_close`                                                                                                                                                              | `orphyn:pip:state-changed` (`PipState`)                                      |

The contracts for media controls and PiP are new (`src/shared/ipc/mediaControls.ts`, `pip.ts`) and
are optional members of `OrphynApi` (`pip?`, `mediaControls?`): Electron does not provide them.
The tasks that fill these modules may refine those contracts.

## The bridge (`src/renderer/src/bridge/`)

`window.rs` injects `window.__ORPHYN_SHELL__ = { platform, webview }` before the page loads
(`platform` uses Node names: `win32`, `darwin`, `linux`). `main.tsx` imports `@renderer/bridge`
first; when that global exists, the bridge defines `window.orphyn` (frozen) with:

- `window`, `accounts`, `updater`, `cast`, `discord`, `pip`, `mediaControls`: one function per
  command, `on*` functions wrap `listen` and return a synchronous unsubscribe.
- `app`: no native menu in Tauri, so the former menu accelerators are keyboard shortcuts:
  Ctrl+R / Cmd+R calls the `onRefresh` listeners (and no longer reloads the page), Ctrl+, / Cmd+,
  calls `onOpenSettings`.
- `platform`: from `__ORPHYN_SHELL__`.

In Electron the preload already defined `window.orphyn`, so the bridge does nothing; in a browser
or in tests (no `__ORPHYN_SHELL__`) it does nothing either.

### Window chrome and drag regions

| Platform | Window                                                                             |
| -------- | ---------------------------------------------------------------------------------- |
| macOS    | Overlay title bar, hidden title, traffic lights at (20, 20), `Sidebar` vibrancy    |
| Windows  | No native frame, transparent window + Mica (Windows 11); opaque colour on Windows 10 |
| Linux    | Native decorations                                                                  |

`WindowState.material` reports what was actually applied (`vibrancy`, `mica`, `none`), so the
renderer keeps its sidebar transparent only when a material is behind it.

Drag regions stay defined by the renderer's CSS class: **any element with `jr-drag` drags the
window**, except interactive children (`button, a, input, select, textarea, [role='button'],
[tabindex], .jr-no-drag`), the same selector as `styles/base.css`. Tauri's
`data-tauri-drag-region` only works on the marked element itself (not its children), so the bridge
listens to `mousedown` and calls `startDragging()`; a double click maximizes or restores. Feature
code keeps using `jr-drag` / `jr-no-drag` and nothing else.

Windows caption buttons: Electron drew native ones over the page (`titleBarOverlay`); Tauri has no
such overlay, so the bridge adds minimize / maximize-restore / close buttons (Segoe Fluent Icons) in
the 140px the shell already reserves at the top right (`--titlebar-inset-right`), 52px tall
(`TITLE_BAR_HEIGHT`). `window.orphyn.window.setTitleBarOverlay(colors)` recolours them, they hide
in fullscreen and dim when the window is not focused. Their root is `#orphyn-caption-buttons`.

Fullscreen for the video player: `window.orphyn.window.setFullscreen` -> `window_set_fullscreen`.

## Accounts

Same `accounts.json` model as Electron (version 1, camelCase, `deviceId`, `activeId`, `accounts`),
in the app data dir (`%APPDATA%\app.lumen.desktop`, `~/Library/Application Support/app.lumen.desktop`,
`~/.local/share/app.lumen.desktop`). `accessToken` is always written as `null`: tokens are in the
OS keychain (service `Orphyn`, user = account id: Windows Credential Manager, macOS Keychain,
Linux Secret Service). The keychain is probed with a read at startup; when it is unusable,
`getStatus()` returns `{ encrypted: false, warning }` and tokens stay in memory only (same text as
Electron). Passwords are never stored.

## Updater

`tauri-plugin-updater` reads
`https://github.com/Dyskolos-Dev/Orphyn/releases/latest/download/latest.json`, signed with the key
pair generated by `npx tauri signer generate` (public key in `tauri.conf.json`). Same UX as
Electron: first check 10 s after launch, then every 6 h when automatic updates are on (setting in
`updater.json` in the app config dir); release builds only (`supported: false` in dev).

- Windows (NSIS, passive install), macOS (the updater swaps the .app bundle, no Squirrel
  signature needed) and Linux AppImage: background download (`downloading`, percent), then
  `ready` -> the "Restart" toast calls `install()`, which installs and restarts. A downloaded
  update also installs when the app quits.
- Linux .deb: `available` -> the guided download opens the release page.

## Video in each webview (`src/shared/webview/`)

`currentWebviewEngine()` returns `webview2` / `wkwebview` / `webkitgtk` in Tauri, `chromium` in
Electron. `mediaPolicyFor(engine)` gives what the device profile must follow:

| Engine          | Direct play                                                         | HLS                | Notes                              |
| --------------- | ------------------------------------------------------------------- | ------------------ | ---------------------------------- |
| chromium / webview2 | as today (H.264, HEVC/AV1/VP9 when detected, WebM, Opus)        | hls.js             | unchanged                          |
| wkwebview       | H.264, HEVC; MP4 audio AAC, MP3, ALAC, FLAC, AC-3/E-AC-3; no WebM, no VP9/Opus in MP4 | native (`.m3u8` on `<video>`) | profile name "Orphyn (macOS)" |
| webkitgtk       | H.264 + AAC/MP3 only                                                | hls.js (MSE)       | prefer transcoding, 40 Mbps cap    |

`restrictSupport(detectCodecSupport(), policy)` limits the `canPlayType` detection to what the
engine allows. The data layer applies it in `src/renderer/src/jellyfin/playback/profile.ts`
(`buildDeviceProfile`: support, MP4 audio list, WebM entries, profile name, bitrate cap) and the
HLS choice in the video player; that file belongs to the data-layer owner.

Known difference seen on WebView2 in dev: external `.vtt` subtitle tracks from the server are
refused as cross-origin ("Unsafe attempt to load URL ... Domains, protocols and ports must
match"). `<track>` needs CORS: set `crossOrigin="anonymous"` on the `<video>`, or fetch the VTT
and pass a `blob:` URL (video player owner).

## Migration from Electron

- **Servers**: on first start (no `accounts.json` in the Tauri data dir), the Electron file is read
  from `<config dir>/{Orphyn,orphyn,Lumen,lumen}/accounts.json` (`%APPDATA%`,
  `~/Library/Application Support`, `~/.config`). Servers, users, names, the active server and the
  device id are imported; the tokens are not (Electron `safeStorage` ciphertext cannot be decrypted
  outside Electron). Each server shows the usual "sign in again" flow (`hasToken: false`).
- **Preferences** (theme, accent, language, player settings in `localStorage`): not migrated.
  Electron keeps them in Chromium's LevelDB (`Local Storage/leveldb`), whose records are
  Snappy-compressed and Chromium-encoded: not a trivial read. They reset to their defaults on the
  first Tauri launch.
- Window size and position: reset once (the window-state plugin keeps its own file).
- Identifier `app.lumen.desktop` is kept for continuity of OS-level registrations.

## CI (to apply in `.github/workflows/ci.yml`, by the last migration task)

The `check` job already runs `npm run check`, which now includes `check:rust`: each runner needs
Rust and, on Linux, the system packages. Add before "Run npm run check":

```yaml
      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install Tauri system packages (Linux)
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
            librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libdbus-1-dev patchelf
```

Release workflow (Tauri builds): `tauri-apps/tauri-action@v0` on the same matrix, with
`TAURI_SIGNING_PRIVATE_KEY` (repository secret, the content of the private key file) and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty) in `env`, `includeUpdaterJson: true` so the release
carries `latest.json`, and macOS built for both `aarch64-apple-darwin` and `x86_64-apple-darwin`.
