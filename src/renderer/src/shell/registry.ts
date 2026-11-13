import { useSyncExternalStore, type ComponentType, type ReactNode } from 'react'

/** Sidebar sections features can add items to. `more` is an untitled group after Music. */
export type ShellSidebarSection = 'watch' | 'music' | 'more'

export interface ShellSidebarEntry {
  /** Unique within the extension. */
  id: string
  section: ShellSidebarSection
  /** Text or a node, e.g. `<Trans ns="cast" i18nKey="nav.cast" />` to stay translated. */
  label: ReactNode
  /** lucide icon element. */
  icon?: ReactNode
  to: string
  /** Sort key inside the section; built-in items use 0..99, default 100 (after them). */
  order?: number
}

/**
 * What a feature can plug into the shell without editing shell files. Register it at module
 * scope of a file your feature's routes.tsx imports:
 *
 *   registerShellExtension({ id: 'music', nowPlaying: MiniPlayer, provider: PlayerProvider })
 */
export interface ShellExtension {
  /** Unique id; registering the same id again replaces the previous one (HMR-safe). */
  id: string
  /** Persistent content of the bottom now-playing bar (music mini player). */
  nowPlaying?: ComponentType
  /** Server switcher at the bottom of the sidebar (servers feature). */
  serverSwitcher?: ComponentType
  /** Extra sidebar links. */
  sidebarItems?: readonly ShellSidebarEntry[]
  /** Global toolbar buttons, shown on the right of every page's toolbar (e.g. a cast button). */
  toolbarItems?: ComponentType
  /** Context provider wrapping the whole shell (routes included), e.g. a player store. */
  provider?: ComponentType<{ children: ReactNode }>
  /** Invisible always-mounted component (global shortcuts, media session...). */
  background?: ComponentType
}

type Listener = () => void
let extensions: readonly ShellExtension[] = []
const listeners = new Set<Listener>()

/** Adds (or replaces, by id) a shell extension. Returns a function that removes it. */
export function registerShellExtension(extension: ShellExtension): () => void {
  extensions = [...extensions.filter((e) => e.id !== extension.id), extension]
  listeners.forEach((l) => l())
  return () => {
    if (!extensions.includes(extension)) return
    extensions = extensions.filter((e) => e !== extension)
    listeners.forEach((l) => l())
  }
}

export function getShellExtensions(): readonly ShellExtension[] {
  return extensions
}

function subscribe(l: Listener): () => void {
  listeners.add(l)
  return () => {
    listeners.delete(l)
  }
}

/** Current extensions, re-rendering when one is added or removed. */
export function useShellExtensions(): readonly ShellExtension[] {
  return useSyncExternalStore(subscribe, getShellExtensions, getShellExtensions)
}
