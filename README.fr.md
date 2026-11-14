<div align="center">

<img src="build/icon.png" alt="Orphyn" width="160" height="160" />

# Orphyn

**Un superbe client de bureau pour Jellyfin : tes films, séries et musiques, sous son meilleur jour.**

[![Dernière version](https://img.shields.io/github/v/release/Dyskolos-Dev/Orphyn?style=for-the-badge&label=version&color=8b8cf8)](https://github.com/Dyskolos-Dev/Orphyn/releases/latest)
[![Licence : MIT](https://img.shields.io/badge/licence-MIT-5ec8d8?style=for-the-badge)](LICENSE)
[![Plateformes](https://img.shields.io/badge/plateformes-Windows%20%7C%20macOS%20%7C%20Linux-b9a7e8?style=for-the-badge)](https://github.com/Dyskolos-Dev/Orphyn/releases/latest)
[![Jellyfin](https://img.shields.io/badge/Jellyfin-10.9%2B-00a4dc?style=for-the-badge&logo=jellyfin&logoColor=white)](https://jellyfin.org)

[**Télécharger**](https://github.com/Dyskolos-Dev/Orphyn/releases/latest) · [Site](https://orphyn.dyskolos.fr) · [English](README.md)

</div>

---

## ✨ Fonctionnalités

| | |
|---|---|
| 🎬 **Films et séries** | Accueil cinématographique avec bannière, « Reprendre », « À suivre » et « Ajouts récents » ; pages détaillées, saisons et épisodes. |
| ▶️ **Lecteur intégré** | Lecture directe ou transcodage par le serveur (HLS), reprise, pistes audio et sous-titres, épisode suivant automatique, raccourcis clavier. |
| 🎵 **Musique** | Bibliothèque soignée : albums, artistes, playlists, file d'attente, aléatoire et répétition, « À l'écoute » en plein écran. |
| 🎤 **Paroles synchronisées** | Les paroles défilent avec la chanson ; un clic sur une ligne y saute. |
| 📡 **Cast** | Google Cast (musique et vidéo), lecteurs UPnP/DLNA dont WiiM, AirPlay vers Apple TV (au mieux). |
| 🗂️ **Plusieurs serveurs** | Ajoute un serveur avec son adresse, ton compte et un nom ; change en un clic. Seul un jeton est conservé, dans le trousseau du système, jamais ton mot de passe. |
| 🔎 **Recherche globale** | Films, séries, épisodes, artistes, albums et titres au même endroit. |
| 🌗 **Clair et sombre** | Suit ton système, avec la couleur d'accent de ton choix. Français et anglais. |
| 🎮 **Discord Rich Presence** | « Écoute » / « Regarde » avec la pochette, activable dans les Réglages. |
| ⌨️ **Comme une app native** | Touches multimédia et contrôles média du système, ⌘/Ctrl+, pour les Réglages, ⌘/Ctrl+R pour rafraîchir la bibliothèque. |
| 🪟 **Picture-in-Picture** | Garde le mini lecteur musique ou une vidéo dans une petite fenêtre au-dessus de tout le reste. |
| 🔄 **Mises à jour automatiques** | Les nouvelles versions se téléchargent et s'installent seules sur Windows, macOS et Linux (AppImage). |

## 📥 Installation

Télécharge l'installeur de ton système dans la [dernière release](https://github.com/Dyskolos-Dev/Orphyn/releases/latest) :

| Système | Fichier |
|---|---|
| Windows 10/11 | `Orphyn_x.y.z_x64-setup.exe` |
| macOS Apple silicon (M1 et plus récent) | `Orphyn_x.y.z_aarch64.dmg` |
| macOS Intel | `Orphyn_x.y.z_x64.dmg` |
| Linux | `Orphyn_x.y.z_amd64.AppImage` ou `Orphyn_x.y.z_amd64.deb` |

> [!NOTE]
> Les builds ne sont pas encore signés. Sur Windows, clique sur **Informations complémentaires → Exécuter quand même** dans SmartScreen. Sur macOS, fais un clic droit sur l'app puis **Ouvrir** la première fois.

Il te faut un serveur [Jellyfin](https://jellyfin.org) en version 10.9 ou plus.

## 🛠️ Développement

Node.js 22+, npm et Rust stable (1.85+) sont nécessaires. Sous Linux, il faut aussi les paquets système de Tauri listés dans [docs/TAURI.md](docs/TAURI.md).

```bash
npm install
npm run dev
```

| Script | Rôle |
|---|---|
| `npm run dev` | Lance l'app avec rechargement à chaud |
| `npm run check` | Types, lint, tests unitaires, build du renderer, puis format, lint et tests Rust |
| `npm run e2e` | Tests de bout en bout sur un vrai serveur (voir [docs/QA.md](docs/QA.md)) |
| `npm run dist` | Construit les installeurs du système courant dans `dist/tauri-target/release/bundle/` |

Les releases sont construites par GitHub Actions quand un tag `v*` est poussé (voir [CONTRIBUTING.md](CONTRIBUTING.md)).

Pour aller plus loin : [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/DESIGN.md](docs/DESIGN.md), [docs/JELLYFIN.md](docs/JELLYFIN.md), [docs/CASTING.md](docs/CASTING.md) et [docs/DISCORD.md](docs/DISCORD.md).

## 📄 Licence

Orphyn est distribué sous [licence MIT](LICENSE).

Orphyn est un projet indépendant, sans lien avec Jellyfin.

<div align="center">

Fait avec ❤️ par [Dyskolos_](https://dyskolos.fr)

</div>
