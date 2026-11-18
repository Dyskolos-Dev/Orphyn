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
