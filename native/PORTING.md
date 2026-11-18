# Porting a React screen to Orphyn native

The reference port is the music album page, mini player and Now Playing sheet
(`crates/orphyn-native/src/features/music`). Copy its patterns; keep every port consistent.

## Ground rules

1. **Same DOM, same classes.** Render the tags, classes, `data-*` and ARIA of the React component:
   the shared CSS styles it unchanged. Never restyle in Rust. A Blitz-only rendering fix goes in
   `native/styles/overrides.css` with the reason; a visual change goes in `src/renderer/src` so both
   apps get it.
2. **New stylesheet?** Add its path to `native/styles/manifest.txt` (React import order).
3. **New icon?** Import it in the React app, run `node native/scripts/gen-icons.mjs`; use it as
   `Icon { name: "disc-3" }` (React import name in kebab case).
4. **Strings** come from the React JSON: `let t = use_i18n(); t.t("music:album.notFound")`,
   `t.tf(key, &[("artist", name)])`, `t.tc(key, count, &[])` for `_one` / `_other` plurals.
5. **Routes**: replace the stub arm of your route in `features/mod.rs` (`RoutedPage`).

## React → Dioxus

| React | Here |
|---|---|
| `useState` | `use_signal(|| v)`; read `s()`, write `s.set(v)` |
| `useEffect(fn, deps)` | `use_effect(use_reactive!(|dep| { ... }))`; signals read inside are tracked |
| `useMemo` | `use_memo` |
| context / zustand store | a `Copy` struct of `Signal`s given with `use_context_provider`, read with `use_context` (see `MusicUi`, `AppState`) |
| `useSyncExternalStore` over a Rust engine | a `tokio::sync::watch` channel in the engine, turned into a signal by one task (`use_music_provider`) — no polling |
| `useNavigate` / `Link` | `use_router().push("/music/album/id")`, `.back()`, `.replace()` |
| `ShellPage` | `ShellPage { title, header: PageHeader::Inline, gutter: true, ... }`; register a page's own large title with `use_page().title_el` in `onmounted` |
| `<Text variant="title-1">` | the classes: `h1 { class: "jr-text-title-1 jr-color-primary" }` |
| design components | `crate::design` (`Button`, `IconButton`, `Slider`, `SegmentedControl`, `Artwork`, `TrackRow`, `Shelf`, `SquareCard`, `EmptyState`, `Spinner`, `Toolbar`); add the missing ones there, same DOM as React |

## Data: TanStack Query → `use_query`

```rust
let album = use_query(keys::item(&s.account_id, &id), stale::ITEM, {
    let (s, id) = (session.clone(), id.clone());
    move || { let (s, id) = (s.clone(), id.clone()); async move { s.item(&id).await } }
});
if album.is_loading() { return rsx! { Spinner {} } }
```

- Keys and stale times mirror `jellyfin/keys.ts` (`orphyn_jellyfin::keys`). Add an endpoint to
  `orphyn-jellyfin/src/endpoints.rs` next to its TypeScript twin, never `reqwest` in a screen.
- Mutations: update the cache optimistically (`cache.update` / `cache.set`), call the server,
  `cache.invalidate(prefix)` on failure (see `toggle_favorite`).
- Image URLs: `art(&base, item, size)` / `track_art` or `images::build_image_url` with
  `ImageOptions::square(css_px, scale, quality)`.

## Players

- Music: `use_services().music` (`MusicPlayer`) has the `musicPlayer` API (`play`, `play_next`,
  `add_to_queue`, `toggle`, `seek`, `set_volume`, `toggle_shuffle`, `cycle_repeat`...). Read state
  with `use_music()` (`state`, `progress` signals).
- Video: `use_services().video` (`VideoController`, API of `features/video/controller.ts`;
  `subscribe()` for the snapshot). Draw frames with
  `object { data: use_hook(|| CustomWidgetAttr::new(VideoWidget::new(video.mpv()?))) }` and put the
  controls as normal RSX on top.

## Motion: framer-motion / CSS transitions

Blitz runs CSS `@keyframes` animations but not transitions or framer-motion. Use `crate::motion`:
`let y = use_spring(if open { 0.0 } else { 100.0 }, springs::SMOOTH);` then
`style: "transform: translateY({y}%)"`; `use_tween(target, durations::BASE, easings::OUT)` for a
CSS transition. Unmount after the exit animation like `AnimatePresence` (see `NowPlaying`).

## Popovers, menus, overlays

Do not portal. `use_overlay().menu(x, y, vec![MenuItem::action(label, Some("icon"), move |_| ...)])`
opens a `Menu` in the single host rendered last by the shell; it closes on outside click, Escape
or selection. Sheets and full-window layers are rendered by the shell (`NowPlaying` pattern).

## Long lists (virtualisation)

The page scroller reports its offset in `use_router().scroll_y`. For a list longer than a few
hundred rows, render only the rows in view: measure the list top once (`get_client_rect`), use a
fixed row height from the CSS token (`--row-height` × density) and render
`(scroll_y - top) / row_h - overscan .. + visible + overscan` with top / bottom spacer elements.

## Blitz traps (also in the shared memory)

- `get_client_rect()` is 0×0 right after `onmounted`: measure at event time or retry a few frames.
- In `rsx!`, string literals inside `if/else` prop values become `String`: compute them in a `let`
  before `rsx!` for `&'static str` props.
- Controls stop `mousedown` propagation so they never start a window drag (`.jr-drag` regions).
- CSS custom properties that React sets as fractions (`--jr-slider-value`) must stay fractions.

## Checking a port

`scripts/dev-run.ps1 -Route /your/route -Out screenshots/x.png` (throwaway profile, test server
from `native/.env`), `scripts/click.ps1` to interact. Compare with the current app, then
`cargo test` and `cargo clippy --all-targets -- -D warnings`.
