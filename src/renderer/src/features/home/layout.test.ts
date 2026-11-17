import { describe, expect, it } from 'vitest'
import {
  LAYOUT_VERSION,
  addRow,
  countOf,
  defaultLayout,
  layoutStorageKey,
  loadLayout,
  migrateLayout,
  moveRow,
  parseLayout,
  removeRow,
  reorderRows,
  saveLayout,
  setRowHidden,
  setRowOptions,
  syncLibraries,
  type HomeLayout,
  type RowIntroduction,
} from './layout'

const ids = (l: HomeLayout) => l.rows.map((r) => r.id)

function memoryStorage() {
  const data = new Map<string, string>()
  return {
    data,
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  }
}

describe('defaults', () => {
  it('video starts with the hero and the music home has no hero', () => {
    expect(defaultLayout('video').rows[0]?.type).toBe('hero')
    expect(defaultLayout('music').rows.some((r) => r.type === ('hero' as never))).toBe(false)
  })
})

describe('add', () => {
  it('appends a row, the hero goes first', () => {
    let l = removeRow(defaultLayout('video'), 'hero')
    l = addRow(l, 'video', { type: 'released' }, 'r1')
    expect(ids(l).at(-1)).toBe('r1')
    l = addRow(l, 'video', { type: 'hero' }, 'h2')
    expect(ids(l)[0]).toBe('h2')
  })

  it('refuses a second unique row, an exact duplicate and a missing parameter', () => {
    let l = defaultLayout('video')
    expect(addRow(l, 'video', { type: 'hero' }, 'x')).toBe(l)
    expect(addRow(l, 'video', { type: 'genre' }, 'x')).toBe(l)
    l = addRow(l, 'video', { type: 'genre', param: 'Drama', label: 'Drama' }, 'g1')
    expect(addRow(l, 'video', { type: 'genre', param: 'Drama' }, 'g2')).toBe(l)
    expect(addRow(l, 'video', { type: 'genre', param: 'Comedy' }, 'g3').rows).toHaveLength(
      l.rows.length + 1,
    )
  })

  it('refuses a type of the other scope', () => {
    const l = defaultLayout('music')
    expect(addRow(l, 'music', { type: 'hero' }, 'x')).toBe(l)
  })
})

describe('remove, hide, move', () => {
  it('removes a row', () => {
    expect(ids(removeRow(defaultLayout('video'), 'nextUp'))).not.toContain('nextUp')
  })

  it('hides and shows without changing the order', () => {
    const l = setRowHidden(defaultLayout('video'), 'continue', true)
    expect(l.rows.find((r) => r.id === 'continue')?.hidden).toBe(true)
    expect(ids(l)).toEqual(ids(defaultLayout('video')))
    expect(setRowHidden(l, 'continue', false).rows.every((r) => !r.hidden)).toBe(true)
  })

  it('moves a row and clamps the target', () => {
    const base = defaultLayout('video') // hero, continue, nextUp, favourites
    expect(ids(moveRow(base, 'favourites', 1))).toEqual([
      'hero',
      'favourites',
      'continue',
      'nextUp',
    ])
    expect(ids(moveRow(base, 'hero', 99)).at(-1)).toBe('hero')
    expect(moveRow(base, 'nope', 0)).toBe(base)
  })

  it('applies a dragged order and keeps rows missing from it', () => {
    const base = defaultLayout('video')
    expect(ids(reorderRows(base, ['nextUp', 'ghost', 'hero']))).toEqual([
      'nextUp',
      'hero',
      'continue',
      'favourites',
    ])
  })
})

describe('options', () => {
  it('sets, clamps and resets size and count', () => {
    let l = setRowOptions(defaultLayout('video'), 'continue', { size: 'large', count: 3 })
    const row = () => l.rows.find((r) => r.id === 'continue')
    expect(row()?.size).toBe('large')
    expect(row()?.count).toBe(5)
    expect(countOf(row()!, 'video')).toBe(5)
    l = setRowOptions(l, 'continue', { size: 'auto', count: undefined })
    expect(row()?.size).toBeUndefined()
    expect(row()?.count).toBeUndefined()
    expect(countOf(row()!, 'video')).toBe(20)
  })
})

describe('video libraries', () => {
  const libs = [
    { id: 'm', name: 'Movies' },
    { id: 's', name: 'Shows' },
  ]

  it('adds one Recently Added row per library, after Next Up', () => {
    const l = syncLibraries(defaultLayout('video'), libs)
    expect(ids(l)).toEqual(['hero', 'continue', 'nextUp', 'latest-m', 'latest-s', 'favourites'])
    expect(l.rows.find((r) => r.id === 'latest-m')).toMatchObject({ param: 'm', label: 'Movies' })
  })

  it('leaves a removed library row removed, and picks up a new library', () => {
    let l = removeRow(syncLibraries(defaultLayout('video'), libs), 'latest-s')
    expect(syncLibraries(l, libs)).toBe(l)
    l = syncLibraries(l, [...libs, { id: 'k', name: 'Kids' }])
    expect(ids(l)).toContain('latest-k')
    expect(ids(l)).not.toContain('latest-s')
  })
})

describe('reset', () => {
  it('a reset layout is the default one, libraries seeded again', () => {
    const edited = removeRow(syncLibraries(defaultLayout('video'), []), 'hero')
    const reset = syncLibraries(defaultLayout('video'), [{ id: 'm', name: 'Movies' }])
    expect(ids(edited)).not.toContain('hero')
    expect(ids(reset)).toEqual(['hero', 'continue', 'nextUp', 'latest-m', 'favourites'])
  })
})

describe('storage', () => {
  it('round-trips per server and user', () => {
    const storage = memoryStorage()
    const a = layoutStorageKey('video', 'srv1', 'u1')
    const b = layoutStorageKey('video', 'srv1', 'u2')
    expect(a).not.toBe(b)
    expect(a).not.toBe(layoutStorageKey('music', 'srv1', 'u1'))
    const edited = setRowHidden(defaultLayout('video'), 'nextUp', true)
    saveLayout(a, edited, storage)
    expect(loadLayout('video', a, storage)).toEqual(edited)
    expect(loadLayout('video', b, storage)).toEqual(defaultLayout('video'))
  })

  it('falls back to the defaults on garbage', () => {
    expect(parseLayout('video', null)).toEqual(defaultLayout('video'))
    expect(parseLayout('video', '{nope')).toEqual(defaultLayout('video'))
    expect(parseLayout('video', '{"version":1}')).toEqual(defaultLayout('video'))
  })

  it('drops unknown rows, other-scope rows, duplicates and invalid options', () => {
    const json = JSON.stringify({
      version: LAYOUT_VERSION,
      rows: [
        { id: 'a', type: 'continue', hidden: false, size: 'huge', count: 'x' },
        { id: 'a', type: 'nextUp' },
        { id: 'b', type: 'warp' },
        { id: 'c', type: 'recentlyPlayed' },
        { id: 'd', type: 'genre' },
        { id: 'e', type: 'continue' },
        { id: 'f', type: 'genre', param: 'Drama', label: 'Drama', size: 'small', count: 500 },
        42,
      ],
    })
    const l = parseLayout('video', json)
    expect(ids(l)).toEqual(['a', 'f'])
    expect(l.rows[0]).toEqual({ id: 'a', type: 'continue', hidden: false })
    expect(l.rows[1]).toMatchObject({ size: 'small', count: 60, param: 'Drama' })
  })
})

describe('migration', () => {
  const intros: RowIntroduction[] = [
    { version: 2, scope: 'video', row: { type: 'released', hidden: false }, after: 'nextUp' },
    { version: 2, scope: 'music', row: { type: 'randomAlbums', hidden: false } },
    { version: 3, scope: 'video', row: { type: 'random', hidden: true } },
  ]
  const stored = (): HomeLayout => ({
    version: 1,
    rows: defaultLayout('video').rows.filter((r) => r.id !== 'hero'),
  })

  it('adds the rows introduced after the stored version, where they belong', () => {
    const l = migrateLayout('video', stored(), intros, 3)
    expect(ids(l)).toEqual(['continue', 'nextUp', 'released', 'favourites', 'random'])
    expect(l.version).toBe(3)
    expect(l.rows.at(-1)?.hidden).toBe(true)
  })

  it('only adds the versions after the stored one, and only for its scope', () => {
    const l = migrateLayout('video', { ...stored(), version: 2 }, intros, 3)
    expect(ids(l)).not.toContain('released')
    expect(ids(l)).toContain('random')
    expect(ids(migrateLayout('music', { version: 1, rows: [] }, intros, 3))).toEqual([
      'randomAlbums',
    ])
  })

  it('does not duplicate a row the user already has, and is a no-op when up to date', () => {
    const l = migrateLayout(
      'video',
      { ...stored(), rows: [{ id: 'z', type: 'released', hidden: false }] },
      intros,
      3,
    )
    expect(l.rows.filter((r) => r.type === 'released')).toHaveLength(1)
    const current = defaultLayout('video')
    expect(migrateLayout('video', current, intros)).toBe(current)
  })

  it('keeps the order and hidden state of a stored layout through load', () => {
    const storage = memoryStorage()
    const edited = moveRow(setRowHidden(defaultLayout('music'), 'playlists', true), 'playlists', 0)
    saveLayout('k', edited, storage)
    const loaded = loadLayout('music', 'k', storage)
    expect(ids(loaded)).toEqual(ids(edited))
    expect(loaded.rows[0]?.hidden).toBe(true)
  })
})
