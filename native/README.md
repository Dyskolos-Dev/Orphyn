# Orphyn native

The full-Rust Orphyn: Dioxus Native (Blitz: Stylo + Taffy + Vello on wgpu), no web engine. It renders
the React app's own stylesheets and translations, so the two apps cannot drift.

```
cd native
cargo run -p orphyn-native        # needs PYTHON3 = a real Python 3 at build time (stylo)
```

Development: a git-ignored `native/.env` with `ORPHYN_E2E_URL` / `ORPHYN_E2E_USER` /
`ORPHYN_E2E_PASSWORD` signs in when no account is saved. `scripts/dev-run.ps1` builds and starts the
app on a throwaway profile (`ORPHYN_USER_DATA_DIR`) and screenshots it; `scripts/click.ps1` clicks in
it. Other variables: `ORPHYN_ROUTE`, `ORPHYN_WIN`, `ORPHYN_LANG`, `ORPHYN_MPV_DIR`.

## Crates

| Crate | Role |
|---|---|
| `orphyn-core` | profile paths (same folders as the Tauri app), `local-storage.json` (the localStorage keys and values), accounts + keychain + artwork + Discord activity + updater state **copied** from `src-tauri` |
| `orphyn-jellyfin` | client (Orphyn identity), sign-in, endpoints, image URLs with device-pixel sizing, query cache (dedup, stale-while-revalidate, invalidation) |
| `orphyn-player` | music engine (queue port, rodio/symphonia, gapless hand-over, crossfade hook not implemented yet), libmpv video controller + Blitz video widget |
| `orphyn-native` | the app: shell, router, i18n, theme / interface styles, stores, design components, the album page / mini player / Now Playing reference port |

CSS: `build.rs` concatenates the files of `styles/manifest.txt` from `src/renderer/src` and
re-anchors `html`/`body`/`#root` selectors on `.jr-root`/`.jr-body`/`.jr-app`. Blitz-only fixes go in
`styles/overrides.css`. Icons: `node scripts/gen-icons.mjs` regenerates `design/icons.rs` from
lucide-react (keyed by the React import name).

## Pinned pre-release crates

Dioxus `=0.8.0-alpha.3`, blitz-dom `=0.3.0-beta.4`, winit `=0.31.0-beta.3`, anyrender, peniko,
kurbo, wgpu 30 and parley are pinned with `=`: these pre-releases change APIs between versions and
must move together. `vendor/blitz-dom` is 0.3.0-beta.4 with two hit-test fixes (nodes removed since
the last layout no longer panic; overflow containers clip hits) — re-apply when bumping.

## Migration of 1.5 users (plan)

Same identifier `app.lumen.desktop`, so the same data folder: `accounts.json` is read as is and the
tokens are the same keychain entries (service `Orphyn`). Settings: on first start, copy the webview's
localStorage entries (`orphyn.*`) into `local-storage.json` (WebView2 LevelDB on Windows, WebKit
`*.localstorage` SQLite on macOS / Linux). Window bounds use `native-window-state.json` so both builds
can be installed side by side.

## Known gaps

- Not done yet in this foundation: the Lyrics / Up Next panels, the multi-step
  onboarding (a single sign-in form stands in), the full Settings page (a theme / style / language
  stand-in), crossfade, macOS traffic-light offset (no winit API), Mica / vibrancy materials.
- Verified on Windows only; WSL Debian build and the CI run on the 3 OSes are not checked yet.

## Video

libmpv (software render API, `hwdec=auto-copy`) draws into a Blitz widget (`orphyn-player/src/video`);
the player page is `orphyn-native/src/features/video/player`. Frame pipeline (`ORPHYN_VIDEO_PIPELINE`):

- default: mpv renders at the **video's own size**, the frame is uploaded once and a wgpu render
  pass makes the texture the scene samples; the GPU scales it into the box (`object-fit: contain`).
  It asks mpv for packed YUV 4:2:2 (`yuyv422`, half the bytes, converted in the shader, BT.709 /
  BT.601 limited range). The software render API takes a single plane, so two-plane NV12 is not
  possible through it. The libmpv tested (Stremio's `libmpv-2.dll`) refuses `yuyv422`, so the frames
  come as `bgr0` through the same pass: no CPU alpha pass, no CPU scaling.
- `bgra`: the spike A2 path (BGRA at the box's pixel size, CPU alpha pass), kept to compare.

CPU at 1080p (Windows, AMD Radeon iGPU, release build, 1080p HEVC episode in direct play, `d3d11va-copy`,
window covering a 2560x1440 screen, process CPU over 10 to 20 s of playback):

| Pipeline | CPU |
|---|---|
| spike A2 (REPORT.md, 2560x1440 texture) | about 3 cores |
| `bgra` (same path in the app) | 1.57 cores |
| default (video size + GPU pass, `bgr0` frames) | 0.76 to 1.15 cores (3 runs) |

Next steps: a libmpv build that accepts `yuyv422` (or another packed YUV format) halves the upload
again; a GPU interop path (D3D11 / Vulkan) would remove the copy entirely.
