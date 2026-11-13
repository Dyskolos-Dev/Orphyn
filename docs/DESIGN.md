# Jellyra design system

Jellyra looks and behaves like **Apple TV** (movies, series) and **Apple Music** (music) on macOS, follows
Apple's Human Interface Guidelines, and keeps the same look on Windows and Linux. Features build their
pages **only** with the components below: import them from `@renderer/design`, and the page frame from
`@renderer/shell`. Do not write raw colours, font sizes or shadows in feature code: use the tokens.

- Tokens: `src/renderer/src/styles/tokens.css` (+ `typography.css`, `materials.css`, `base.css`).
- Components: `src/renderer/src/design/` (barrel `index.ts`), each with tests; a live catalogue of all of them
  is `DesignGallery` (`@renderer/design/demo/DesignGallery`), handy to mount on a scratch route.
- Shell: `src/renderer/src/shell/` (barrel `index.ts`).

## 1. Rules

### Colour

Colours follow Apple's system colours, with light values on `:root` and dark values on
`[data-theme='dark']`. Any element with `data-theme="dark"` re-scopes the tokens for its subtree (the
HeroBanner does this, so everything placed on artwork is white-on-dark).

| Token                                                                    | Use                                                        |
| ------------------------------------------------------------------------ | ---------------------------------------------------------- |
| `--system-background`, `--secondary-system-background`, `--tertiary-...` | Apple's backgrounds (iOS values)                           |
| `--surface-window`                                                       | Content area background (`#fff` / `#1d1d1f`)               |
| `--surface-sidebar`                                                      | Sidebar; `transparent` when the native material shows      |
| `--surface-elevated`                                                     | Sheets, popovers' solid fallback                           |
| `--surface-hover` / `--surface-pressed` / `--surface-selected`           | Row and button states                                      |
| `--label`, `--secondary-label`, `--tertiary-label`, `--quaternary-label` | Text, from primary to disabled                             |
| `--separator`, `--opaque-separator`                                      | Hairlines (always `0.5px`)                                 |
| `--system-fill` … `--quaternary-system-fill`                             | Neutral fills (gray buttons, tracks, fields)               |
| `--accent`, `--accent-contrast`, `--accent-tint`                         | Accent (default Apple Music red `#fa2d48`); text on accent |
| `--link` (systemBlue), `--danger` (systemRed), `--success`               | Links, destructive, success                                |
| `--system-red/pink/orange/yellow/green/teal/blue/indigo/purple/gray`     | Raw Apple colours (status dots, charts)                    |

Accent: one of `ACCENT_PRESETS` (`red` default, `pink`, `orange`, `yellow`, `green`, `teal`, `blue`,
`indigo`, `purple`, `graphite`) or a custom `#rrggbb` (text colour on it is computed). Use the accent for
**the** primary action, icons in the sidebar, current track, toggles and selection; never for large areas.

### Typography

SF Pro (`-apple-system`) on macOS; **Inter Variable** everywhere else (bundled, no network); Segoe UI as a
last resort. Sizes are tuned for a desktop window (between Apple's macOS and iOS scales).

| Style       | Token / class                               | Size / line / weight | Use                                   |
| ----------- | ------------------------------------------- | -------------------- | ------------------------------------- |
| Large Title | `--text-large-title` `.jr-text-large-title` | 32 / 38 bold         | Page title (ShellPage does it)        |
| Title 1     | `--text-title-1`                            | 26 / 32 bold         | Detail page title (album, movie)      |
| Title 2     | `--text-title-2`                            | 22 / 28 bold         | Shelf / section titles                |
| Title 3     | `--text-title-3`                            | 19 / 24 semibold     | Sheet titles, empty states            |
| Headline    | `--text-headline`                           | 15 / 20 semibold     | Emphasised body, toolbar title        |
| Body        | `--text-body`                               | 15 / 20              | Default text                          |
| Callout     | `--text-callout`                            | 14 / 19              | Sidebar items, buttons, descriptions  |
| Subheadline | `--text-subheadline`                        | 13 / 18              | Card titles and subtitles, track rows |
| Footnote    | `--text-footnote`                           | 12 / 16              | Metadata, section headings in sidebar |
| Caption     | `--text-caption`                            | 11 / 14              | Badges, tiny labels                   |

Use `<Text variant="title-2">` (or `font: var(--text-title-2)` in CSS). Large titles get negative tracking
(`--tracking-large-title`). Durations and counters use tabular figures (`<Text tabular>`, `.jr-tabular`).

### Spacing and layout

- **4 pt grid**: `--space-1` (4px) … `--space-20` (80px); half steps `--space-0-5` (2), `--space-1-5` (6),
  `--space-2-5` (10). Never use other values.
- **Page gutter** `--page-gutter` (40px): the left/right margin of titles, shelves and grids.
- Vertical rhythm: 32px (`--space-8`) between shelves/sections (ShellPage content does it), 12px between a
  shelf title and its items, 8px between artwork and its title.
- Toolbar height `--toolbar-height` 52px, sidebar `--sidebar-width` 248px, now-playing bar 64px.
- Radii: `--radius-artwork` 8px (albums), `--radius-poster` 10px (posters, 16:9 cards), `--radius-md` 8px
  (buttons, fields), `--radius-lg` 12px (inset lists, popovers), `--radius-xl` 16px (sheets, toasts),
  `--radius-full` (capsules, artists).

### Materials

Translucent surfaces blur what is **behind** them (`backdrop-filter: blur() saturate()`), like Apple's
materials. Use them only over content (toolbars over scrolled pages, popovers, HUDs, controls on artwork),
never over a flat colour.

| Material  | Class / component                                              | Blur | Use                                               |
| --------- | -------------------------------------------------------------- | ---- | ------------------------------------------------- |
| ultraThin | `.jr-material-ultra-thin`, `<Material thickness="ultra-thin">` | 16px | Badges on artwork                                 |
| thin      | `.jr-material-thin`                                            | 24px | Glass buttons on artwork                          |
| regular   | `.jr-material-regular`                                         | 30px | Shelf chevrons, floating panels                   |
| thick     | `.jr-material-thick`                                           | 40px | Toolbar, popovers, menus, toasts, now-playing bar |

The **sidebar** uses the native window material: macOS `vibrancy: 'sidebar'`, Windows 11 Mica. The
renderer knows it through `<html data-material="vibrancy|mica|none">`; with `none` (Linux, Windows 10,
browser) the sidebar is `--surface-sidebar`.

### Shadows and artwork

Shadows are subtle: `--shadow-artwork` under every piece of artwork, `--shadow-popover` for floating
surfaces, `--shadow-lg` for sheets. Artwork always has rounded corners, a 0.5px hairline and fades in
(`<Artwork>` does all of it).

### Motion

| Token                                   | Value                        | Use                                |
| --------------------------------------- | ---------------------------- | ---------------------------------- |
| `--duration-fast` / `durations.fast`    | 160ms                        | Hovers, opacity                    |
| `--duration-base`                       | 240ms                        | Toolbar collapse, small transforms |
| `--duration-slow`                       | 360ms                        | Artwork fade-in, segmented thumb   |
| `--ease-standard`, `--ease-out`         | cubic-bezier                 | Most transitions                   |
| `--ease-spring`, `--ease-spring-bouncy` | sampled springs (`linear()`) | Thumbs, knobs, presses             |
| `springs.smooth/snappy/bouncy`          | framer-motion springs        | Sheets, popovers, toasts           |

Animate `opacity` and `transform` only. "Reduce motion" is honoured globally (CSS) and by framer-motion
(`ThemeProvider` sets `reducedMotion="user"`).

### Which card?

| Content                                      | Component       | Ratio | Shelf / Grid `kind` |
| -------------------------------------------- | --------------- | ----- | ------------------- |
| Movie, series, season                        | `PosterCard`    | 2:3   | `poster`            |
| Episode, Continue Watching, trailer, chapter | `LandscapeCard` | 16:9  | `landscape`         |
| Album, playlist, music video thumbnail       | `SquareCard`    | 1:1   | `square`            |
| Artist, cast & crew                          | `RoundCard`     | 1:1 ○ | `round`             |
| Featured item on a home page                 | `HeroBanner`    | full  | —                   |

A shelf shows a fixed number of items for the window width (posters 4–8, landscape 2–5, squares 4–8,
artists 5–9) and pages by a full width with the chevrons. A library page uses `Grid` with the same `kind`.
Songs are `TrackRow`s in a `TrackList`, settings are `ListRow`s in an inset `List`.

### Accessibility and keyboard

- Every interactive element is a real `button` / `a` / `input`, or has the right role (`switch`, `slider`,
  `radio`, `menuitem`) and keyboard support. Keyboard focus shows the accent ring (`:focus-visible`).
- Icon-only buttons need `label` (IconButton enforces it). Decorative artwork has `alt=""`.
- Keys: SegmentedControl ←/→/Home/End; Slider ←/→ (step), PageUp/PageDown (10 %), Home/End; Shelf ←/→
  between items; TrackList ↑/↓ between rows, Enter plays / pauses; Menu ↑/↓/Home/End, letters jump,
  Enter/Space select, Esc closes; ContextMenu opens with right-click, Shift+F10 or the Menu key; Sheet
  traps Tab and restores focus on close; SearchField Esc clears then blurs; Cmd+F (macOS) / Ctrl+F focuses
  the sidebar search.
- Chrome text is not selectable (desktop app); wrap selectable text in `<Text selectable>`.

### Platforms

`<html>` carries `data-platform` (`darwin` / `win32` / `linux` / `web`), `data-material`,
`data-fullscreen`, `data-window-focused` (set by the shell from `useWindowState()`).

- **macOS**: hidden-inset title bar with traffic lights at (20, 20) inside the sidebar; vibrancy sidebar.
- **Windows 11**: hidden title bar + native caption buttons (title bar overlay, recoloured with the theme);
  Mica behind the sidebar. Toolbars reserve `--titlebar-inset-right` (140px) for the caption buttons.
- **Linux**: native frame, menu bar hidden; opaque sidebar.
- The window remembers its size, position, maximized and fullscreen state; minimum 1000×640.
- Draggable regions: the sidebar top and every page toolbar (`.jr-drag`; buttons, links and inputs inside
  stay clickable). Add `.jr-drag` to any other empty title area you create.

## 2. Theme

```tsx
import { useTheme } from '@renderer/design'

const { mode, resolvedTheme, accent, setMode, setAccent } = useTheme()
setMode('system' | 'light' | 'dark') // instant, persisted (localStorage "jellyra.theme.mode")
setAccent('blue' | '#30d158') // persisted ("jellyra.theme.accent")
```

`ThemeProvider` is mounted by the shell; `useTheme()` works anywhere. The stored theme is applied before
React renders (no flash). In `system` mode the app follows the OS appearance live. The native window
(vibrancy, Mica, caption buttons) follows the chosen mode.

## 3. Window (`window.jellyra.window`)

```ts
import { useWindowState } from '@renderer/design'

const { platform, isFullscreen, isMaximized, isFocused, material, setFullscreen, toggleMaximize } =
  useWindowState()
await setFullscreen(true) // native fullscreen (video player); DOM fullscreen outside Electron
```

Raw API (`src/shared/ipc/window.ts`): `minimize`, `toggleMaximize`, `close`, `getState`, `isFullscreen`,
`setFullscreen`, `setTitleBarOverlay`, `setAppearance`, `onStateChange(cb) => unsubscribe`.

## 4. App shell (`@renderer/shell`)

Layout: translucent sidebar (search, **Watch**: Home, Movies, TV Shows; **Music**: Home, Albums, Artists,
Playlists, Songs; bottom: server switcher slot + Settings) | content (page toolbar + large title + page) |
now-playing bar (only shown when something is plugged in).

Movies and TV Shows link to `/video/library/movies` and `/video/library/series`: the video feature treats
`movies` / `series` as aliases for "all movie / series libraries".

### Every page: `ShellPage`

```tsx
import { ShellPage } from '@renderer/shell'

<ShellPage
  title="Albums" // large title, collapses into the toolbar on scroll; also the window title
  subtitle="248 albums"
  headerAccessory={<SegmentedControl … />} // right of the large title
  actions={<MenuButton label="Sort" items={…} />} // toolbar, right side
  gutter // page margins for grids / lists (shelves bring their own)
>
  <Grid kind="square">…</Grid>
</ShellPage>

<ShellPage title="Home" header="hero"> {/* transparent toolbar over the hero */}
  <HeroBanner … />
  <Shelf …>…</Shelf>
</ShellPage>

<ShellPage title={album.name} header="inline"> {/* title always in the toolbar */}
```

A back button appears when there is history.

### Plugging in without editing shell files

Register at module scope in a file that your feature's `routes.tsx` imports:

```tsx
import { registerShellExtension } from '@renderer/shell'

registerShellExtension({
  id: 'music',
  provider: PlayerProvider, // wraps the whole shell (routes included)
  nowPlaying: MiniPlayer, // persistent bottom bar
  background: MediaKeys, // always-mounted, renders nothing
})
registerShellExtension({ id: 'servers', serverSwitcher: ServerSwitcher })
registerShellExtension({
  id: 'cast',
  toolbarItems: CastButton, // right of every page toolbar
  sidebarItems: [
    {
      id: 'cast',
      section: 'more',
      label: <Trans ns="cast" i18nKey="nav" />,
      icon: <Cast />,
      to: '/cast',
    },
  ],
})
```

Same `id` again replaces the previous registration (HMR-safe); the call returns an unregister function.
For content that should only show while a page is mounted, use the slot components instead:

```tsx
<ShellNowPlayingSlot><MiniPlayer /></ShellNowPlayingSlot>
<ShellServerSwitcherSlot><ServerSwitcher /></ShellServerSwitcherSlot>
```

Video player: `useShellImmersive()` (or `<ShellImmersive />`) hides the sidebar, toolbars and bar while
mounted; combine with `useWindowState().setFullscreen(true)`.

## 5. Component catalogue (`@renderer/design`)

All props are typed and documented in the source (JSDoc). Icons come from `lucide-react`.

### Text, Material

```tsx
<Text variant="title-2">Recently Added</Text>
<Text variant="footnote" color="secondary" truncate>2024 · 12 songs</Text>
<Material thickness="thick" bordered>…</Material>
```

### Button

`variant`: `filled` (one primary action), `tinted` (secondary), `gray`, `plain` (text), `contrast` (on
artwork). `size`: `small` 28 / `medium` 34 / `large` 44. `shape`: `rounded` | `capsule`. `icon`,
`trailingIcon`, `loading`, `destructive`, `fullWidth`.

```tsx
<Button icon={<Play fill="currentColor" />}>Play</Button>
<Button variant="tinted" icon={<Shuffle />}>Shuffle</Button>
<Button variant="contrast" size="large">Resume</Button>
```

### IconButton

`icon`, `label` (required), `variant` `plain` | `gray` | `filled` | `tinted` | `glass`, `size`, `pressed`.

```tsx
<IconButton icon={<Shuffle />} label="Shuffle" pressed={shuffle} onClick={toggleShuffle} />
```

### SegmentedControl

```tsx
<SegmentedControl
  aria-label="Filter"
  value={v}
  onChange={setV}
  options={[
    { value: 'all', label: 'All' },
    { value: 'unwatched', label: 'Unwatched' },
  ]}
/>
```

### Toggle

```tsx
<Toggle label="Autoplay" description="Play the next episode" checked={on} onChange={setOn} />
<Toggle aria-label="Lossless" checked={on} onChange={setOn} />
```

### Slider

Thin Apple scrubber (`variant="scrubber"`, default) or `regular` with knob; `onMedia` for video.

```tsx
<Slider aria-label="Seek" value={position} max={duration} buffered={buffered}
  onChangeStart={pauseUpdates} onChange={setPreview} onChangeEnd={seek} formatValue={formatDuration} />
<Slider aria-label="Volume" value={volume} onChange={setVolume} variant="regular" />
```

### TextField, SearchField

```tsx
<TextField label="Server address" hint="Include the port" error={error} value={url} onChange={…} />
<SearchField aria-label="Search albums" value={q} onChange={setQ} onSubmit={run} />
```

### Sidebar, SidebarSection, SidebarItem

Used by the shell; available for secondary navigation (e.g. settings panes).

```tsx
<Sidebar aria-label="Settings" header={…} footer={…}>
  <SidebarSection title="Music" collapsible>
    <SidebarItem icon={<Disc3 />} label="Albums" to="/music/albums" />
  </SidebarSection>
</Sidebar>
```

### Toolbar

Rendered by `ShellPage`; use directly only for a custom full-window page.

```tsx
<Toolbar title="Queue" leading={<IconButton … />} trailing={…} appearance="material" />
```

### Shelf

```tsx
<Shelf title="Continue Watching" kind="landscape" seeAllTo="/video/library/series">
  {items.map((i) => <LandscapeCard key={i.id} … />)}
</Shelf>
```

### Cards

Shared props: `title`, `subtitle`, `image`, `to` (link) or `onClick` (button), `onPlay`, `onMore`,
`onContextMenu`, `badge`, `fallback`, `selected`.

```tsx
<PosterCard title="Dune" subtitle="2021" image={url} to="/video/item/1" progress={0.4} badge="4K" />
<LandscapeCard title="S2 · E3" subtitle="Severance" caption="24 min left" progress={0.6} to="/video/item/9" />
<SquareCard title="Discovery" subtitle="Daft Punk" image={url} to="/music/album/7" onPlay={play} />
<RoundCard title="Daft Punk" image={url} to="/music/artist/3" />
```

Wrap a card in `<ContextMenu items={…}>` for a right-click menu.

### HeroBanner

```tsx
<HeroBanner
  backdrop={bd}
  logo={logo}
  title="Severance"
  eyebrow="Up Next"
  meta="2025 · Drama · TV-MA"
  description={overview}
  progress={0.3}
  progressCaption="31 min left"
  actions={
    <>
      <Button variant="contrast" size="large" icon={<Play fill="currentColor" />}>
        Resume
      </Button>
      <IconButton variant="glass" size="large" icon={<Plus />} label="Add to Up Next" />
    </>
  }
/>
```

### Grid

```tsx
<Grid kind="poster" aria-label="Movies">{movies.map((m) => <PosterCard key={m.id} … />)}</Grid>
```

### List, ListRow

```tsx
<List variant="inset" header="Playback">
  <ListRow title="Servers" subtitle="2 connected" to="/onboarding" />
  <ListRow title="Autoplay" trailing={<Toggle aria-label="Autoplay" … />} />
</List>
```

### TrackList, TrackRow

```tsx
<TrackList aria-label="Tracks">
  {tracks.map((t, i) => (
    <TrackRow
      key={t.id}
      index={i + 1}
      title={t.name}
      artist={t.artist}
      duration={t.seconds}
      isCurrent={t.id === current}
      isPlaying={playing}
      onPlay={() => play(t)}
      onPause={pause}
      onMore={(e) => openMenu(e, t)}
      explicit={t.explicit}
    />
  ))}
</TrackList>
```

`artwork={url}` replaces the number with a thumbnail (songs, playlists); `album` adds a column.
`formatDuration(seconds)` formats `3:07` / `1:02:03`.

### Sheet (Modal)

```tsx
<Sheet
  open={open}
  onClose={close}
  title="Add a Server"
  description="…"
  footer={
    <>
      <Button variant="gray" onClick={close}>
        Cancel
      </Button>
      <Button onClick={save}>Connect</Button>
    </>
  }
>
  <TextField label="Address" />
</Sheet>
```

### Popover

```tsx
const anchor = useRef<HTMLButtonElement>(null)
<IconButton ref={anchor} … onClick={() => setOpen(!open)} />
<Popover open={open} onClose={() => setOpen(false)} anchor={anchor} placement="bottom-end" aria-label="Queue">…</Popover>
```

### Menu, MenuButton, ContextMenu

```tsx
const items: MenuItem[] = [
  { label: 'Play Next', icon: <ListStart />, onSelect: playNext },
  { label: 'Love', checked: loved, onSelect: toggleLove },
  { type: 'separator' },
  { label: 'Delete from Library', destructive: true, onSelect: remove },
]
<MenuButton label="More" items={items} />
<ContextMenu items={items}><SquareCard … /></ContextMenu>
<Menu open anchor={{ x, y }} items={items} onClose={…} />
```

### Toast

```tsx
import { toast } from '@renderer/design'
toast({ title: 'Added to Library', icon: <Check />, action: { label: 'Undo', onClick: undo } })
toast({ title: 'Server unreachable', tone: 'error' })
```

`ToastViewport` is mounted by the shell.

### Feedback

```tsx
<Spinner size="large" />
<Skeleton aspectRatio="2 / 3" radius="var(--radius-poster)" /> <Skeleton shape="text" lines={2} />
<EmptyState icon={<Music />} title="No Albums" message="…" action={<Button>Add a Server</Button>} />
<ProgressRing label="Downloading" value={0.4}><Square fill="currentColor" /></ProgressRing>
<Artwork src={url} ratio="1/1" fallback={<Music />} />
```

### Hooks and helpers

`useTheme`, `useWindowState`, `getPlatform`, `useToast` / `toast` / `dismissToast`, `formatDuration`,
`initials`, `springs` / `durations` / `easings`.

## 6. Strings

Shell and shared component strings live in the `shell` namespace
(`src/renderer/src/i18n/locales/{en,fr}/shell.json`). Component labels you pass (titles, menu items) come
from your feature's own namespace.
