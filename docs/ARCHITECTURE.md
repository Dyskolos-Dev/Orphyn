# Jellyra architecture

Electron + electron-vite + React 19 + TypeScript (strict), npm. Desktop Jellyfin client for music and
movies/series. The layout is built so several agents can work in separate folders without editing the
same files: every cross-cutting concern has a **composition point** (a barrel/registry file) that only
imports from feature-owned folders.

## Scripts

| Script                            | What it does                                    |
| --------------------------------- | ----------------------------------------------- |
| `npm run dev`                     | electron-vite dev (HMR renderer, restarts main) |
| `npm run build`                   | electron-vite build into `out/`                 |
| `npm run typecheck`               | `tsc` for node (main/preload/shared) then web   |
| `npm run lint` / `npm run format` | ESLint (flat config) / Prettier                 |
| `npm test`                        | vitest run (jsdom, Testing Library)             |
| `npm run check`                   | typecheck + lint + test + build                 |
| `npm run dist`                    | build + electron-builder for the current OS     |

`package.json` is owned by the foundation: all runtime dependencies are already installed.

## Folders and owners

| Folder                                | Owner (agent)                                         | Contents                                                                        |
| ------------------------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------- |
| `src/main/index.ts`                   | foundation                                            | App bootstrap; only calls the register functions below                          |
| `src/main/window/`                    | window agent                                          | `createMainWindow`, window IPC handlers                                         |
| `src/main/accounts/`                  | servers agent                                         | `registerAccountsIpc`, secure account/token storage                             |
| `src/main/cast/`                      | cast agent                                            | `registerCastIpc`, Chromecast/DLNA discovery                                    |
| `src/preload/`                        | foundation (+ per-API files by the owner of that API) | `window.jellyra` bridge                                                         |
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
| `build/`, `resources/`                | foundation                                            | Icons / packaging resources                                                     |

`features/*/StubPage` placeholders live in `shell/StubPage.tsx`; replace them with real pages.

## Composition points

1. **Main process** (`src/main/index.ts`) calls, in order: `registerAccountsIpc()`
   (`src/main/accounts/index.ts`), `registerCastIpc()` (`src/main/cast/index.ts`), then
   `createMainWindow()` (`src/main/window/index.ts`). Each owner edits only its folder; the
   function signatures stay the same.
2. **Preload** (`src/preload/index.ts`) exposes
   `window.jellyra = { window, accounts, cast, platform }` built from `src/preload/window.ts`,
   `accounts.ts`, `cast.ts`. Types: `src/shared/ipc/*.ts` (`JellyraApi` in `shared/ipc/index.ts`) and
   the global `Window` augmentation in `src/preload/index.d.ts`. Security: `contextIsolation: true`,
   `nodeIntegration: false`, `sandbox: true` (preload may only import `electron` and `@shared/*`).
3. **Routes** (`src/renderer/src/app/routes.tsx`) spreads `xxxRoutes` arrays exported from
   `features/{servers,settings,search,video,music,cast}/routes.tsx` under the `AppShell` layout. A
   feature adds/changes routes only in its own `routes.tsx`. The router is a hash router (works with
   `file://` in production).
4. **i18n** (`src/renderer/src/i18n/index.ts`) globs `./locales/*/*.json` (see below).
5. **Design / styles / data layer**: `design/index.ts`, `styles/index.ts`, `jellyfin/index.ts` are
   barrels owned by their agent; other agents import from `@renderer/design`, `@renderer/jellyfin`.

Path aliases: `@renderer/*` -> `src/renderer/src/*`, `@shared/*` -> `src/shared/*` (configured in
`electron.vite.config.ts`, `vitest.config.ts` and the tsconfigs; `@shared` works in main, preload and
renderer).

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

- Channel names are `<domain>:<action>` (e.g. `window:minimize`) and live as constants in
  `src/shared/ipc/<domain>.ts`, next to the `<Domain>Api` interface.
- Renderer -> main uses `ipcRenderer.invoke` / `ipcMain.handle` (promises). Main -> renderer events
  use `webContents.send` and are exposed as `onXxx(cb) => unsubscribe` functions in the preload API.
- Preload files never expose `ipcRenderer` itself, only typed functions. Payloads must be
  structured-cloneable and defined in `src/shared/ipc/<domain>.ts`.
- Main handlers are registered inside the domain's `registerXxxIpc()`. Secrets (tokens) stay in main.
- The renderer reaches the API through `window.jellyra.<domain>`.

## i18n namespace convention

- Languages: `fr`, `en`. Default = OS language (`navigator.language`), fallback `en`; a stored choice
  (`localStorage`) wins.
- One JSON file per namespace per language: `src/renderer/src/i18n/locales/{fr,en}/<namespace>.json`.
  The namespace is the file name. `common.json` is the default namespace (shell, navigation, shared
  strings). A feature owns `<feature>.json` (`servers`, `settings`, `search`, `video`, `music`,
  `cast`) in **both** languages and never edits another feature's file.
- Files are discovered automatically via `import.meta.glob('./locales/*/*.json', { eager: true })`;
  no registration step is needed. Use `useTranslation('<feature>')`.
