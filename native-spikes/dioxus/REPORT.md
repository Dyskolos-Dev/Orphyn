# Spike A: Dioxus Native (Blitz) for Orphyn

Standalone cargo project (`native-spikes/dioxus`), `dioxus-native 0.8.0-alpha.3` (Blitz 0.3 beta: Stylo + Taffy + Vello hybrid, wgpu). No webview. Data is real: Jellyfin login via env vars (`ORPHYN_E2E_*`, optionally a git-ignored `.env`), a random album with artwork and tracks (`ORPHYN_ALBUM_ID` pins one), real audio playback.

## What was built

| Item | Result |
|---|---|
| 1. Jellyfin login + client (reqwest/serde) | Done (`src/jellyfin.rs`): auth, album, tracks, image and audio URLs. |
| 2. Album page | Done. The app's **real CSS, unmodified** (tokens, controls, lists, media, navigation, shell, music: 118 KB concatenated by `scripts/sync-css.sh`; the only rewrite is `html[..]` → `.jr-root[..]`). DOM mirrors `AlbumPage`/`DetailHeader`/`TrackRows`. Backdrop, header, buttons, hover rows, real images, dark theme, sidebar. |
| 3. Mini player + Now Playing sheet | Done, with defects below. Progress bar updates from the audio thread and seeks on click. Sheet slides in with the same spring constants as framer-motion's `springs.smooth` (300/30/1), integrated in a Rust loop because Blitz has no JS animation. |
| 4a. Audio | Done: rodio 0.21 + symphonia, track downloaded then decoded; play/pause/seek/volume/next/prev, auto-advance. |
| 4b. **Video (libmpv)** | **Done** (A2 spike): see "Video" below. Works on Windows and functionally on Linux, with a CPU-cost caveat. |
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

## Video (libmpv) in the Blitz window: works (software render, GPU texture composite)

Binary `src/bin/video` (`cargo build --release --bin video`; `src/bin/video/mpv.rs` is the libmpv binding, loaded at run time, adapted from the GPUI spike). Test: `ORPHYN_VIDEO_ID=<jellyfin item id> ORPHYN_SECS=60 ORPHYN_SEEK_TEST=1 ./target/release/video` (+ `ORPHYN_MPV_DIR` for the libmpv folder; Windows used the `libmpv-2.dll` shipped with Stremio, Debian uses `libmpv2` 0.40). Jellyfin direct play (`/Videos/{id}/stream?static=true`), 1080p HEVC, 24 fps.

**How (approach 1, as far as it can go):** Blitz exposes a `Widget` trait (`<object data=CustomWidgetAttr>`, blitz-dom `custom-widget`). The widget gets the renderer's wgpu `DeviceHandle` in `can_create_surfaces`, owns a `Bgra8Unorm` texture, registers it with `try_register_custom_resource`, and returns a Scene that fills the box with `Paint::Resource(id)`. mpv runs on its own thread through the **software render API** (`hwdec=auto-copy`, so decoding is hardware: `d3d11va-copy`), renders BGRA at the widget's current pixel size, and `queue.write_texture` uploads the newest frame. The video is therefore an ordinary Blitz box: the overlay (title, HUD, gradient scrim, play/pause, seek bar, time) is plain RSX/CSS painted **on top**, with real transparency (`screenshots/win-video.png`). Seek by clicking the bar works.
Why not GPU interop: libmpv's render API only has OpenGL and SW targets, and Blitz/vello-hybrid runs on wgpu (D3D12/Vulkan); there is no shared-texture path between them. Approach 2 (child HWND/X11 window) was not needed, and it cannot show Blitz controls above the video anyway.

Windows (AMD Radeon iGPU, Vulkan, 1080p HEVC, window 1280x720 logical = 2560x1440 physical texture, 60 s run):
| Measure | Result |
|---|---|
| Playback | steady **24 fps renders = 24 uploads/s** (source rate), hwdec `d3d11va-copy` |
| Dropped frames | 1 (vo) / 0 (decoder) over the 60 s run, including one seek |
| CPU | **about 3 cores busy** (3.0 to 3.3 cores for the process, sampled over 30 s; includes mpv hardware-copy decode, BGRA conversion and Blitz repaint) |
| GPU | roughly 45 % of the 3D engine and 10 % of the video engine (one Get-Counter sample, approximate) |
| Memory | about 700 MB working set |
| Seek latency | **about 250 ms** to the new position (click on the bar, 60 % target) |
| Resize / maximize | works: the texture is recreated for each size (800x500, 1500x900, 640x400, maximized) and fps stays at 24; frames were not inspected visually while resizing |
| Near-4K output | 3300x1860 px output: renders 23-25/s but uploads only 19-20/s (Blitz repaint cannot keep up), so visible drops; **3.3 cores** |
| 4K source | **not measured**: the Jellyfin test library has no 4K title and this libmpv build has no `lavfi` source |

Linux (WSL Debian, WSLg): builds and plays with the same code (`vo` frames render at 23-24/s, seek 250 ms, same overlay), **but wgpu there is llvmpipe (CPU Vulkan)**, so Blitz repaints only about 2 to 5 times per second and **5 cores** are busy (`drop(vo)` grows). No hwdec in WSL (`hwdec=no`, no VAAPI). So Linux "works functionally" and its speed cannot be judged here. On a real Linux machine it needs: a real Vulkan GPU driver (Mesa radv/anv or NVIDIA) for wgpu, `libmpv2`, and `hwdec=auto-copy` with `libva` (vaapi-copy) or NVDEC; the same code path applies. Not verified on real hardware.

Bug found on the way (portable trap): the closure capturing only some fields of the `Api` struct left the `libloading::Library` behind, dropped when `Player::start` returned and `dlclose`d libmpv, so mpv jumped into unmapped memory (segfault on Linux, silently survived on Windows). Fixed with `let api = api;` in the thread.

**Verdict on video: go, with a performance caveat.** Video plays inside the Blitz window with HTML/CSS controls on top, hardware decode, seek and resize working, on Windows and (functionally) Linux. It is not yet "smooth for free": the pipeline copies every frame (mpv converts to BGRA on the CPU, then CPU to GPU upload), costing about 3 CPU cores at 1080p output on this machine and failing at ~4K output. Fine for a 1080p desktop player; fixing 4K/HDR properly needs either (a) mpv built with a D3D11/Vulkan render target upstreamed or the GL path with a custom wgpu-GL interop (high effort), or (b) a pure-wgpu video pipeline (decode with `hwdec` to a texture, e.g. via `libplacebo`/own YUV shader, bypassing mpv rendering). Quick wins not yet tried: keep the NV12/YUV frame and convert in a shader (removes the CPU BGRA conversion and 2/3 of the upload size), render at video size and let Vello scale, and skip the alpha pass. Not tested: HDR/tone mapping, subtitles, audio sync under load, fullscreen via the window API (maximize works).

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

**Conditional go.** Video is now proven (see Video: plays inside the window with Blitz overlay controls, 1080p direct play + hardware decode, CPU-copy path costs about 3 cores).

For: the reuse promise holds: the 7.8k lines of CSS rendered a faithful page with essentially no CSS edits, and React components map to RSX 1:1 (this spike's pages are about 450 lines of RSX for what is about 700 lines of TSX plus design-system components). Windows and Linux both work from one code base, no webview.

Against / risks, in order:
1. **Video**: works via mpv sw render + wgpu texture, but the per-frame CPU copy is costly and 4K is not smooth; a YUV/hwdec to wgpu path is the follow-up (see Video).
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
