# Spike B: GPUI (Zed's UI framework) for a webview-free Orphyn

Short, honest report. Everything below was run on this machine (Windows 11, WSL Debian 13 + WSLg).
Server: the e2e test Jellyfin (credentials only via `ORPHYN_E2E_*` env vars, never committed).

## What was built (`native-spikes/gpui/`, standalone cargo project)

| Piece | State |
|---|---|
| Jellyfin client (login, album pick, tracks, artwork, stream URLs; `reqwest` blocking + `serde`) | works (`src/jellyfin.rs`) |
| Album page: sidebar, blurred artwork backdrop, header (232px art + shadow, title, artist, meta, Play / Shuffle / heart / more), 47 track rows with zebra + hover + play-glyph swap + current-track accent, footer | works (`src/main.rs`) |
| Mini player: transport, "LCD" capsule (art, title, subtitle, hover times, clickable progress bar), volume, icons | works; progress bar seeks |
| Now Playing sheet, opened by clicking the capsule: backdrop blobs, art, title, seek bar, transport; slide-up driven by the same spring as `design/motion.ts` (stiffness 300, damping 30) | works |
| Real audio (`rodio` + symphonia, whole file downloaded then decoded) | works on Windows and Linux; next-track on end, pause, seek |
| Video through **libmpv** inside the window, with a GPUI control bar painted over it (play/pause, seek, time, close) | works on Windows; **not working on Linux** (see below) |

Test hooks: `ORPHYN_AUTOPLAY`, `ORPHYN_OPEN_NP`, `ORPHYN_OPEN_VIDEO`, `ORPHYN_FPS` (overlay + stderr log), `ORPHYN_ALBUM_ID`, `ORPHYN_MPV_DIR` (folder holding `libmpv-2.dll`/`libmpv.so`).

Run (Windows): `cargo build --release`, then with the env vars set run `target/release/orphyn-gpui-spike.exe`.
`gen-icons.mjs` regenerates `assets/icons/*.svg` from the lucide data the app already uses.

## Screenshots (`screenshots/`)

- Reference (current app, from the site): `ref-album-dark.webp`, `ref-mini-player-dark.webp`
- GPUI Windows: `gpui-album-win.png` (a track playing, so the capsule and the accent row are live), `gpui-nowplaying-win.png`, `gpui-video-win.png`
- GPUI Linux (WSLg, X11): `gpui-nowplaying-linux.png`

Side by side, the album page is recognisably identical in layout, spacing, radii, colours, button shapes, zebra rows and lucide icons. The differences I can see: the reference uses the user's accent (a more saturated red for text; I used the theme fallback `#fc7b8c`, the real one comes from the accent store), the reference has a search field, Home Server footer and window buttons (not built), my row height is 52 (`.jrm-songs .jr-track`) vs. what the shot shows, and the backdrop is subtle on dark artwork. This is not a pixel diff, only a visual comparison.

## What could not be reproduced faithfully

- **`filter: blur()` / `backdrop-filter` (materials, sidebar/toolbar glass): not available in GPUI.** The artwork backdrop is a CPU pre-blur (64px downscale, saturate/brightness, gaussian blur, GPU upscales it) saved as a PNG. This looks right for a static image but cannot blur live content behind the toolbar/sidebar, so the "glass" interface style, the sticky toolbar blur and popover materials have no equivalent. They would need a custom blur shader, which means forking GPUI's renderer.
- **Rotating / transforming images and divs**: only SVG has `Transformation`; the Now Playing blobs drift (translate) but do not rotate like `jrm-drift-a/b`.
- **Animations**: no CSS transitions. A spring (closed form, in my code) drives the sheet; hover transitions are instant, `linear()` easings, keyframes and framer-motion's `AnimatePresence` have no equivalent (exit animations are hand-managed state).
- **Text**: GPUI shapes text itself (DirectWrite on Windows, cosmic-text on Linux). It looks very close, but no `font-variant-numeric: tabular-nums`, no letter-spacing tokens (`--tracking-*`) were used, `-webkit-line-clamp` / multi-line ellipsis is not built in (single-line `truncate` only), no text selection inside labels out of the box. The font stack (`SF Pro`/`Inter`/`Segoe UI Variable`) needs explicit family names per platform.
- **Shadows**: `BoxShadow` works (artwork shadow matches), but there is no inset shadow, so `box-shadow: inset 0 0 0 0.5px` (capsule hairline) is a 0.5px border instead.
- **Gradients**: only 2-stop linear gradients (enough for the veil and the video scrim); no radial gradient (`glass` wash), no `color-mix()`.
- **Everything cascade-based**: CSS variables, `:is(:hover, :focus-visible)`, `nth-child`, container queries (`@container` on the mini player), `content-visibility`, `html[data-style]` theming. All of it becomes Rust code (zebra = `i % 2`, hover = `.hover()`, group hover = `.group()`). Light/dark and the 4 interface styles would need a runtime token struct.
- Not built: scrolling-sticky header, keyboard navigation/focus rings, context menus/popovers, drag and drop, lyrics/queue views, i18n.

## Effort to restyle every CSS rule by hand

The two screens cost about 1,000 lines of Rust (blank lines included) for roughly 250 CSS rules read. The app has 7.8k lines of CSS, so a hand port is on the order of 25-30k lines of Rust styling, plus a component library (Button, IconButton, Slider, Menu/Popover, SegmentedControl, Toggle, TextField, virtualised lists) that GPUI does not ship in the crate (Zed's `gpui-component` exists as a third-party crate but would impose its own look). There is no way to reuse the CSS, so the "looks exactly like today" requirement is a manual, rule-by-rule translation with no automated way to detect drift. Layout is flexbox-like (taffy), so it ports predictably; `gap`, `flex`, `min-w-0`, `absolute` map 1:1.

## Video integration result

Windows: **works.** libmpv (`libmpv-2.dll`, loaded at run time with `libloading`, no link-time dependency) is driven through its **software render API** on a worker thread; each frame is rendered as BGRA into a 1280x720 buffer, wrapped in a GPUI `RenderImage` and shown by an `img` element. Because it is an ordinary GPUI element, the control bar (gradient scrim, seek bar, buttons) is painted over the video by GPUI, which is exactly what a native child window cannot do. Measured on this machine: about **45-48 renders/s** with a 720p stream playing. Costs: CPU conversion and upload of ~3.7 MB per frame, so 1080p/4K, HDR, hardware decode and a proper frame clock would need the real GPU path. GPUI's public API does not let you inject an external D3D11/Vulkan texture, so GL/D3D render-API interop would require forking GPUI (blade-graphics). The fallback is a child HWND/X11 window for mpv, but then GPUI cannot draw controls above it.

Linux: libmpv was loaded and the app started the video code path, but **no frame or error came out in 30 s (hang or silent failure in mpv init under WSLg); not diagnosed**, so Linux video is unverified.

## Linux behaviour (WSL Debian 13, WSLg)

- Builds after installing `libxkbcommon-dev libwayland-dev libxcb1-dev libfontconfig-dev libasound2-dev libmpv-dev libvulkan1 mesa-vulkan-drivers` etc. One build break fixed: `xattr 0.2.3` (transitive via GPUI) needs `libc::ENOATTR`, which current libc no longer exports on Linux; patched with a 1-line vendored copy (`vendor/xattr`, `[patch.crates-io]`).
- **Wayland (WSLg's compositor): panics** (`UnsupportedVersion` in gpui's wayland client). **X11 with the WSLg GPU Vulkan: panics** (`ERROR_SURFACE_LOST_KHR`). **X11 with software Vulkan (lavapipe, `VK_ICD_FILENAMES=.../lvp_icd.json`): works**, audio too. Real Linux desktops with a native GPU were not tested, so these two are likely WSLg limitations, not proof, but they show GPUI on Linux depends on Vulkan and a modern Wayland.
- Smoothness: with CPU Vulkan the Now Playing animation ran at **7-16 renders/s** (a slideshow); that is software rendering at 1100x680, not representative of a GPU. No GPU number on Linux.
- Font fell back to the installed Inter; no per-platform font tuning.

## Performance, size, build time

- Windows (D3D11): idle ~30 renders/s from my 33 ms UI clock (it re-renders unconditionally, so not a GPUI cost); Now Playing sheet animation ran at 30-48 renders/s on screen; page scroll felt smooth but I only measured by eye, no frame-time capture.
- Binary: **19.2 MB** `orphyn-gpui-spike.exe` (release, thin LTO) + `libmpv-2.dll` (123 MB in the build used here, shipped separately); an Electron app is far larger.
- Build: cold Windows build ~9 min (deps ~5.5 min, crate with thin LTO 3 min 46 s, then 2 min 18 s per rebuild after a one-line change). Linux 3 min 55 s on the machine's many cores after the first failed attempt (~8 min).
- 500+ crates, MSVC toolchain required on Windows.

## Verdict: conditional **no-go as the sole UI stack, today**

GPUI proved it can match the *static* look of Orphyn's dark theme very closely, with native speed and a 19 MB binary, and libmpv video with overlay controls is possible. But for a "looks and behaves EXACTLY like today" port of 32k lines of React + 7.8k lines of CSS the risks are large:

1. **No blur / backdrop-filter / radial gradients / inset shadows**: the Glass and Cinema interface styles and every material (toolbar, popovers, now-playing bar) cannot be done without patching GPUI's renderer.
2. **GPUI is a moving target**: the crates.io `0.2.2` snapshot has sparse docs, Zed-driven API churn, and needed a vendored fix for a dependency on Linux; there is no stable release policy.
3. **Linux is shaky**: Wayland and WSLg GPU paths both crashed; it needs working Vulkan.
4. **Video** works only through a CPU copy; real-time 1080p/4K + HDR would require engine-level work.
5. **No component library and no CSS reuse**: all of 7.8k CSS lines and ~30 components have to be rebuilt and kept consistent by hand; accessibility (screen readers), IME and text selection are weak or missing.

Go only if the person accepts: a downgrade of the Glass/Cinema styles, forking GPUI for blur and video textures, and a multi-month rewrite. The Dioxus Native (Blitz) spike should be compared on exactly these points, since it keeps real CSS.
