import { useMemo, useSyncExternalStore } from 'react'
import { useSession } from '@renderer/jellyfin'
import {
  addRow,
  defaultLayout,
  layoutStorageKey,
  moveRow,
  parseLayout,
  removeRow,
  reorderRows,
  saveLayout,
  setRowHidden,
  setRowOptions,
  syncLibraries,
  type HomeLayout,
  type HomeScope,
  type RowSize,
  type RowSpec,
} from './layout'

const listeners = new Set<() => void>()
const subscribe = (fn: () => void) => {
  listeners.add(fn)
  window.addEventListener('storage', fn)
  return () => {
    listeners.delete(fn)
    window.removeEventListener('storage', fn)
  }
}

function readRaw(key: string | null): string | null {
  if (!key) return null
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

export interface HomeEditor {
  /** The saved layout of the home. */
  layout: HomeLayout
  /** Puts the default layout back. */
  reset(): void
  add(spec: RowSpec): void
  remove(id: string): void
  move(id: string, toIndex: number): void
  /** A full new order, as produced by a drag. */
  reorder(ids: readonly string[]): void
  setHidden(id: string, hidden: boolean): void
  setOptions(id: string, options: { size?: RowSize; count?: number | undefined }): void
}

/**
 * The layout of a home for the active server and user. The homes only read `layout`; Settings >
 * Home uses the other methods, each of which saves to localStorage at once. `libraries` (video)
 * are the libraries the home may show: each gets its "Recently Added" row once.
 */
export function useHomeEditor(
  scope: HomeScope,
  libraries: readonly { id: string; name: string }[] = [],
): HomeEditor {
  const account = useSession((s) => s.account)
  const key = account ? layoutStorageKey(scope, account.serverId, account.userId) : null
  const raw = useSyncExternalStore(
    subscribe,
    () => readRaw(key),
    () => null,
  )
  const libKey = libraries.map((l) => `${l.id}:${l.name}`).join('|')
  const saved = useMemo(
    () => syncLibraries(parseLayout(scope, raw), libraries),
    // `libraries` is compared through libKey: callers build a fresh array on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [scope, raw, libKey],
  )
  const commit = (next: HomeLayout) => {
    if (!key) return
    saveLayout(key, next, window.localStorage)
    listeners.forEach((fn) => fn())
  }

  return {
    layout: saved,
    reset: () => commit(syncLibraries(defaultLayout(scope), libraries)),
    add: (spec) => commit(addRow(saved, scope, spec)),
    remove: (id) => commit(removeRow(saved, id)),
    move: (id, to) => commit(moveRow(saved, id, to)),
    reorder: (ids) => commit(reorderRows(saved, ids)),
    setHidden: (id, hidden) => commit(setRowHidden(saved, id, hidden)),
    setOptions: (id, o) => commit(setRowOptions(saved, id, o)),
  }
}
