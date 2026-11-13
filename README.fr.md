# Lumen

**Lumen** est un client de bureau pour [Jellyfin](https://jellyfin.org), le serveur multimédia libre et gratuit. Il lit votre musique, vos films et vos séries sur Windows, macOS et Linux, avec une interface inspirée des applications Apple TV et Apple Musique.

> **Statut : en développement.** Lumen n'est pas encore publié. Les fonctionnalités ci-dessous correspondent au périmètre prévu pour la v1 et sont en cours de réalisation.

Langue : [English](README.md) · **Français**

## Fonctionnalités

Prévu pour la v1 (toutes **en cours**) :

- **Musique** : parcourir les albums, artistes, playlists et titres ; un lecteur musical complet.
- **Films et séries** : parcourir les bibliothèques, les fiches détaillées et un lecteur vidéo.
- **Recherche** dans vos bibliothèques.
- **Plusieurs serveurs** : connecter plusieurs serveurs Jellyfin et passer de l'un à l'autre.
- **Interface bilingue** : français et anglais, selon la langue du système par défaut.
- **Diffusion (cast)** : Google Cast, UPnP/DLNA (y compris les appareils WiiM), et AirPlay en mode « best effort ».
- **Esthétique Apple TV / Apple Musique** : un design cohérent et soigné.

## Prérequis

- [Node.js](https://nodejs.org) **22 ou plus récent**
- npm (fourni avec Node.js)
- Un serveur Jellyfin **10.9 ou plus récent**, accessible depuis votre ordinateur

## Mise en place pour le développement

```bash
git clone <url-du-depot> lumen
cd lumen
npm install
npm run dev
```

`npm run dev` lance l'application avec electron-vite : le renderer bénéficie du rechargement à chaud (HMR) et le processus principal redémarre à chaque modification.

## Scripts

| Script                 | Rôle                                                                        |
| ---------------------- | --------------------------------------------------------------------------- |
| `npm run dev`          | Lance l'application en mode développement (electron-vite dev)               |
| `npm run build`        | Construit les bundles main, preload et renderer dans `out/`                 |
| `npm run preview`      | Prévisualise le build de production (electron-vite preview)                 |
| `npm run typecheck`    | Vérification TypeScript du code main/preload/shared, puis du renderer       |
| `npm run lint`         | Analyse le projet avec ESLint                                               |
| `npm run format`       | Formate le projet avec Prettier (modifie les fichiers)                      |
| `npm run format:check` | Vérifie le formatage avec Prettier sans modifier les fichiers               |
| `npm test`             | Lance une fois les tests unitaires et de composants avec Vitest (jsdom)     |
| `npm run check`        | Typecheck, lint, tests et build : à lancer avant d'ouvrir une PR            |
| `npm run dist`         | Construit puis génère les installeurs pour le système d'exploitation actuel |

## Créer les installeurs

`npm run dist` construit l'application et la package avec electron-builder dans le dossier `release/`. Le packaging ne produit des installeurs que pour le système sur lequel il est lancé :

| Lancé sous | Installeurs produits                                        |
| ---------- | ----------------------------------------------------------- |
| Windows    | Installeur NSIS (`.exe`)                                    |
| macOS      | Image disque DMG, pour Intel (x64) et Apple Silicon (arm64) |
| Linux      | AppImage et paquet Debian (`.deb`)                          |

```bash
npm run dist
```

Les installeurs ne sont pas encore signés numériquement. La signature Windows et macOS (certificats, notarisation) est à faire ; voir le workflow de publication dans `.github/workflows/release.yml`. Les versions officielles pour les trois systèmes sont produites par le workflow GitHub Actions de publication lorsqu'un tag `v*` est poussé.

## Organisation du projet

Lumen est une application Electron + React 19 + TypeScript (strict). Le code est réparti entre un processus principal (`src/main`), un pont preload (`src/preload`), les contrats IPC partagés (`src/shared`) et le renderer (`src/renderer/src`), organisé par fonctionnalité. Lisez ces documents avant toute modification :

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) : dossiers, points de composition, conventions IPC et i18n.
- [docs/DESIGN.md](docs/DESIGN.md) : système de design et directives visuelles.
- [docs/JELLYFIN.md](docs/JELLYFIN.md) : comment Lumen communique avec les serveurs Jellyfin.

## Contribuer

Les contributions sont les bienvenues. Consultez [CONTRIBUTING.md](CONTRIBUTING.md) pour les noms de branches, les messages de commit, la règle d'internationalisation et les vérifications à lancer avant une pull request.

## Licence

Lumen est distribué sous licence MIT (voir [LICENSE](LICENSE)). La licence n'est pas encore définitive et peut changer avant la première version publiée.

## Avertissement

Lumen est un projet indépendant. Il **n'est ni affilié, ni approuvé, ni parrainé** par le projet Jellyfin ni par Apple Inc. « Jellyfin » est une marque du projet Jellyfin ; « Apple TV », « Apple Musique », « AirPlay » et « Apple » sont des marques d'Apple Inc. Elles ne sont citées que pour décrire la compatibilité et l'inspiration visuelle.
