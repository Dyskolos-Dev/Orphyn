import { Download, Heart, Library, Music, Play, Plus, Shuffle, Square } from 'lucide-react'
import { useRef, useState, type ReactNode } from 'react'
import {
  ACCENT_PRESETS,
  Artwork,
  Button,
  ContextMenu,
  EmptyState,
  Grid,
  HeroBanner,
  IconButton,
  LandscapeCard,
  List,
  ListRow,
  Material,
  MenuButton,
  Popover,
  PosterCard,
  ProgressRing,
  RoundCard,
  SearchField,
  SegmentedControl,
  Shelf,
  Sheet,
  Skeleton,
  Slider,
  Spinner,
  SquareCard,
  Text,
  TextField,
  Toggle,
  Toolbar,
  toast,
  TrackList,
  TrackRow,
  useTheme,
  type AccentColor,
  type MenuItem,
  type ThemeMode,
} from '..'

/**
 * Story-like catalogue of every component, with placeholder content (no real data). Render it on a
 * scratch route while building a feature to see the components in both themes. Covered by a
 * smoke test (demo.test.tsx).
 */
export function DesignGallery() {
  const { mode, setMode, accent, setAccent } = useTheme()
  const [segment, setSegment] = useState<'all' | 'movies' | 'shows'>('all')
  const [toggle, setToggle] = useState(true)
  const [volume, setVolume] = useState(0.6)
  const [seek, setSeek] = useState(42)
  const [query, setQuery] = useState('')
  const [sheet, setSheet] = useState(false)
  const [popover, setPopover] = useState(false)
  const popoverAnchor = useRef<HTMLButtonElement>(null)
  const menu: MenuItem[] = [
    { label: 'Play Next', icon: <Play />, onSelect: () => toast({ title: 'Playing next' }) },
    {
      label: 'Add to Library',
      icon: <Plus />,
      onSelect: () => toast({ title: 'Added to Library' }),
    },
    { type: 'separator' },
    { label: 'Love', checked: true, onSelect: () => undefined },
  ]

  return (
    <div className="jr-gallery" style={{ display: 'grid', gap: 40, padding: '24px 0 64px' }}>
      <Demo title="Theme">
        <SegmentedControl<ThemeMode>
          aria-label="Appearance"
          value={mode}
          onChange={setMode}
          options={[
            { value: 'system', label: 'System' },
            { value: 'light', label: 'Light' },
            { value: 'dark', label: 'Dark' },
          ]}
        />
        <SegmentedControl<AccentColor>
          aria-label="Accent"
          size="small"
          value={accent}
          onChange={setAccent}
          options={ACCENT_PRESETS.map((a) => ({ value: a, label: a }))}
        />
      </Demo>

      <Demo title="Text">
        <Text variant="large-title">Large Title</Text>
        <Text variant="title-1">Title 1</Text>
        <Text variant="title-2">Title 2</Text>
        <Text variant="title-3">Title 3</Text>
        <Text variant="headline">Headline</Text>
        <Text>Body</Text>
        <Text variant="callout">Callout</Text>
        <Text variant="subheadline" color="secondary">
          Subheadline
        </Text>
        <Text variant="footnote" color="secondary">
          Footnote
        </Text>
        <Text variant="caption" color="tertiary">
          Caption
        </Text>
      </Demo>

      <Demo title="Button">
        <Button icon={<Play fill="currentColor" />}>Play</Button>
        <Button variant="tinted" icon={<Shuffle />}>
          Shuffle
        </Button>
        <Button variant="gray">Cancel</Button>
        <Button variant="plain">See All</Button>
        <Button size="small">Small</Button>
        <Button size="large" shape="capsule">
          Large capsule
        </Button>
        <Button loading>Saving</Button>
        <Button variant="filled" destructive>
          Delete
        </Button>
      </Demo>

      <Demo title="IconButton">
        <IconButton icon={<Heart />} label="Love" />
        <IconButton icon={<Shuffle />} label="Shuffle" pressed />
        <IconButton icon={<Plus />} label="Add" variant="gray" />
        <IconButton
          icon={<Play fill="currentColor" />}
          label="Play"
          variant="filled"
          size="large"
        />
        <MenuButton label="More" items={menu} variant="gray" />
      </Demo>

      <Demo title="SegmentedControl, Toggle, Slider">
        <SegmentedControl
          aria-label="Filter"
          value={segment}
          onChange={setSegment}
          options={[
            { value: 'all', label: 'All' },
            { value: 'movies', label: 'Movies' },
            { value: 'shows', label: 'TV Shows' },
          ]}
        />
        <Toggle
          checked={toggle}
          onChange={setToggle}
          label="Autoplay"
          description="Play the next episode"
        />
        <div style={{ width: 240 }}>
          <Slider aria-label="Volume" value={volume} onChange={setVolume} variant="regular" />
        </div>
        <div style={{ width: 320 }}>
          <Slider
            aria-label="Seek"
            value={seek}
            max={200}
            buffered={120}
            onChange={setSeek}
            formatValue={(v) => `${Math.round(v)} s`}
          />
        </div>
      </Demo>

      <Demo title="TextField, SearchField">
        <div style={{ width: 280 }}>
          <TextField
            label="Server address"
            placeholder="https://jellyfin.local:8096"
            hint="Include the port"
          />
        </div>
        <div style={{ width: 220 }}>
          <SearchField aria-label="Search" value={query} onChange={setQuery} />
        </div>
      </Demo>

      <Demo title="Toolbar">
        <div style={{ width: '100%' }}>
          <Toolbar title="Albums" trailing={<IconButton icon={<Plus />} label="Add" />} />
        </div>
      </Demo>

      <HeroBanner
        title="Placeholder Title"
        eyebrow="Up Next"
        meta="2025 · Drama · 2 h 10 min"
        description="A placeholder synopsis that shows how three lines of description sit over the backdrop gradient."
        actions={
          <>
            <Button variant="contrast" size="large" icon={<Play fill="currentColor" />}>
              Resume
            </Button>
            <IconButton icon={<Plus />} label="Add to Up Next" variant="glass" size="large" />
          </>
        }
        progress={0.35}
        progressCaption="1 h 24 min left"
      />

      <Shelf title="Posters" kind="poster" onSeeAll={() => undefined}>
        {Array.from({ length: 10 }, (_, i) => (
          <PosterCard
            key={i}
            title={`Movie ${i + 1}`}
            subtitle="2024"
            progress={i === 1 ? 0.6 : undefined}
          />
        ))}
      </Shelf>
      <Shelf title="Continue Watching" kind="landscape">
        {Array.from({ length: 6 }, (_, i) => (
          <LandscapeCard
            key={i}
            title={`Episode ${i + 1}`}
            subtitle="Series"
            caption="24 min left"
            progress={0.3}
          />
        ))}
      </Shelf>
      <Shelf title="Albums" kind="square">
        {Array.from({ length: 8 }, (_, i) => (
          <ContextMenu key={i} items={menu}>
            <SquareCard title={`Album ${i + 1}`} subtitle="Artist" onPlay={() => undefined} />
          </ContextMenu>
        ))}
      </Shelf>
      <Shelf title="Artists" kind="round">
        {Array.from({ length: 8 }, (_, i) => (
          <RoundCard key={i} title={`Artist Name ${i + 1}`} />
        ))}
      </Shelf>

      <Demo title="Grid">
        <div style={{ width: '100%' }}>
          <Grid kind="square">
            {Array.from({ length: 6 }, (_, i) => (
              <SquareCard key={i} title={`Album ${i + 1}`} subtitle="Artist" />
            ))}
          </Grid>
        </div>
      </Demo>

      <Demo title="TrackList, List">
        <div style={{ width: '100%' }}>
          <TrackList aria-label="Tracks">
            <TrackRow
              index={1}
              title="First Track"
              artist="Artist"
              duration={214}
              onPlay={() => undefined}
            />
            <TrackRow
              index={2}
              title="Playing Track"
              artist="Artist"
              duration={187}
              isCurrent
              isPlaying
              explicit
              onPlay={() => undefined}
              onMore={() => undefined}
            />
            <TrackRow
              index={3}
              title="Third Track"
              artist="Artist"
              duration={301}
              onPlay={() => undefined}
            />
          </TrackList>
        </div>
        <div style={{ width: 420 }}>
          <List variant="inset" header="Library">
            <ListRow
              leading={<Library />}
              title="Servers"
              subtitle="1 connected"
              onClick={() => undefined}
              chevron
            />
            <ListRow
              leading={<Music />}
              title="Lossless"
              trailing={<Toggle aria-label="Lossless" checked onChange={() => undefined} />}
            />
          </List>
        </div>
      </Demo>

      <Demo title="Sheet, Popover, Toast">
        <Button variant="gray" onClick={() => setSheet(true)}>
          Open sheet
        </Button>
        <Button variant="gray" ref={popoverAnchor} onClick={() => setPopover((o) => !o)}>
          Open popover
        </Button>
        <Button
          variant="gray"
          onClick={() => toast({ title: 'Added to Library', icon: <Heart /> })}
        >
          Show toast
        </Button>
        <Sheet
          open={sheet}
          onClose={() => setSheet(false)}
          title="Add a Server"
          description="Placeholder sheet content."
          footer={
            <>
              <Button variant="gray" onClick={() => setSheet(false)}>
                Cancel
              </Button>
              <Button onClick={() => setSheet(false)}>Connect</Button>
            </>
          }
        >
          <TextField label="Address" />
        </Sheet>
        <Popover
          open={popover}
          onClose={() => setPopover(false)}
          anchor={popoverAnchor}
          aria-label="Popover"
          width={260}
        >
          <div style={{ padding: 16 }}>
            <Text variant="headline">Popover</Text>
            <Text variant="footnote" color="secondary">
              On the thick material.
            </Text>
          </div>
        </Popover>
      </Demo>

      <Demo title="Feedback">
        <Spinner />
        <ProgressRing label="Downloading" value={0.4}>
          <Square fill="currentColor" />
        </ProgressRing>
        <ProgressRing label="Preparing" />
        <div style={{ width: 160 }}>
          <Skeleton aspectRatio="1 / 1" radius="var(--radius-artwork)" />
          <Skeleton shape="text" lines={2} />
        </div>
        <Material thickness="thin" bordered style={{ padding: 16, borderRadius: 12 }}>
          Thin material
        </Material>
        <div style={{ width: 120 }}>
          <Artwork src={null} fallback={<Download />} />
        </div>
      </Demo>

      <EmptyState
        icon={<Music />}
        title="No Music Yet"
        message="Placeholder empty state."
        action={<Button>Add a Server</Button>}
      />
    </div>
  )
}

function Demo({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} style={{ padding: '0 var(--page-gutter)' }}>
      <Text variant="title-3" as="h2" style={{ marginBottom: 12 }}>
        {title}
      </Text>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, alignItems: 'center' }}>
        {children}
      </div>
    </section>
  )
}
