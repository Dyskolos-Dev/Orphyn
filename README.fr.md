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
> Les builds ne sont pas encore signés. Sur Windows, clique sur **Informations complémentaires → Exécuter quand même** dans SmartScreen.
>
> Sur macOS, glisse Orphyn dans Applications puis ouvre-le. Si macOS dit qu'il ne peut pas vérifier l'app, ouvre **Réglages Système → Confidentialité et sécurité** et clique sur **Ouvrir quand même** (une seule fois). S'il dit que l'app est **endommagée**, lance ceci dans le Terminal, puis rouvre-la :
>
> ```bash
> xattr -cr /Applications/Orphyn.app
> ```
>
> Les mises à jour s'installent ensuite depuis l'app, sans autre étape.

Orphyn 2 est une app entièrement native (Rust, sans moteur web). La vidéo est lue par [libmpv](https://mpv.io), que les installeurs embarquent : rien à installer sur Windows, macOS ni avec l'AppImage. Le `.deb` dépend du paquet `libmpv2` (Ubuntu 24.04 / Debian 13 et plus récents), que `apt` installe pour toi. Si tu utilises déjà Orphyn 1.5, il se met à jour vers la version native et garde tes comptes et tes réglages.

Il te faut un serveur [Jellyfin](https://jellyfin.org) en version 10.9 ou plus.

## 🛠️ Développement

Rust stable (1.85+) est nécessaire. L'app est écrite en Rust avec Dioxus Native (Blitz, wgpu, libmpv) et vit dans [`native/`](native/README.md). Sous Linux, il faut les paquets listés dans [docs/NATIVE-RELEASE.md](docs/NATIVE-RELEASE.md), et libmpv à l'exécution (`ORPHYN_MPV_DIR` sous Windows). Node.js 22+ ne sert qu'à empaqueter les installeurs.

```bash
cd native
cargo run -p orphyn-native
```

| Commande | Ce qu'elle fait |
|---|---|
| `cd native && cargo run -p orphyn-native` | Lance l'app |
| `npm run check` | Clippy (avertissements = erreurs) et tests du workspace Rust |
| `npm run dist` | Construit les installeurs natifs du système courant dans `dist/native/` (voir [docs/NATIVE-RELEASE.md](docs/NATIVE-RELEASE.md)) |

Les releases sont construites par GitHub Actions quand un tag `v*` est poussé (voir [CONTRIBUTING.md](CONTRIBUTING.md)).

Pour aller plus loin : [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/DESIGN.md](docs/DESIGN.md), [docs/JELLYFIN.md](docs/JELLYFIN.md), [docs/CASTING.md](docs/CASTING.md) et [docs/DISCORD.md](docs/DISCORD.md).

## 📄 Licence

Orphyn est distribué sous [licence MIT](LICENSE).

Orphyn est un projet indépendant, sans lien avec Jellyfin.

<div align="center">

Fait avec ❤️ par [Dyskolos_](https://dyskolos.fr)

</div>
