# Spike A: Dioxus Native (Blitz) for Orphyn

Standalone cargo project (`native-spikes/dioxus`), `dioxus-native 0.8.0-alpha.3` (Blitz 0.3 beta: Stylo + Taffy + Vello hybrid, wgpu). No webview. Data is real: Jellyfin login via env vars (`ORPHYN_E2E_*`, optionally a git-ignored `.env`), a random album with artwork and tracks (`ORPHYN_ALBUM_ID` pins one), real audio playback.

## What was built

| Item | Result |
|---|---|
| 1. Jellyfin login + client (reqwest/serde) | Done (`src/jellyfin.rs`): auth, album, tracks, image and audio URLs. |
| 2. Album page | Done. The app's **real CSS, unmodified** (tokens, controls, lists, media, navigation, shell, music: 118 KB concatenated by `scripts/sync-css.sh`; the only rewrite is `html[..]` → `.jr-root[..]`). DOM mirrors `AlbumPage`/`DetailHeader`/`TrackRows`. Backdrop, header, buttons, hover rows, real images, dark theme, sidebar. |
| 3. Mini player + Now Playing sheet | Done, with defects below. Progress bar updates from the audio thread and seeks on click. Sheet slides in with the same spring constants as framer-motion's `springs.smooth` (300/30/1), integrated in a Rust loop because Blitz has no JS animation. |
| 4a. Audio | Done: rodio 0.21 + symphonia, track downloaded then decoded; play/pause/seek/volume/next/prev, auto-advance. |
| 4b. **Video (libmpv)** | **Not done.** See "Video" below. |
| 5. Windows + Linux | Both build and run (screenshots below). fps not measured (see below). |

Screenshots (`screenshots/`): `win-album.png`, `win-sheet.png`, `linux-album.png`; references from the site: `ref-album-dark.webp`, `ref-mini-player-dark.webp`, `ref-now-playing-dark.webp`. The spike shows a different random album than the references, so compare layout, not content. Window is 840x540 logical (the test screen is 200 % DPI).

## What matches the current app (without any porting of CSS)

Sidebar (translucent surface, accent icons, active row), toolbar, big artwork with rounded corners and shadow, title/subtitle/meta typography, filled and tinted buttons, circular gray icon buttons, track rows (hover fill, current-row accent), `color-mix`, custom properties and `calc()`, flex layouts, SVG icons (lucide paths inline), the blurred and veiled artwork backdrop (`filter: blur` + gradient), the Now Playing blurred blob background and artwork shadow, rounded `overflow:hidden` clipping.

## What did not render / differs (seen in the screenshots)

- **Mini player layout is wrong**: the capsule is narrow and title/subtitle are cut with no ellipsis ("Live Wire ("), so `text-overflow: ellipsis`/`.jr-truncate` and likely the `@container` rules (`container-type: inline-size`) do not behave. The elapsed/remaining times and progress fill are not visible.
- **Sliders**: the thumb sits at the start even at 100 % volume, so the `--jr-slider-value` custom property inside `calc()`/`%` is not resolved the way Chrome does it.
- **Segmented control** (Now Playing tabs) stacks vertically: its thumb/grid positioning CSS did not apply; I did not chase it.
- **Fonts**: no `@font-face`/Inter wired in; on Windows the system font is used, on Linux DejaVu Sans. Inter would have to be bundled and registered explicitly.
- **Hover-only elements** (the `.jrm-heart--off` opacity rule, row play button) were not verified, the capture is non-interactive.
- **Transitions / CSS animations**: not verified working (`jr-ambient-in` fade, `transition` on times). Assume absent: Blitz has no animation engine in this version, so every animation needs a Rust-driven loop like the sheet spring.
- **`backdrop-filter`** (`.jr-material-*`: the mini player, toolbar materials): the code path exists in blitz-paint, but I did not confirm a blur behind the bar; the player bar looked flat-translucent.
- Not ported in the spike: multi-disc headers, context menus/popovers, text selection, drag and drop, keyboard navigation, scroll-driven toolbar collapse (Blitz has no `IntersectionObserver`/`ResizeObserver`, scroll events need Rust glue).
- Native window decorations only (no custom titlebar).

## Video (libmpv): not attempted, and why

I did not integrate libmpv; I did not want to claim a half result. Findings from reading the code:
- Blitz's default renderer here is **vello-hybrid**; the only hook for foreign GPU content is `CustomPaintSource` (a `<canvas>`-like node) and it is wired for the **vello** feature (`wgpu_context`), i.e. a renderer switch to be validated.
- libmpv's render API offers **OpenGL and software (SW) only**, no wgpu/D3D11/Vulkan target. So video in the Blitz surface means either SW render plus a per-frame CPU→wgpu texture upload (workable for 1080p at the cost of CPU and a copy, poor for 4K/HDR) or a GL context sharing hack that does not map to wgpu on Windows (D3D12/Vulkan).
- The robust alternative is a **child native window/surface** for mpv (`wid` option) under the Blitz window, with the control bar as a separate transparent overlay window. On Windows that works (HWND child), on Linux/Wayland it is hard (no reparenting; subsurfaces needed).
- Also needs a libmpv build for Windows (not present on this machine) and `libmpv-dev` on Linux (installed in WSL).
Cost estimate to prove it properly: 1 to 2 days. It is the **largest unknown** of this stack.

## Linux (WSL Debian + WSLg, Wayland)

Builds and runs with the same code (packages: build-essential, pkg-config, libasound2-dev, libxkbcommon-dev, libwayland-dev, libx11-dev, libvulkan-dev, mesa-vulkan-drivers, libssl-dev, python3). The look is identical apart from the font. In WSLg wgpu used **llvmpipe (software Vulkan)**, so Linux performance numbers on this machine say nothing about real GPUs. Audio works once ALSA is routed to PulseServer (`/etc/asound.conf` + `PULSE_SERVER=unix:/mnt/wslg/PulseServer`); without it rodio reports "no output device".

## Numbers

| | Windows | Linux (WSL) |
|---|---|---|
| Release binary | 33.2 MB (34,835,456 B) | 46.2 MB (48,410,008 B, not stripped) |
| Clean release build | about 5 to 6 min (12 cores, LTO thin) | about 12 min (WSL, 12 cores) |
| Incremental rebuild after a code change | about 20 s to 3 min (thin LTO relinking) | not measured |
| RAM (working set) | 350 to 440 MB with a 1080p-sized window | not measured |
| Start to first frame | about 1.2 s login/album fetch + under 2 s window | similar |

**fps / smoothness: not measured.** I could not drive scroll/animations automatically and did not build with `log-frame-times`. The sheet spring ran from a 8 ms timer and completed. Anyone evaluating should rebuild with `--features dioxus-native/log-frame-times` and scroll the track list.

Build trap: `stylo` needs a real **Python 3** at build time (on Windows the Microsoft Store `python.exe` stub fails: set `PYTHON3` to a real install).

## Verdict

**Conditional go for the UI, no-go until video is proven.**

For: the reuse promise holds: the 7.8k lines of CSS rendered a faithful page with essentially no CSS edits, and React components map to RSX 1:1 (this spike's pages are about 450 lines of RSX for what is about 700 lines of TSX plus design-system components). Windows and Linux both work from one code base, no webview.

Against / risks, in order:
1. **Video in the window** is unsolved and the design (mpv render API vs wgpu) is awkward (see above). This decides the whole port.
2. **CSS gaps are silent**: Blitz drops unsupported features without error (container queries, truncation, calc with variables, segmented control). With 7.8k lines of CSS, expect a long tail of visual fixes, plus visual regression tests that do not exist yet.
3. **No animation/transition engine, no scroll/resize observers**: every `transition`, framer-motion spring, sticky-collapse title and virtualised list must be rewritten as Rust state machines.
4. **Pre-release stack**: dioxus 0.8 alpha, Blitz 0.3 beta, Stylo from git snapshots, winit 0.31 beta; API churn is certain. Hand-written pieces needed for menus, popovers, focus management, accessibility (accesskit is wired but untested), text input and IME, drag and drop.
5. Binary 33 MB and a 5 to 12 min cold build (Stylo + wgpu + Vello); acceptable.

Recommendation: before committing to the 32k-line port, spend one more focused day on the video path (child window on Windows plus SW-render fallback), and compare with the GPUI spike on that same criterion.

## Reproduce

```
# repo root, Windows (PowerShell); needs a real Python for stylo
$env:PYTHON3 = "$env:LOCALAPPDATA\Programs\Python\Python314\python.exe"
cd native-spikes/dioxus
# .env (git-ignored) or env: ORPHYN_E2E_URL / ORPHYN_E2E_USER / ORPHYN_E2E_PASSWORD
cargo build --release
./target/release/orphyn-dioxus-spike      # ORPHYN_AUTOPLAY=1 ORPHYN_OPEN_SHEET=1 ORPHYN_WIN=840x540 are debug flags
scripts/shot.ps1 -Out screenshots/win-album.png [-Sheet]
# CSS / icons are regenerated from the app sources:
sh native-spikes/dioxus/scripts/sync-css.sh; node native-spikes/dioxus/scripts/gen-icons.mjs
```
