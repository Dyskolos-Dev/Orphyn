/**
 * Home screen layouts: which rows a home shows, in which order, hidden or not, with which options.
 * Everything here is pure (no React, no storage but the one passed in) so it is easy to test.
 */

export type HomeScope = 'video' | 'music'

export type VideoRowType =
  | 'hero'
  | 'continue'
  | 'nextUp'
  | 'latest'
  | 'favourites'
  | 'genre'
  | 'collection'
  | 'because'
  | 'released'
  | 'random'

export type MusicRowType =
  | 'recentlyPlayed'
  | 'recentlyAdded'
  | 'favAlbums'
  | 'favArtists'
  | 'favSongs'
  | 'playlists'
  | 'instantMix'
  | 'genre'
  | 'artist'
  | 'randomAlbums'

export type RowType = VideoRowType | MusicRowType

/** What a row needs the user to pick when it is added. */
export type RowParam = 'library' | 'genre' | 'collection' | 'artist'

export interface RowDef {
  type: RowType
  /** At most one such row per home. */
  unique?: boolean
  param?: RowParam
  /** Default number of items. */
  count: number
  /** Whether the row has a "card size" option (the hero has none). */
  sized: boolean
}

export const ROW_DEFS: Record<HomeScope, readonly RowDef[]> = {
  video: [
    { type: 'hero', unique: true, count: 8, sized: false },
    { type: 'continue', unique: true, count: 20, sized: true },
    { type: 'nextUp', unique: true, count: 20, sized: true },
    { type: 'latest', param: 'library', count: 20, sized: true },
    { type: 'favourites', unique: true, count: 30, sized: true },
    { type: 'genre', param: 'genre', count: 20, sized: true },
    { type: 'collection', param: 'collection', count: 20, sized: true },
    { type: 'because', unique: true, count: 20, sized: true },
    { type: 'released', unique: true, count: 20, sized: true },
    { type: 'random', unique: true, count: 20, sized: true },
  ],
  music: [
    { type: 'recentlyPlayed', unique: true, count: 20, sized: true },
    { type: 'recentlyAdded', unique: true, count: 20, sized: true },
    { type: 'favAlbums', unique: true, count: 20, sized: true },
    { type: 'favArtists', unique: true, count: 20, sized: true },
    { type: 'favSongs', unique: true, count: 20, sized: true },
    { type: 'playlists', unique: true, count: 20, sized: true },
    { type: 'instantMix', unique: true, count: 20, sized: true },
    { type: 'genre', param: 'genre', count: 20, sized: true },
    { type: 'artist', param: 'artist', count: 20, sized: true },
    { type: 'randomAlbums', unique: true, count: 20, sized: true },
  ],
}

/** Choices offered for the number of items of a row. */
export const COUNT_OPTIONS = [10, 20, 30, 40] as const
const HERO_COUNT_OPTIONS = [5, 8, 10] as const

export const countChoices = (type: RowType): readonly number[] =>
  type === 'hero' ? HERO_COUNT_OPTIONS : COUNT_OPTIONS

/** Card size of a row: `auto` follows Settings > Appearance, the others override it. */
export const SIZE_OPTIONS = ['auto', 'small', 'large'] as const
export type RowSize = (typeof SIZE_OPTIONS)[number]

export interface HomeRow {
  /** Stable identity of the row (default rows use their type; added ones a generated id). */
  id: string
  type: RowType
  hidden: boolean
  /** Library / genre / collection / artist the row is about (an id, or the genre name). */
  param?: string
  /** Display name of `param`, kept so the row can be titled without loading anything. */
  label?: string
  size?: RowSize
  count?: number
}

export interface HomeLayout {
  version: number
  rows: HomeRow[]
  /** Video: libraries whose "Recently Added" row was already created, so a removed one stays removed. */
  knownLibraries?: string[]
}

/** Bump when a release brings new default rows, and describe them in `INTRODUCTIONS`. */
export const LAYOUT_VERSION = 1

export const defOf = (scope: HomeScope, type: string): RowDef | undefined =>
  ROW_DEFS[scope].find((d) => d.type === type)

/** A row that a later layout version adds to the layouts stored by older ones. */
export interface RowIntroduction {
  /** Layout version that brings the row (older stored layouts get it). */
  version: number
  scope: HomeScope
  row: Omit<HomeRow, 'id'> & { id?: string }
  /** Placed after the last row of this type; at the end when there is none. */
  after?: RowType
}

/** New default rows per version. Version 1 is the first one: nothing to add yet. */
export const INTRODUCTIONS: readonly RowIntroduction[] = []

const defaultRow = (type: RowType): HomeRow => ({ id: type, type, hidden: false })

export function defaultLayout(scope: HomeScope): HomeLayout {
  const types: RowType[] =
    scope === 'video'
      ? ['hero', 'continue', 'nextUp', 'favourites']
      : ['recentlyPlayed', 'recentlyAdded', 'instantMix', 'favAlbums', 'favArtists', 'playlists']
  return { version: LAYOUT_VERSION, rows: types.map(defaultRow), knownLibraries: [] }
}

// ---------------------------------------------------------------- validation & migration

const isRecord = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === 'object'

/** Keeps the valid rows of a stored object; anything unusable is dropped (never throws). */
export function sanitizeLayout(scope: HomeScope, raw: unknown): HomeLayout | null {
  if (!isRecord(raw) || !Array.isArray(raw['rows'])) return null
  const rows: HomeRow[] = []
  const ids = new Set<string>()
  const uniques = new Set<string>()
  for (const r of raw['rows'] as unknown[]) {
    if (!isRecord(r) || typeof r['id'] !== 'string' || typeof r['type'] !== 'string') continue
    const def = defOf(scope, r['type'])
    if (!def || ids.has(r['id'])) continue
    if (def.unique) {
      if (uniques.has(def.type)) continue
      uniques.add(def.type)
    }
    const param = typeof r['param'] === 'string' && r['param'] ? r['param'] : undefined
    if (def.param && !param) continue
    const row: HomeRow = { id: r['id'], type: def.type, hidden: r['hidden'] === true }
    if (def.param) {
      row.param = param
      if (typeof r['label'] === 'string') row.label = r['label']
    }
    if (
      def.sized &&
      (SIZE_OPTIONS as readonly unknown[]).includes(r['size']) &&
      r['size'] !== 'auto'
    )
      row.size = r['size'] as RowSize
    if (typeof r['count'] === 'number' && Number.isFinite(r['count']))
      row.count = clampCount(r['count'])
    ids.add(row.id)
    rows.push(row)
  }
  const version = typeof raw['version'] === 'number' ? raw['version'] : 0
  const known = Array.isArray(raw['knownLibraries'])
    ? (raw['knownLibraries'] as unknown[]).filter((x): x is string => typeof x === 'string')
    : []
  return { version, rows, knownLibraries: known }
}

const clampCount = (n: number) => Math.min(60, Math.max(5, Math.round(n)))

/**
 * Brings a stored layout up to `LAYOUT_VERSION`: rows introduced by later versions are added
 * (hidden rows the user deleted before the row existed cannot be mistaken for it, since the row is
 * new), the user's own order and choices are kept.
 */
export function migrateLayout(
  scope: HomeScope,
  layout: HomeLayout,
  introductions: readonly RowIntroduction[] = INTRODUCTIONS,
  target: number = LAYOUT_VERSION,
): HomeLayout {
  if (layout.version >= target) return layout
  const rows = [...layout.rows]
  for (const intro of [...introductions].sort((a, b) => a.version - b.version)) {
    if (intro.scope !== scope || intro.version <= layout.version || intro.version > target) continue
    const row: HomeRow = { ...intro.row, id: intro.row.id ?? intro.row.type }
    if (rows.some((r) => r.id === row.id)) continue
    const def = defOf(scope, row.type)
    if (def?.unique && rows.some((r) => r.type === row.type)) continue
    let at = rows.length
    if (intro.after) {
      const i = rows.map((r) => r.type).lastIndexOf(intro.after)
      if (i >= 0) at = i + 1
    }
    rows.splice(at, 0, row)
  }
  return { ...layout, rows, version: target }
}

/** Parses a stored JSON string: defaults when there is none or it is unusable, migrated otherwise. */
export function parseLayout(scope: HomeScope, json: string | null): HomeLayout {
  if (!json) return defaultLayout(scope)
  try {
    const layout = sanitizeLayout(scope, JSON.parse(json))
    return layout ? migrateLayout(scope, layout) : defaultLayout(scope)
  } catch {
    return defaultLayout(scope)
  }
}

// ---------------------------------------------------------------- pure edits

let counter = 0
/** Id of a row added by the user. */
export function newRowId(type: string): string {
  counter += 1
  return `${type}-${Date.now().toString(36)}${counter.toString(36)}`
}

export const hasRow = (layout: HomeLayout, type: RowType, param?: string): boolean =>
  layout.rows.some((r) => r.type === type && (param === undefined || r.param === param))

export interface RowSpec {
  type: RowType
  param?: string
  label?: string
}

/** Adds a row: the hero goes first, anything else last. Unique rows and exact duplicates are refused. */
export function addRow(
  layout: HomeLayout,
  scope: HomeScope,
  spec: RowSpec,
  id: string = newRowId(spec.type),
): HomeLayout {
  const def = defOf(scope, spec.type)
  if (!def) return layout
  if (def.unique && hasRow(layout, spec.type)) return layout
  if (def.param && !spec.param) return layout
  if (def.param && hasRow(layout, spec.type, spec.param)) return layout
  const row: HomeRow = { id, type: spec.type, hidden: false }
  if (def.param) {
    row.param = spec.param
    if (spec.label) row.label = spec.label
  }
  const rows = spec.type === 'hero' ? [row, ...layout.rows] : [...layout.rows, row]
  return { ...layout, rows }
}

export const removeRow = (layout: HomeLayout, id: string): HomeLayout => ({
  ...layout,
  rows: layout.rows.filter((r) => r.id !== id),
})

/** Moves a row to `toIndex` (clamped). */
export function moveRow(layout: HomeLayout, id: string, toIndex: number): HomeLayout {
  const from = layout.rows.findIndex((r) => r.id === id)
  if (from < 0) return layout
  const to = Math.min(layout.rows.length - 1, Math.max(0, toIndex))
  if (to === from) return layout
  const rows = [...layout.rows]
  const [row] = rows.splice(from, 1)
  rows.splice(to, 0, row as HomeRow)
  return { ...layout, rows }
}

/** Applies a full new order (ids), as produced by a drag; unknown ids are ignored, missing rows kept at the end. */
export function reorderRows(layout: HomeLayout, ids: readonly string[]): HomeLayout {
  const byId = new Map(layout.rows.map((r) => [r.id, r]))
  const rows: HomeRow[] = []
  for (const id of ids) {
    const row = byId.get(id)
    if (row) {
      rows.push(row)
      byId.delete(id)
    }
  }
  rows.push(...byId.values())
  return { ...layout, rows }
}

export const setRowHidden = (layout: HomeLayout, id: string, hidden: boolean): HomeLayout => ({
  ...layout,
  rows: layout.rows.map((r) => (r.id === id ? { ...r, hidden } : r)),
})

/** Sets the card size / item count of a row; `undefined` returns the option to its default. */
export function setRowOptions(
  layout: HomeLayout,
  id: string,
  options: { size?: RowSize | undefined; count?: number | undefined },
): HomeLayout {
  return {
    ...layout,
    rows: layout.rows.map((r) => {
      if (r.id !== id) return r
      const next = { ...r }
      if ('size' in options) {
        if (!options.size || options.size === 'auto') delete next.size
        else next.size = options.size
      }
      if ('count' in options) {
        if (options.count === undefined) delete next.count
        else next.count = clampCount(options.count)
      }
      return next
    }),
  }
}

/** Layout with the rows of every video library that has none yet (new libraries appear on their own). */
export function syncLibraries(
  layout: HomeLayout,
  libraries: readonly { id: string; name: string }[],
): HomeLayout {
  const known = new Set(layout.knownLibraries ?? [])
  const fresh = libraries.filter((l) => !known.has(l.id))
  if (fresh.length === 0) return layout
  const rows = [...layout.rows]
  // New libraries follow the other "Recently Added" rows, else the "Next Up" one, else the end.
  let at = rows.map((r) => r.type).lastIndexOf('latest')
  if (at < 0) at = rows.map((r) => r.type).lastIndexOf('nextUp')
  if (at < 0) at = rows.length - 1
  const added: HomeRow[] = fresh
    .filter((l) => !rows.some((r) => r.type === 'latest' && r.param === l.id))
    .map((l) => ({
      id: `latest-${l.id}`,
      type: 'latest' as const,
      hidden: false,
      param: l.id,
      label: l.name,
    }))
  rows.splice(at + 1, 0, ...added)
  return { ...layout, rows, knownLibraries: [...known, ...fresh.map((l) => l.id)] }
}

export const countOf = (row: HomeRow, scope: HomeScope): number =>
  row.count ?? defOf(scope, row.type)?.count ?? 20

// ---------------------------------------------------------------- storage

export const layoutStorageKey = (scope: HomeScope, serverId: string, userId: string): string =>
  `orphyn.home.${scope}.${serverId}.${userId}`

export function loadLayout(
  scope: HomeScope,
  key: string,
  storage: Pick<Storage, 'getItem'>,
): HomeLayout {
  let json: string | null = null
  try {
    json = storage.getItem(key)
  } catch {
    // storage unavailable: defaults
  }
  return parseLayout(scope, json)
}

export function saveLayout(
  key: string,
  layout: HomeLayout,
  storage: Pick<Storage, 'setItem'>,
): void {
  try {
    storage.setItem(key, JSON.stringify({ ...layout, version: LAYOUT_VERSION }))
  } catch {
    // quota / private mode: the layout simply is not remembered
  }
}
