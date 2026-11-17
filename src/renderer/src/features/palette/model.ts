import type { BaseItemDto } from '@renderer/jellyfin'
import { COMMAND_GROUPS, type CommandMatch } from './registry'

export type LibraryKind = 'movie' | 'series' | 'episode' | 'artist' | 'album' | 'song' | 'playlist'

export interface LibrarySection {
  id: string
  kind: LibraryKind
  items: BaseItemDto[]
}

export interface CommandEntry {
  type: 'command'
  key: string
  match: CommandMatch
}

export interface LibraryEntry {
  type: 'item'
  key: string
  kind: LibraryKind
  item: BaseItemDto
  /** Highlighted positions in the item's name (empty when the server matched it another way). */
  indices: number[]
}

export type PaletteEntry = CommandEntry | LibraryEntry

export interface PaletteSection {
  /** `recent`, `commands`, `group:<group>` or `library:<id>`. */
  id: string
  entries: PaletteEntry[]
}

export interface PaletteModel {
  sections: PaletteSection[]
  /** Every entry in display order (the keyboard moves through this list). */
  flat: PaletteEntry[]
  /** Index in `flat` where each section starts (Tab jumps between them). */
  groupStarts: number[]
}

const MAX_COMMANDS = 6
const MAX_PER_LIBRARY_SECTION = 4
/** A command scoring at least this well (a clear name match) is listed above library results. */
export const STRONG_COMMAND_SCORE = 60

export interface BuildInput {
  query: string
  commands: readonly CommandMatch[]
  recentIds: readonly string[]
  library: readonly LibrarySection[]
  /** Highlights for library names. */
  highlight(name: string): number[]
}

/**
 * Lays the palette out. Empty query: recently used commands, then every command by group. With a
 * query: the matching commands and the library hits; the commands lead when one matches clearly.
 */
export function buildModel(input: BuildInput): PaletteModel {
  const sections: PaletteSection[] = []
  const queried = input.query.trim().length > 0

  const commandEntry = (match: CommandMatch): CommandEntry => ({
    type: 'command',
    key: `cmd:${match.command.id}`,
    match,
  })

  const librarySections: PaletteSection[] = input.library
    .filter((s) => s.items.length > 0)
    .map((s) => ({
      id: `library:${s.id}`,
      entries: s.items.slice(0, MAX_PER_LIBRARY_SECTION).map<LibraryEntry>((item) => ({
        type: 'item',
        key: `item:${s.kind}:${item.Id ?? item.Name}`,
        kind: s.kind,
        item,
        indices: input.highlight(item.Name ?? ''),
      })),
    }))

  if (queried) {
    const top = input.commands.slice(0, MAX_COMMANDS)
    const commandSection: PaletteSection[] = top.length
      ? [{ id: 'commands', entries: top.map(commandEntry) }]
      : []
    const commandsLead =
      librarySections.length === 0 || (top[0]?.score ?? 0) >= STRONG_COMMAND_SCORE
    sections.push(
      ...(commandsLead
        ? [...commandSection, ...librarySections]
        : [...librarySections, ...commandSection]),
    )
  } else {
    const recent = input.recentIds
      .map((id) => input.commands.find((m) => m.command.id === id))
      .filter((m): m is CommandMatch => m !== undefined)
    const recentIds = new Set(recent.map((m) => m.command.id))
    if (recent.length) sections.push({ id: 'recent', entries: recent.map(commandEntry) })
    for (const group of COMMAND_GROUPS) {
      const entries = input.commands
        .filter((m) => m.command.group === group && !recentIds.has(m.command.id))
        .map(commandEntry)
      if (entries.length) sections.push({ id: `group:${group}`, entries })
    }
  }

  const flat = sections.flatMap((s) => s.entries)
  const groupStarts: number[] = []
  let at = 0
  for (const s of sections) {
    groupStarts.push(at)
    at += s.entries.length
  }
  return { sections, flat, groupStarts }
}

/** App route that opens a library result. */
export function openRoute(kind: LibraryKind, item: BaseItemDto): string {
  const id = item.Id ?? ''
  switch (kind) {
    case 'movie':
    case 'series':
    case 'episode':
      return `/video/item/${id}`
    case 'artist':
      return `/music/artist/${id}`
    case 'album':
      return `/music/album/${id}`
    case 'playlist':
      return `/music/playlist/${id}`
    case 'song':
      return `/music/album/${item.AlbumId ?? id}`
  }
}

export type PlayAction = { type: 'video'; route: string } | { type: 'audio'; id: string } | null

/** What Cmd/Ctrl+Enter does on a library result: play it. A series has no single thing to play. */
export function playAction(kind: LibraryKind, item: BaseItemDto): PlayAction {
  const id = item.Id
  if (!id) return null
  switch (kind) {
    case 'movie':
    case 'episode':
      return { type: 'video', route: `/video/play/${id}` }
    case 'series':
      return null
    default:
      return { type: 'audio', id }
  }
}
