# Contributing to Lumen

Thank you for helping. This guide explains how to propose changes. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) first: it describes the folder ownership and the composition points you must respect.

## Getting started

```bash
npm install
npm run dev
```

Requirements: Node.js 22 or newer, npm, and a Jellyfin server 10.9+ to test against.

## Branch naming

Create a branch from `main` for every change. Use a lowercase prefix that matches the kind of change, then a short kebab-case description:

- `feat/<short-description>`: a new feature (`feat/music-player-queue`)
- `fix/<short-description>`: a bug fix (`fix/cast-device-timeout`)
- `docs/<short-description>`: documentation only (`docs/readme-fr`)
- `chore/<short-description>`: tooling, dependencies, CI (`chore/ci-matrix`)
- `refactor/<short-description>`: code change without behaviour change (`refactor/jellyfin-client`)

Do not push directly to `main`.

## Commit messages: Conventional Commits

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org):

```
<type>(<optional scope>): <short imperative summary>

<optional body explaining why>
```

Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.

Examples:

```
feat(music): add shuffle to the album player
fix(cast): retry UPnP discovery after a network change
docs: document the release workflow
```

Use `!` after the type or a `BREAKING CHANGE:` footer for incompatible changes.

## Before opening a pull request

Run the full check and make sure it passes:

```bash
npm run check
```

It runs typecheck, lint, tests and the build. Also run `npm run format:check` and fix any formatting issue with `npm run format`.

Pull requests must:

- target `main` and have a clear title in the Conventional Commits form;
- describe what changed and why, and how you tested it;
- keep one topic per PR where possible;
- include tests for new logic and bug fixes when practical;
- keep the CI green on Linux, Windows and macOS.

Use the pull request template: it lists these points as a checklist.

## Internationalisation (i18n) rule

Lumen ships in French and English. **Every user-visible string must exist in both languages**:

- add the key to `src/renderer/src/i18n/locales/en/<namespace>.json` **and** to `src/renderer/src/i18n/locales/fr/<namespace>.json`;
- use the same key in both files; never hard-code a user-visible string in a component;
- a feature edits only its own namespace file (see the namespace convention in `docs/ARCHITECTURE.md`).

A missing key in either language is a bug, and reviewers will ask for it to be fixed before merging.

## Releasing

1. Bump `version` in `package.json` (and `package-lock.json`, e.g. `npm version 1.1.0 --no-git-tag-version`) and add the entry to `CHANGELOG.md`.
   **Also add `src/renderer/src/features/whatsnew/notes/<version>.json`** (`{ "fr": { "title", "items" }, "en": { ... } }`, each item `{ "icon", "title", "text" }`, in both languages): it is the "What's new" sheet users see once after updating. A unit test fails when the running version has no notes.
2. Commit, then tag and push the tag: `git tag v1.1.0 && git push origin main v1.1.0`.
3. The **Release** workflow builds Windows, macOS and Linux and publishes **one** GitHub release holding every installer, the `latest.yml` / `latest-mac.yml` / `latest-linux.yml` update feeds and the blockmaps. The release is published (not a draft), otherwise installed apps cannot see it.

The installer artwork (`build/icon.ico`, the NSIS bitmaps, the DMG background) comes from the logo: regenerate it with `node build/scripts/generate-installer-assets.mjs` when the logo changes.

Installed apps check for updates 10 seconds after launch and every 6 hours (Settings > About). Windows (NSIS) and Linux AppImage download and install in the background; unsigned macOS builds and the `.deb` only tell the user a new version exists and open the release page.

Code signing is optional and out of scope for now. To enable it, add these repository secrets and drop `CSC_IDENTITY_AUTO_DISCOVERY` from the release workflow: `CSC_LINK` and `CSC_KEY_PASSWORD` (Windows `.pfx` / macOS Developer ID `.p12`), plus `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD` and `APPLE_TEAM_ID` for macOS notarization. Without a signed macOS build, in-app updates cannot be applied there.

## Reporting bugs and requesting features

Open an issue using the bug report or feature request template. For bugs, include your operating system, Lumen version, Jellyfin server version and the steps to reproduce.

## Licence

By contributing, you agree that your contribution is released under the project licence (see [LICENSE](LICENSE)).
