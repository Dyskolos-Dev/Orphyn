<div align="center">

<img src="build/icon.png" alt="Lumen" width="160" height="160" />

# Lumen

**Un superbe client de bureau pour Jellyfin : tes films, séries et musiques, façon Apple.**

[![Dernière version](https://img.shields.io/github/v/release/Dyskolos-Dev/Lumen?style=for-the-badge&label=version&color=8b8cf8)](https://github.com/Dyskolos-Dev/Lumen/releases/latest)
[![Licence : MIT](https://img.shields.io/badge/licence-MIT-5ec8d8?style=for-the-badge)](LICENSE)
[![Plateformes](https://img.shields.io/badge/plateformes-Windows%20%7C%20macOS%20%7C%20Linux-b9a7e8?style=for-the-badge)](https://github.com/Dyskolos-Dev/Lumen/releases/latest)
[![Jellyfin](https://img.shields.io/badge/Jellyfin-10.9%2B-00a4dc?style=for-the-badge&logo=jellyfin&logoColor=white)](https://jellyfin.org)

[**Télécharger**](https://github.com/Dyskolos-Dev/Lumen/releases/latest) · [Site](https://lumen.dyskolos.fr) · [English](README.md)

</div>

---

## ✨ Fonctionnalités

| | |
|---|---|
| 🎬 **Films et séries** | Accueil façon Apple TV avec bannière, « Reprendre », « À suivre » et « Ajouts récents » ; pages détaillées, saisons et épisodes. |
| ▶️ **Lecteur intégré** | Lecture directe ou transcodage par le serveur (HLS), reprise, pistes audio et sous-titres, épisode suivant automatique, raccourcis clavier. |
| 🎵 **Musique** | Bibliothèque façon Apple Music : albums, artistes, playlists, file d'attente, aléatoire et répétition, « À l'écoute » en plein écran. |
| 🎤 **Paroles synchronisées** | Les paroles défilent avec la chanson ; un clic sur une ligne y saute. |
| 📡 **Cast** | Google Cast (musique et vidéo), lecteurs UPnP/DLNA dont WiiM, AirPlay vers Apple TV (au mieux). |
| 🗂️ **Plusieurs serveurs** | Ajoute un serveur avec son adresse, ton compte et un nom ; change en un clic. Seul un jeton chiffré est conservé, jamais ton mot de passe. |
| 🔎 **Recherche globale** | Films, séries, épisodes, artistes, albums et titres au même endroit. |
| 🌗 **Clair et sombre** | Suit ton système, avec la couleur d'accent de ton choix. Français et anglais. |
| 🎮 **Discord Rich Presence** | « Écoute » / « Regarde » avec la pochette, activable dans les Réglages. |
| ⌨️ **Comme une app native** | Touches multimédia et contrôles média du système, menu macOS, ⌘/Ctrl+R pour rafraîchir la bibliothèque. |
| 🔄 **Mises à jour automatiques** | Les nouvelles versions s'installent seules sur Windows et Linux (AppImage) ; sur macOS, Lumen te prévient et ouvre le téléchargement. |

## 📥 Installation

Télécharge l'installeur de ton système dans la [dernière release](https://github.com/Dyskolos-Dev/Lumen/releases/latest) :

| Système | Fichier |
|---|---|
| Windows 10/11 | `Lumen Setup x.y.z.exe` |
| macOS (Intel et Apple silicon) | `Lumen-x.y.z.dmg` |
| Linux | `Lumen-x.y.z.AppImage` ou `.deb` |

> [!NOTE]
> Les builds ne sont pas encore signés. Sur Windows, clique sur **Informations complémentaires → Exécuter quand même** dans SmartScreen. Sur macOS, fais un clic droit sur l'app puis **Ouvrir** la première fois.

Il te faut un serveur [Jellyfin](https://jellyfin.org) en version 10.9 ou plus.

## 🛠️ Développement

Node.js 22+ et npm sont nécessaires.

```bash
npm install
npm run dev
```

| Script | Rôle |
|---|---|
| `npm run dev` | Lance l'app avec rechargement à chaud |
| `npm run check` | Types, lint, tests unitaires et build |
| `npm run e2e` | Tests de bout en bout sur un vrai serveur (voir [docs/QA.md](docs/QA.md)) |
| `npm run dist` | Construit l'installeur du système courant dans `release/` |

Les releases sont construites par GitHub Actions quand un tag `v*` est poussé (voir [CONTRIBUTING.md](CONTRIBUTING.md)).

Pour aller plus loin : [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/DESIGN.md](docs/DESIGN.md), [docs/JELLYFIN.md](docs/JELLYFIN.md), [docs/CASTING.md](docs/CASTING.md) et [docs/DISCORD.md](docs/DISCORD.md).

## 📄 Licence

Lumen est distribué sous [licence MIT](LICENSE).

Lumen est un projet indépendant, sans lien avec Jellyfin ni Apple.

<div align="center">

Fait avec ❤️ par [Dyskolos_](https://dyskolos.fr)

</div>
