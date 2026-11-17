import type { LucideIcon } from 'lucide-react'
import { rank } from './fuzzy'

/** Section a command is listed under. */
export type CommandGroup = 'playback' | 'navigate' | 'actions' | 'appearance' | 'servers'

/** Order of the command sections in the palette. */
export const COMMAND_GROUPS: readonly CommandGroup[] = [
  'playback',
  'navigate',
  'actions',
  'appearance',
  'servers',
]

export type Lang = 'fr' | 'en'
export type RepeatState = 'off' | 'all' | 'one'

/** What a command may read and do: built by the palette host from the live app state. */
export interface PaletteContext {
  navigate(to: string): void
  player: {
    hasTrack: boolean
    playing: boolean
    shuffle: boolean
    repeat: RepeatState
    toggle(): void
    next(): void
    previous(): void
    toggleShuffle(): void
    cycleRepeat(): void
  }
  /** Favourite state of the track that is playing. */
  favorite: { available: boolean; value: boolean; toggle(): void }
  openNowPlaying(): void
  /** Undefined where the shell has no Picture in Picture. */
  openPip?: () => void
  addToPlaylist(): void
  theme: { resolved: 'light' | 'dark'; setMode(mode: 'light' | 'dark'): void }
  interfaceStyle: { current: string; styles: readonly string[]; set(style: string): void }
  accounts: { id: string; name: string; active: boolean }[]
  switchServer(id: string): void
  signedIn: boolean
  signOut(): void
  refreshLibrary(): void
  /** Undefined outside the desktop app. */
  checkForUpdates?: () => void
}

export interface Command {
  /** Stable id (kebab-case); also the i18n key under `commands.` and the recents key. */
  id: string
  group: CommandGroup
  /** Key under `commands.` in palette.json; may depend on the state ("Pause" vs "Play"). */
  labelKey?: string | ((ctx: PaletteContext) => string)
  /** Literal name, for commands generated from data (a server's name). Same in every language. */
  title?: string
  icon: LucideIcon
  /** Picks the icon from the state instead (Play / Pause); `icon` is the fallback. */
  iconFor?(ctx: PaletteContext): LucideIcon
  /** Extra search terms shared by both languages (synonyms, English or French slang). */
  keywords?: readonly string[]
  /** Shortcut hint shown on the right. */
  shortcut?: string
  available?(ctx: PaletteContext): boolean
  run(ctx: PaletteContext): void | Promise<void>
}

/** Icon of a command in the current state . */
export function commandIcon(cmd: Command, ctx: PaletteContext): LucideIcon {
  return cmd.iconFor?.(ctx) ?? cmd.icon
}

/** A command, or a function producing commands from the context (one per server...). */
export type CommandSource = Command | ((ctx: PaletteContext) => Command[])

export interface CommandRegistry {
  /** Adds a command or provider; an id registered twice replaces the previous one. Returns remove. */
  register(...sources: CommandSource[]): () => void
  /** Commands available in `ctx`, in registration order. */
  list(ctx: PaletteContext): Command[]
  subscribe(listener: () => void): () => void
}

export function createCommandRegistry(): CommandRegistry {
  let sources: CommandSource[] = []
  const listeners = new Set<() => void>()
  const emit = () => listeners.forEach((l) => l())
  const idOf = (s: CommandSource) => (typeof s === 'function' ? null : s.id)

  return {
    register(...added) {
      const ids = new Set(added.map(idOf).filter((id): id is string => id !== null))
      sources = [...sources.filter((s) => !ids.has(idOf(s) ?? '')), ...added]
      emit()
      return () => {
        sources = sources.filter((s) => !added.includes(s))
        emit()
      }
    },
    list(ctx) {
      const seen = new Set<string>()
      const out: Command[] = []
      for (const source of sources) {
        for (const cmd of typeof source === 'function' ? source(ctx) : [source]) {
          if (seen.has(cmd.id) || cmd.available?.(ctx) === false) continue
          seen.add(cmd.id)
          out.push(cmd)
        }
      }
      return out
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}

/** `translate(lang, key)` returns the palette string for a language, or '' when it has none. */
export type Translate = (lang: Lang, key: string) => string

export interface CommandNames {
  /** Name shown, in the interface language. */
  title: string
  /** Everything else that should match: other language, keywords. */
  aliases: string[]
}

export function commandNames(
  cmd: Command,
  ctx: PaletteContext,
  lang: Lang,
  translate: Translate,
): CommandNames {
  const other: Lang = lang === 'fr' ? 'en' : 'fr'
  if (cmd.title) return { title: cmd.title, aliases: [...(cmd.keywords ?? [])] }
  const key = typeof cmd.labelKey === 'function' ? cmd.labelKey(ctx) : (cmd.labelKey ?? cmd.id)
  const k = `commands.${key}`
  return {
    title: translate(lang, k) || cmd.id,
    aliases: [translate(other, k), ...(cmd.keywords ?? [])].filter(Boolean),
  }
}

export interface CommandMatch {
  command: Command
  title: string
  /** Highlighted positions in `title`. */
  indices: number[]
  score: number
}

/**
 * Commands matching `query`, best first. An empty query returns `recentIds` first (most recent
 * first), then every other command in registration order.
 */
export function searchCommands(
  commands: readonly Command[],
  ctx: PaletteContext,
  query: string,
  opts: { lang: Lang; translate: Translate; recentIds?: readonly string[] },
): CommandMatch[] {
  const named = commands.map((command) => ({
    command,
    ...commandNames(command, ctx, opts.lang, opts.translate),
  }))
  if (!query.trim()) {
    const recent = opts.recentIds ?? []
    const pos = (id: string) => {
      const i = recent.indexOf(id)
      return i < 0 ? Infinity : i
    }
    return named
      .map((n, order) => ({ n, order }))
      .sort((a, b) => pos(a.n.command.id) - pos(b.n.command.id) || a.order - b.order)
      .map(({ n }) => ({ command: n.command, title: n.title, indices: [], score: 0 }))
  }
  return rank(named, query, (n) => [n.title, ...n.aliases]).map((r) => ({
    command: r.item.command,
    title: r.item.title,
    indices: r.indices,
    score: r.score,
  }))
}
