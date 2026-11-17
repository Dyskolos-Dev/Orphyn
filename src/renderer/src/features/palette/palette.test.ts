import { Play } from 'lucide-react'
import { fuzzyMatch, highlightRuns, normalize, rank } from './fuzzy'
import { initialNavState, navReducer, type NavState } from './navReducer'
import { buildModel } from './model'
import { MAX_RECENT_COMMANDS, pushRecent } from './recents'
import {
  commandNames,
  createCommandRegistry,
  searchCommands,
  type Command,
  type PaletteContext,
  type Translate,
} from './registry'
import { BUILTIN_COMMANDS } from './commands'

describe('fuzzy matching', () => {
  it('ignores accents and case, mapping highlights onto the original text', () => {
    const m = fuzzyMatch('ele', 'Éléphant')
    expect(m?.indices).toEqual([0, 1, 2])
    expect(normalize('Éléphant')).toBe('elephant')
    expect(fuzzyMatch('EPH', 'Éléphant')).not.toBeNull()
  })

  it('returns null when a letter is missing or out of order', () => {
    expect(fuzzyMatch('xyz', 'settings')).toBeNull()
    expect(fuzzyMatch('gs', 'sg')).toBeNull()
  })

  it('ranks exact, prefix, word-start then scattered matches', () => {
    const names = ['Open Settings', 'Settings', 'Set up the server in tiny gaps', 'Reset']
    const ranked = rank(names, 'settings', (n) => [n]).map((r) => r.item)
    expect(ranked[0]).toBe('Settings')
    expect(ranked[1]).toBe('Open Settings')
    expect(ranked).not.toContain('Reset')
  })

  it('prefers word starts for initials', () => {
    const ranked = rank(['Go to Music', 'Mute', 'Remix'], 'gtm', (n) => [n])
    expect(ranked[0]?.item).toBe('Go to Music')
  })

  it('matches an alias without highlighting the title', () => {
    const [hit] = rank([{ fr: 'Réglages', en: 'Settings' }], 'sett', (c) => [c.fr, c.en])
    expect(hit).toBeDefined()
    expect(hit?.indices).toEqual([])
  })

  it('keeps input order for equal scores', () => {
    const ranked = rank(['abc', 'abc'], 'abc', (n) => [n]).map((r) => r.order)
    expect(ranked).toEqual([0, 1])
  })

  it('splits highlight runs', () => {
    expect(highlightRuns('Settings', [0, 1])).toEqual([
      { text: 'Se', match: true },
      { text: 'ttings', match: false },
    ])
    expect(highlightRuns('abc', [])).toEqual([{ text: 'abc', match: false }])
  })
})

const NAMES: Record<string, Record<string, string>> = {
  en: {
    'commands.go-settings': 'Open Settings',
    'commands.go-home': 'Go to Home',
    'commands.play': 'Play',
    'commands.pause': 'Pause',
  },
  fr: {
    'commands.go-settings': 'Ouvrir les réglages',
    'commands.go-home': 'Aller à l’accueil',
    'commands.play': 'Lecture',
    'commands.pause': 'Pause',
  },
}
const translate: Translate = (lang, key) => NAMES[lang]?.[key] ?? ''

function makeCtx(over: Partial<PaletteContext> = {}): PaletteContext {
  return {
    navigate: vi.fn(),
    player: {
      hasTrack: true,
      playing: false,
      shuffle: false,
      repeat: 'off',
      toggle: vi.fn(),
      next: vi.fn(),
      previous: vi.fn(),
      toggleShuffle: vi.fn(),
      cycleRepeat: vi.fn(),
    },
    favorite: { available: true, value: false, toggle: vi.fn() },
    openNowPlaying: vi.fn(),
    addToPlaylist: vi.fn(),
    theme: { resolved: 'light', setMode: vi.fn() },
    interfaceStyle: { current: 'classic', styles: ['classic', 'glass', 'cinema'], set: vi.fn() },
    accounts: [
      { id: 'a', name: 'Home server', active: true },
      { id: 'b', name: 'Office', active: false },
    ],
    switchServer: vi.fn(),
    signedIn: true,
    signOut: vi.fn(),
    refreshLibrary: vi.fn(),
    ...over,
  }
}

const cmd = (id: string, over: Partial<Command> = {}): Command => ({
  id,
  group: 'actions',
  icon: Play,
  run: vi.fn(),
  ...over,
})

describe('command registry', () => {
  it('lists commands in order and replaces an id registered twice', () => {
    const registry = createCommandRegistry()
    const first = cmd('a')
    registry.register(first, cmd('b'))
    const replacement = cmd('a', { group: 'navigate' })
    registry.register(replacement)
    const list = registry.list(makeCtx())
    expect(list.map((c) => c.id)).toEqual(['b', 'a'])
    expect(list[1]).toBe(replacement)
  })

  it('hides unavailable commands and expands providers', () => {
    const registry = createCommandRegistry()
    registry.register(cmd('needs-track', { available: (c) => c.player.hasTrack }), (c) =>
      c.accounts.map((a) => cmd(`s-${a.id}`)),
    )
    expect(registry.list(makeCtx()).map((c) => c.id)).toEqual(['needs-track', 's-a', 's-b'])
    expect(
      registry.list(makeCtx({ player: { ...makeCtx().player, hasTrack: false } })).map((c) => c.id),
    ).toEqual(['s-a', 's-b'])
  })

  it('removes what a registration added and notifies listeners', () => {
    const registry = createCommandRegistry()
    const listener = vi.fn()
    registry.subscribe(listener)
    const off = registry.register(cmd('x'))
    off()
    expect(registry.list(makeCtx())).toEqual([])
    expect(listener).toHaveBeenCalledTimes(2)
  })

  it('built-ins: playback commands need a track, other servers are listed, current style is not', () => {
    const registry = createCommandRegistry()
    registry.register(...BUILTIN_COMMANDS)
    const ids = registry.list(makeCtx()).map((c) => c.id)
    expect(ids).toContain('play-pause')
    expect(ids).toContain('server-b')
    expect(ids).not.toContain('server-a')
    expect(ids).toContain('style-glass')
    expect(ids).not.toContain('style-classic')
    const idle = registry
      .list(
        makeCtx({
          player: { ...makeCtx().player, hasTrack: false },
          favorite: { available: false, value: false, toggle: vi.fn() },
        }),
      )
      .map((c) => c.id)
    expect(idle).not.toContain('play-pause')
    expect(idle).not.toContain('favorite')
    expect(idle).toContain('go-settings')
  })

  it('names follow the state and carry both languages', () => {
    const registry = createCommandRegistry()
    registry.register(...BUILTIN_COMMANDS)
    const playing = makeCtx({ player: { ...makeCtx().player, playing: true } })
    const play = registry.list(playing).find((c) => c.id === 'play-pause') as Command
    expect(commandNames(play, playing, 'en', translate).title).toBe('Pause')
    const idle = makeCtx()
    expect(commandNames(play, idle, 'fr', translate)).toMatchObject({
      title: 'Lecture',
      aliases: expect.arrayContaining(['Play']),
    })
  })

  it('finds a command by its French or English name, accent-insensitive', () => {
    const registry = createCommandRegistry()
    registry.register(...BUILTIN_COMMANDS)
    const ctx = makeCtx()
    const commands = registry.list(ctx)
    const ids = (q: string, lang: 'fr' | 'en') =>
      searchCommands(commands, ctx, q, { lang, translate }).map((m) => m.command.id)
    expect(ids('reglages', 'fr')[0]).toBe('go-settings')
    expect(ids('settings', 'fr')[0]).toBe('go-settings')
    expect(ids('réglages', 'en')[0]).toBe('go-settings')
    expect(ids('accueil', 'en')).toContain('go-home')
  })

  it('lists recently used commands first on an empty query', () => {
    const ctx = makeCtx()
    const commands = createCommandRegistry()
    commands.register(cmd('one'), cmd('two'), cmd('three'))
    const out = searchCommands(commands.list(ctx), ctx, '', {
      lang: 'en',
      translate,
      recentIds: ['three', 'one'],
    }).map((m) => m.command.id)
    expect(out).toEqual(['three', 'one', 'two'])
  })

  it('keeps recents unique, newest first and bounded', () => {
    expect(pushRecent(['a', 'b', 'c'], 'b')).toEqual(['b', 'a', 'c'])
    const many = Array.from({ length: 20 }, (_, i) => `c${i}`).reduce<string[]>(
      (acc, id) => pushRecent(acc, id),
      [],
    )
    expect(many).toHaveLength(MAX_RECENT_COMMANDS)
    expect(many[0]).toBe('c19')
  })
})

describe('palette model', () => {
  const ctx = makeCtx()
  const registry = createCommandRegistry()
  registry.register(...BUILTIN_COMMANDS)
  const commands = registry.list(ctx)

  it('empty query: recents section first, then groups without duplicates', () => {
    const matches = searchCommands(commands, ctx, '', {
      lang: 'en',
      translate,
      recentIds: ['go-home'],
    })
    const model = buildModel({
      query: '',
      commands: matches,
      recentIds: ['go-home'],
      library: [],
      highlight: () => [],
    })
    expect(model.sections[0]?.id).toBe('recent')
    expect(model.sections.map((s) => s.id)).toContain('group:navigate')
    const keys = model.flat.map((e) => e.key)
    expect(new Set(keys).size).toBe(keys.length)
    expect(model.groupStarts[0]).toBe(0)
    expect(model.groupStarts[1]).toBe(model.sections[0]?.entries.length)
  })

  it('query: library results lead unless a command matches clearly', () => {
    const library = [
      { id: 'albums', kind: 'album' as const, items: [{ Id: '1', Name: 'Pause Album' }] },
    ]
    const weak = buildModel({
      query: 'pau',
      commands: [{ command: cmd('x'), title: 'x', indices: [], score: 20 }],
      recentIds: [],
      library,
      highlight: () => [],
    })
    expect(weak.sections.map((s) => s.id)).toEqual(['library:albums', 'commands'])
    const strong = buildModel({
      query: 'pau',
      commands: [{ command: cmd('x'), title: 'x', indices: [], score: 150 }],
      recentIds: [],
      library,
      highlight: () => [],
    })
    expect(strong.sections.map((s) => s.id)).toEqual(['commands', 'library:albums'])
  })
})

describe('keyboard navigation reducer', () => {
  const at = (index: number): NavState => ({ ...initialNavState, index })

  it('moves and wraps in both directions', () => {
    expect(navReducer(at(0), { type: 'move', delta: 1, count: 3 }).index).toBe(1)
    expect(navReducer(at(2), { type: 'move', delta: 1, count: 3 }).index).toBe(0)
    expect(navReducer(at(0), { type: 'move', delta: -1, count: 3 }).index).toBe(2)
    expect(navReducer(at(0), { type: 'move', delta: 1, count: 0 }).index).toBe(0)
  })

  it('resets the highlight when the query changes, not when it is unchanged', () => {
    const s = navReducer(at(4), { type: 'setQuery', query: 'abc' })
    expect(s).toMatchObject({ query: 'abc', index: 0 })
    const same = navReducer(s, { type: 'move', delta: 1, count: 5 })
    expect(navReducer(same, { type: 'setQuery', query: 'abc' })).toBe(same)
  })

  it('Tab cycles through groups and wraps, Shift+Tab goes back', () => {
    const groupStarts = [0, 3, 7]
    const a = navReducer(at(1), { type: 'cycleGroup', direction: 1, groupStarts })
    expect(a.index).toBe(3)
    const b = navReducer(at(5), { type: 'cycleGroup', direction: 1, groupStarts })
    expect(b.index).toBe(7)
    expect(navReducer(at(8), { type: 'cycleGroup', direction: 1, groupStarts }).index).toBe(0)
    expect(navReducer(at(4), { type: 'cycleGroup', direction: -1, groupStarts }).index).toBe(0)
    expect(navReducer(at(0), { type: 'cycleGroup', direction: -1, groupStarts }).index).toBe(7)
    expect(navReducer(at(2), { type: 'cycleGroup', direction: 1, groupStarts: [] }).index).toBe(2)
  })

  it('hover follows the pointer without asking the list to scroll', () => {
    const s = navReducer(at(0), { type: 'hover', index: 2 })
    expect(s).toMatchObject({ index: 2, source: 'pointer' })
    expect(navReducer(s, { type: 'hover', index: 2 })).toBe(s)
    expect(navReducer(s, { type: 'move', delta: 1, count: 5 }).source).toBe('keyboard')
  })

  it('clamps when the list shrinks and jumps to the edges', () => {
    expect(navReducer(at(6), { type: 'clamp', count: 3 }).index).toBe(2)
    expect(navReducer(at(1), { type: 'clamp', count: 3 }).index).toBe(1)
    expect(navReducer(at(6), { type: 'clamp', count: 0 }).index).toBe(0)
    expect(navReducer(at(1), { type: 'edge', to: 'last', count: 4 }).index).toBe(3)
    expect(navReducer(at(3), { type: 'edge', to: 'first', count: 4 }).index).toBe(0)
    expect(navReducer(at(3), { type: 'reset' })).toEqual(initialNavState)
  })
})
