# Orphyn design system

Orphyn is a modern, elegant, cinematic interface for movies, series and music, with native-feeling
behaviour on macOS and the same look on Windows and Linux. Features build their
pages **only** with the components below: import them from `@renderer/design`, and the page frame from
`@renderer/shell`. Do not write raw colours, font sizes or shadows in feature code: use the tokens.

- Tokens: `src/renderer/src/styles/tokens.css` (+ `typography.css`, `materials.css`, `base.css`).
- Components: `src/renderer/src/design/` (barrel `index.ts`), each with tests; a live catalogue of all of them
  is `DesignGallery` (`@renderer/design/demo/DesignGallery`), handy to mount on a scratch route.
- Shell: `src/renderer/src/shell/` (barrel `index.ts`).

## 1. Rules

### Colour

Colours follow a system palette, with light values on `:root` and dark values on
`[data-theme='dark']`. Any element with `data-theme="dark"` re-scopes the tokens for its subtree (the
HeroBanner does this, so everything placed on artwork is white-on-dark).

| Token                                                                    | Use                                                   |
| ------------------------------------------------------------------------ | ----------------------------------------------------- |
| `--system-background`, `--secondary-system-background`, `--tertiary-...` | System backgrounds                                    |
| `--surface-window`                                                       | Content area background (`#fff` / `#1d1d1f`)          |
| `--surface-sidebar`                                                      | Sidebar; `transparent` when the native material shows |
| `--surface-elevated`                                                     | Sheets, popovers' solid fallback                      |
| `--surface-hover` / `--surface-pressed` / `--surface-selected`           | Row and button states                                 |
| `--label`, `--secondary-label`                                           | Text; both reach AA (4.5:1) on every surface          |
| `--tertiary-label`, `--quaternary-label`                                 | Disabled text and glyphs only (not AA)                |
| `--separator`, `--opaque-separator`                                      | Hairlines (always `0.5px`)                            |
| `--system-fill` … `--quaternary-system-fill`                             | Neutral fills (gray buttons, tracks, fields)          |
| `--accent`, `--accent-contrast`, `--accent-tint`                         | Accent as text / icon / ring (AA); text on it; tint   |
| `--accent-fill`, `--accent-fill-contrast`                                | Filled controls (buttons, selection, switches)        |
| `--link`, `--danger`, `--danger-fill`, `--success`                       | Links, destructive text / buttons (AA), success       |
| `--system-red/pink/orange/yellow/green/teal/blue/indigo/purple/gray`     | Raw system colours (status dots, charts)              |

Accent: one of `ACCENT_PRESETS` (`red` default, `pink`, `orange`, `yellow`, `green`, `teal`, `blue`,
`indigo`, `purple`, `graphite`), a custom `#rrggbb`, or the colour of the artwork on screen ("Follow artwork
colour"). Whatever the source, `accentTokens(colour, theme)` (`design/theme/theme.ts`) derives the tokens per
theme, keeping the hue and moving only the lightness as far as WCAG AA needs:

- `--accent` reads at 4.5:1 as text on every surface of its theme (light: darker; dark: lighter), and
  `--accent-contrast` is the text colour on it;
- `--accent-fill` stands out from the window (3:1) and carries `--accent-fill-contrast` at 4.5:1: white on
  a fill dark enough for it, black on bright hues (yellow, a white custom colour).

The store writes both sets on `<html>` (`--accent-light`, `--accent-fill-dark`, ...) and `tokens.css` picks
the set of the current `[data-theme]`, so a subtree forced to dark gets the dark accent. Use `--accent` for
text, icons and rings, `--accent-fill` + `--accent-fill-contrast` for filled controls. Use the accent for
**the** primary action, icons in the sidebar, current track, toggles and selection; never for large areas.

### Typography

SF Pro (`-apple-system`) on macOS; **Inter Variable** everywhere else (bundled, no network); Segoe UI as a
last resort. Sizes are tuned for a desktop window (between desktop and mobile scales).

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
- Toolbar height `--toolbar-height` 52px, sidebar `--sidebar-width` 248px, now-playing bar 72px (transport left, "LCD" capsule with a thin progress line in the centre, volume / cast / lyrics / queue right).
- Radii: `--radius-artwork` 8px (albums), `--radius-poster` 10px (posters, 16:9 cards), `--radius-md` 8px
  (buttons, fields), `--radius-lg` 12px (inset lists, popovers), `--radius-xl` 16px (sheets, toasts),
  `--radius-full` (capsules, artists).
- These are Classic's values: styles change them, and the Appearance options scale them (corners scale
  every radius but `--radius-full`, text size the whole type ramp, density the gaps, row heights and card
  sizes). Always use the tokens, never the numbers, and things follow.

### Materials

Translucent surfaces blur what is **behind** them (`backdrop-filter: blur() saturate()`), like native
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

The Transparency option (0–100) scales every material: at 0 they are opaque and blur nothing
(`html[data-reduce-transparency]` also stops any other backdrop blur). A material never needs a blur of its
own inside another material (no blur on blur): use plain fills there.

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

Animate `opacity` and `transform` only. Reduced motion (the OS setting, or Settings > Appearance >
Motion, `html[data-motion='reduced']`) is honoured globally (CSS) and by framer-motion (`ThemeProvider`
sets `reducedMotion` to `"user"`, or `"always"` with the app's option).

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
When the user picks landscape video shelves (Settings > Appearance), a `poster` shelf lays out as
`landscape` and its `PosterCard`s become 16:9 cards showing `landscapeImage` (thumb / backdrop; pass it on
every `PosterCard` of a shelf), else the poster cropped to its top. Grids keep posters.
Songs are `TrackRow`s in a `TrackList`, settings are `ListRow`s in an inset `List`.

### Accessibility and keyboard

- Every interactive element is a real `button` / `a` / `input`, or has the right role (`switch`, `slider`,
  `radio`, `menuitem`) and keyboard support. Keyboard focus shows a solid 2px accent ring
  (`:focus-visible`, `--focus-ring`), offset 2px; fields show it inside their box.
- Contrast: body text is AA (4.5:1) on every surface and material of every style, in both themes, with any
  accent: `--label` and `--secondary-label` are tuned for it, accent text uses `--accent`, filled controls
  `--accent-fill` + `--accent-fill-contrast` (see Colour), links `--link`, destructive text `--danger`.
  `--tertiary-label` is for disabled content only. Cinema's ambient colour is tamed to a mid tone
  (`tameAmbientColor`) so its tinted surfaces keep the labels readable whatever the artwork. Tests in
  `theme.test.tsx` check every preset and a spread of custom colours against the surfaces.
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

const { mode, resolvedTheme, accent, followArtwork, setMode, setAccent, setFollowArtwork } =
  useTheme()
setMode('system' | 'light' | 'dark') // instant, persisted (localStorage "orphyn.theme.mode")
setAccent('blue' | '#30d158') // persisted ("orphyn.theme.accent")
setFollowArtwork(true) // persisted ("orphyn.theme.followArtwork"): the accent takes the artwork's colour
```

`ThemeProvider` is mounted by the shell; `useTheme()` works anywhere. The stored theme is applied before
React renders (no flash). In `system` mode the app follows the OS appearance live. The native window
(vibrancy, Mica, caption buttons) follows the chosen mode. With "Follow artwork colour", the shell gives
the dominant colour of the artwork on screen (else the now-playing artwork) to `themeStore.setArtworkColor`,
which stores it ("orphyn.theme.artworkColor") so the PiP windows follow too; the chosen accent stays the
fallback.

### Interface styles

The user picks one of five interface styles in **Settings > Appearance**, then tunes it. Each works in
light and dark and with every accent; switching is instant (a 220 ms cross-fade through View Transitions,
skipped when motion is reduced) and persisted.

| Style     | Character                                                                                                                                                                                                      |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `classic` | The base tokens, unchanged (default).                                                                                                                                                                          |
| `glass`   | Translucent materials everywhere, strong blur + saturation, specular top edges, rounder corners, sidebar / toolbar / now-playing bar floating off the window edges over a soft colour wash, liquid buttons.    |
| `cinema`  | The visible artwork tints the whole UI and glows, blurred, behind it (backdrop `immersive`); the sidebar is a rail of icons that expands over the content on hover or keyboard focus; bigger type and artwork. |
| `minimal` | Flat opaque surfaces, no blur and no shadow anywhere (always solid), hairlines, more whitespace, monochrome sidebar icons and primary buttons, quiet selection.                                                |
| `compact` | Smaller type scale, artwork, rows and controls (track rows 32 px, one line; 30 px buttons and fields), tighter shelves and grids, 208 px sidebar.                                                              |

### Appearance options

On top of the style, Settings > Appearance offers (with a live preview of the current style):

| Option           | Values                                   | On `<html>`                                                                         |
| ---------------- | ---------------------------------------- | ----------------------------------------------------------------------------------- |
| Transparency     | 0–100 (0 = solid, no blur)               | `--transparency` (0–1), `data-transparency`, `data-reduce-transparency` at 0        |
| Corners          | `sharp` / `standard` / `round`           | `--radius-scale` (0.35 / 1 / 1.45), `data-corners`                                  |
| Density          | `comfortable` / `standard` / `compact`   | `--density`, `--density-rows`, `--density-cards`, `--density-items`, `data-density` |
| Text size        | `s` / `m` / `l` / `xl`                   | `--text-scale` (0.92 / 1 / 1.1 / 1.22), `data-text-size`                            |
| Sidebar          | `expanded` / `icons` / `auto-hide`       | `data-sidebar`                                                                      |
| Artwork backdrop | `off` / `subtle` / `immersive`           | `data-backdrop`                                                                     |
| Motion           | `full` / `reduced` (the OS setting also) | `data-motion`                                                                       |
| Video shelves    | `poster` / `landscape`                   | `data-video-cards`                                                                  |

Each style has its own defaults (`STYLE_DEFAULTS` in `design/theme/appearance.ts`: Cinema uses the icon
rail and the immersive backdrop, Minimal is solid); the user's choices are stored as overrides
("orphyn.interface.appearance", JSON) and survive a change of style. "Reset to style defaults" drops them.
Density is independent of the style: Compact + comfortable is still denser than Classic + comfortable.
Everything lives on `<html>` as variables and data attributes, so the PiP windows, which re-apply the
stored theme and style on every `storage` event, inherit it all.

```tsx
import { useInterfaceStyle, useAppearance } from '@renderer/design'

const { style, appearance, overrides, setStyle, setAppearance, resetAppearance } =
  useInterfaceStyle()
setStyle('glass') // persisted ("orphyn.interface.style"), <html data-style="glass">
setAppearance({ corners: 'round', textSize: 'l' }) // instant, persisted, merged with the overrides
resetAppearance() // back to the current style's defaults
const { videoCards } = useAppearance() // the effective options only
```

`setReduceTransparency(on)` remains: it sets the transparency to 0 (or back). The 1.1 key
"orphyn.interface.reduceTransparency" is read once and migrated.

**Sidebar modes** (`shell/shell.css`): `icons` is a rail (`--sidebar-rail-width`, 60 px, 80 px on macOS for
the traffic lights) whose labels fade in when it expands over the content on hover or keyboard focus; the
search field becomes its magnifier. `auto-hide` slides the sidebar in when the pointer reaches the left
edge of the window or focus enters it (Cmd/Ctrl+F included); toolbars then make room for the traffic
lights. **Artwork backdrop**: `ShellAmbient` paints the visible artwork, blurred, behind the sidebar and the
content (`--backdrop-art-opacity`, per option and theme, under a veil of the window colour).

**How it works: token layers.** `tokens.css` splits the tokens in three:

- _Theme tokens_ (colours) on `:root` / `[data-theme='dark']`.
- _Style inputs_, which a style overrides: radii `--radius-*-base`, `--chrome-radius-base`; the type ramp
  numbers `--fs-*`, `--lh-*`, `--fw-*`; spacing `--section-gap-base`, `--shelf-gap-base`,
  `--grid-row-gap-base`, `--grid-column-gap-base`, `--card-scale-base`, `--shelf-density-base`,
  `--sidebar-item-height-base`, `--track-row-height-base`; the material recipe `--material-base`,
  `--material-{ultra-thin,thin,regular,thick}-opacity` / `-blur`, `--material-saturation`; and plain sizes
  (`--page-gutter`, `--hero-height`, `--sidebar-width`, `--sidebar-icon-size`, `--toolbar-height`,
  `--now-playing-height`, `--track-art-size`, `--chrome-inset`, `--material-highlight`), plus the tintable
  surfaces `--surface-window`, `--surface-sidebar`, `--surface-grouped`, `--shadow-artwork` (theme values
  are `--*-base`).
- _Style outputs_, what components read (`--radius-md`, `--text-body`, `--section-gap`, `--grid-gap`,
  `--material-thick-bg`, `--material-thick-filter`, ...), computed from the inputs and the appearance
  multipliers (`--radius-scale`, `--text-scale`, `--density*`, `--transparency`). They are declared on
  `:root, [data-style]` (materials also on `[data-theme]`), so they re-resolve wherever a style or a theme
  is forced.

`styles/interface-styles.css` holds one layer per style, `[data-style='x']` overriding inputs only (and
`[data-style='x'] [data-theme]` for values that depend on the theme, so a forced theme such as the HeroBanner
re-resolves them). Theme-dependent primitives a layer needs (`--glass-rgb`, `--cinema-base`, ...) sit on
`:root` / `[data-theme='dark']` at the top of that file. Because everything is redeclared on every
`[data-style]` element, any subtree can wear another style than the window: the Settings previews are live
miniatures laid out with the real tokens under their own `data-style` (and `data-sidebar`,
`data-backdrop`, `data-cards`).

What tokens cannot express is a component variant scoped to `html[data-style='x']` (so it never reaches a
preview): at the end of `interface-styles.css` for design components (glass lists, sheets, liquid buttons;
minimal tracks and monochrome buttons; compact one-line track rows and smaller controls; cinema's melting
hero), in `shell/shell.css` for the layout (floating glass chrome, sidebar modes, ambient layer). Features
never need to change: build pages with the tokens and components.

**Ambient artwork.** `ShellAmbient` (shell) uses `useAmbientArtwork(scrollRef, enabled, fallback)` from the
design system whenever Cinema, a backdrop or "Follow artwork colour" needs it: it watches the scroll
container (route changes, images loading, scrolling) for the largest visible `.jr-artwork__img` (a
`.jr-hero__backdrop` wins), else the now-playing bar's artwork, samples it on a 32×32 canvas
(`extractPalette`, cached per URL; `null` when the server forbids cross-origin reads, then only the blurred
image shows) and writes `--ambient-rgb` / `--ambient-rgb-2` on `<html>` (Cinema, tamed to a mid tone) and
the accent colour (follow artwork).

**Adding a style.**

1. Add its id to `INTERFACE_STYLES` (`design/theme/interfaceStyle.ts`) and its defaults to
   `STYLE_DEFAULTS` (`design/theme/appearance.ts`; add it to `SOLID_STYLES` if it is always opaque).
2. Add a `[data-style='id']` layer in `styles/interface-styles.css`: override style _inputs_ only (never an
   output such as `--radius-md` or `--text-body`, or the appearance options stop working); put
   theme-dependent values on `[data-style='id'], [data-style='id'] [data-theme]` with primitives on
   `:root` / `[data-theme='dark']`.
3. Only if tokens are not enough, add `html[data-style='id'] .jr-…` variants (design components at the end of
   `interface-styles.css`, shell layout in `shell.css`), and a `.jr-style-preview__window[data-style='id']`
   rule in `features/settings/StyleGallery.css` if the miniature needs one.
4. Add `appearance.styles.id.name` / `.description` to `settings.json` (en, fr).
5. Check it in light and dark, with transparency at 0 and 100, sharp and round corners, text XL, the three
   sidebar modes and the backdrop on; keep body text at AA on its surfaces.

### Adding an appearance option

Add the field and its values to `Appearance`, `BASE` and `CHOICES` in `appearance.ts`, map it in
`appearanceTokens` (a data attribute and / or a variable on `<html>`), consume it in CSS (a multiplier in the
_outputs_ of `tokens.css`, or an `html[data-…]` rule), add its row to `AppearancePane.tsx` and its strings to
`settings.json`, and extend the tests in `interfaceStyle.test.tsx`.

## 3. Window (`window.orphyn.window`)

```ts
import { useWindowState } from '@renderer/design'

const { platform, isFullscreen, isMaximized, isFocused, material, setFullscreen, toggleMaximize } =
  useWindowState()
await setFullscreen(true) // native fullscreen (video player); DOM fullscreen outside the app
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

Thin scrubber (`variant="scrubber"`, default) or `regular` with knob; `onMedia` for video.

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
