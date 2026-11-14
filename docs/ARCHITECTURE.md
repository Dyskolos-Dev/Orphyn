# Orphyn architecture

Tauri v2 (Rust core) + Vite + React 19 + TypeScript (strict), npm. Desktop Jellyfin client for music and
movies/series. The layout is built so several agents can work in separate folders without editing the
same files: every cross-cutting concern has a **composition point** (a barrel/registry file) that only
imports from feature-owned folders.

## Scripts

| Script                            | What it does                                    |
| --------------------------------- | ----------------------------------------------- |
| `npm run dev`                     | `tauri dev` (Vite HMR renderer + Rust core)     |
| `npm run build`                   | release build of the app, no installer          |
| `npm run typecheck`               | `tsc` for node configs/shared, web, then e2e    |
| `npm run lint` / `npm run format` | ESLint (flat config) / Prettier                 |
| `npm test`                        | vitest run (jsdom, Testing Library)             |
| `npm run check`                   | typecheck + lint + test + Rust (fmt, clippy, test) |
| `npm run dist`                    | installers for the current OS (see TAURI.md)    |

`package.json` is owned by the foundation: all runtime dependencies are already installed.

## Folders and owners

| Folder                                | Owner (agent)                                         | Contents                                                                        |
| ------------------------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------- |
| `src-tauri/`                          | foundation (+ one module per domain)                  | Rust core: window, accounts, cast, discord, media controls, PiP, updater (see `docs/TAURI.md`) |
| `src/renderer/src/bridge/`            | foundation (+ per-API files by the owner of that API) | `window.orphyn` bridge to the Rust commands and events                          |
| `src/shared/ipc/`                     | owner of each API file                                | IPC channel names + typed API contracts (`window.ts`, `accounts.ts`, `cast.ts`) |
| `src/renderer/src/app/`               | foundation                                            | `App.tsx`, `routes.tsx` (composition)                                           |
| `src/renderer/src/shell/`             | shell agent                                           | `AppShell.tsx` layout (sidebar + outlet)                                        |
| `src/renderer/src/design/`            | design agent                                          | Design-system components (barrel `index.ts`)                                    |
| `src/renderer/src/styles/`            | design agent                                          | Global CSS, tokens (imported by `styles/index.ts`)                              |
| `src/renderer/src/jellyfin/`          | data-layer agent                                      | SDK client, query hooks, stores                                                 |
| `src/renderer/src/i18n/`              | foundation                                            | i18n init; `locales/` files are owned per feature                               |
| `src/renderer/src/features/servers/`  | servers agent                                         | Onboarding / server + account management                                        |
| `src/renderer/src/features/settings/` | settings agent                                        | Settings                                                                        |
| `src/renderer/src/features/search/`   | search agent                                          | Search                                                                          |
| `src/renderer/src/features/video/`    | video agent                                           | Movies/series browse, detail, player                                            |
| `src/renderer/src/features/music/`    | music agent                                           | Albums, artists, playlists, songs, player                                       |
| `src/renderer/src/features/cast/`     | cast agent                                            | Cast UI                                                                         |
| `src/renderer/src/features/discord/`  | -                                                     | Mirrors the music and video players to Discord (sends a normalized presence)    |
| `build/`, `scripts/`                  | foundation                                            | Icons, installer pages and art, build and release scripts                       |

`features/*/StubPage` placeholders live in `shell/StubPage.tsx`; replace them with real pages.

## Composition points

1. **Native core** (`src-tauri/src/lib.rs`) registers the plugins, calls each module's `init`
   and lists every command; each domain owns its module folder (see `docs/TAURI.md`).
2. **Bridge** (`src/renderer/src/bridge/index.ts`, imported first by `main.tsx`) defines
   `window.orphyn` from one file per API (`window.ts`, `accounts.ts`, `cast.ts`...). Types:
   `src/shared/ipc/*.ts` (`OrphynApi` in `shared/ipc/index.ts`) and the global `Window` augmentation
   in `src/renderer/src/orphyn.d.ts`.
3. **Routes** (`src/renderer/src/app/routes.tsx`) spreads `xxxRoutes` arrays exported from
   `features/{servers,settings,search,video,music,cast}/routes.tsx` under the `AppShell` layout. A
   feature adds/changes routes only in its own `routes.tsx`. The router is a hash router.
4. **i18n** (`src/renderer/src/i18n/index.ts`) globs `./locales/*/*.json` (see below).
5. **Design / styles / data layer**: `design/index.ts`, `styles/index.ts`, `jellyfin/index.ts` are
   barrels owned by their agent; other agents import from `@renderer/design`, `@renderer/jellyfin`.

Path aliases: `@renderer/*` -> `src/renderer/src/*`, `@shared/*` -> `src/shared/*` (configured in
`vite.renderer.config.ts`, `vitest.config.ts` and the tsconfigs).

## Route table

| Path                        | Feature  | Page            |
| --------------------------- | -------- | --------------- |
| `/onboarding`               | servers  | Onboarding      |
| `/settings`                 | settings | Settings        |
| `/search?q=`                | search   | Search results  |
| `/video`                    | video    | Video home      |
| `/video/library/:libraryId` | video    | Library         |
| `/video/item/:itemId`       | video    | Item detail     |
| `/video/play/:itemId`       | video    | Player          |
| `/music`                    | music    | Music home      |
| `/music/albums`             | music    | Albums          |
| `/music/artists`            | music    | Artists         |
| `/music/playlists`          | music    | Playlists       |
| `/music/songs`              | music    | Songs           |
| `/music/album/:id`          | music    | Album detail    |
| `/music/artist/:id`         | music    | Artist detail   |
| `/music/playlist/:id`       | music    | Playlist detail |
| `/cast`                     | cast     | Cast (stub)     |

`/` redirects to `/video`. Query parameters (e.g. `q`) are read with `useSearchParams`. Link to routes
by these literal paths; do not invent new top-level paths without updating this table.

## IPC conventions

- Commands are `<domain>_<action>` (snake_case) Rust functions; events keep the channel names of
  `src/shared/ipc/<domain>.ts` (e.g. `window:state-changed`), next to the `<Domain>Api` interface.
- The bridge wraps commands in typed functions (`invoke`) and events in `onXxx(cb) => unsubscribe`
  functions (`listen`). Payloads are JSON, defined in `src/shared/ipc/<domain>.ts`.
- Secrets (tokens) stay in the native core and the OS keychain.
- The renderer reaches the API through `window.orphyn.<domain>`.

## i18n namespace convention

- Languages: `fr`, `en`. Default = OS language (`navigator.language`), fallback `en`; a stored choice
  (`localStorage`) wins.
- One JSON file per namespace per language: `src/renderer/src/i18n/locales/{fr,en}/<namespace>.json`.
  The namespace is the file name. `common.json` is the default namespace (shell, navigation, shared
  strings). A feature owns `<feature>.json` (`servers`, `settings`, `search`, `video`, `music`,
  `cast`) in **both** languages and never edits another feature's file.
- Files are discovered automatically via `import.meta.glob('./locales/*/*.json', { eager: true })`;
  no registration step is needed. Use `useTranslation('<feature>')`.

## Native shell

Tauri v2 (`src-tauri/`, Rust): see `docs/TAURI.md`. Drag regions are the `jr-drag` / `jr-no-drag`
classes. `src/shared/webview/` tells which web engine renders the app (WebView2, WKWebView,
WebKitGTK) and what it can play.
