# Architecture

Orphyn is a desktop Jellyfin client for music, movies and series, written in Rust with Dioxus Native
(Blitz rendering, wgpu, libmpv for playback). Everything lives in the `native/` Cargo workspace.

## Commands

| Command                                  | What it does                                              |
| ---------------------------------------- | --------------------------------------------------------- |
| `cd native && cargo run -p orphyn-native` | runs the app                                              |
| `npm run check`                          | `cargo clippy -D warnings` + `cargo test` on the workspace |
| `npm run dist`                           | installers for the current OS (see `NATIVE-RELEASE.md`)   |

`package.json` only exists for the release tooling and as the single source of the app version
(`app_version()` reads it).

## Layout

| Path                                | What it holds                                                                   |
| ----------------------------------- | ------------------------------------------------------------------------------- |
| `native/crates/orphyn-core`         | framework-free logic: profile paths, accounts, cast, Discord, media keys, updater |
| `native/crates/orphyn-jellyfin`     | Jellyfin data layer: auth, endpoints, image URLs, query cache, SyncPlay          |
| `native/crates/orphyn-player`       | libmpv playback                                                                  |
| `native/crates/orphyn-native`       | the app: window, shell, design system, one folder per feature under `src/features` |
| `native/styles/`                    | stylesheets (`manifest.txt` lists them in order; `build.rs` bundles them), `overrides*` for Blitz fixes |
| `native/locales/`                   | `en` and `fr` JSON, one file per namespace, bundled by `build.rs`                |
| `native/notes/`                     | "What's new" notes, one JSON per version                                         |
| `native/assets/`                    | fonts and logo                                                                   |
| `build/`                            | app icons, installer art and scripts, Linux `.desktop` file                      |
| `scripts/`                          | packaging (`native-package.mjs`) and updater feed scripts used by the Release workflow |

## Conventions

- Every user-visible string exists in `native/locales/en` and `native/locales/fr`.
- Each feature owns its folder in `orphyn-native/src/features`; shared widgets live in `src/design`.
- App icon: `build/icon.ico` is embedded in the Windows exe by `build.rs`, the window icon and the
  Linux `app_id` / `WM_CLASS` (`orphyn`) are set in `src/window.rs`, `build/icons/icon.icns` goes in
  the macOS bundle.
