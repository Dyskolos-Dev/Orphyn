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

## Reporting bugs and requesting features

Open an issue using the bug report or feature request template. For bugs, include your operating system, Lumen version, Jellyfin server version and the steps to reproduce.

## Licence

By contributing, you agree that your contribution is released under the project licence (see [LICENSE](LICENSE)).
