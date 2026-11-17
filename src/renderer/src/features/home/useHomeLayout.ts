import { useCallback, useMemo, useState, useSyncExternalStore } from 'react'
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
  /** Layout to display: the draft while editing, the saved one otherwise. */
  layout: HomeLayout
  editing: boolean
  start(): void
  /** Saves the draft and leaves edit mode. */
  done(): void
  /** Leaves edit mode without saving. */
  cancel(): void
  /** Puts the default layout in the draft (saved by Done). */
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
 * The layout of a home for the active server and user, and its edit mode. Edits go to a draft;
 * "Done" writes it to localStorage, "Cancel" drops it. `libraries` (video) are the libraries the
 * home may show: each gets its "Recently Added" row once.
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
  const [draft, setDraft] = useState<HomeLayout | null>(null)
  const edit = useCallback(
    (fn: (l: HomeLayout) => HomeLayout) => setDraft((d) => (d ? fn(d) : d)),
    [],
  )

  return {
    layout: draft ?? saved,
    editing: draft !== null,
    start: () => setDraft(saved),
    done: () => {
      if (draft && key) {
        saveLayout(key, draft, window.localStorage)
        listeners.forEach((fn) => fn())
      }
      setDraft(null)
    },
    cancel: () => setDraft(null),
    reset: () => edit(() => syncLibraries(defaultLayout(scope), libraries)),
    add: (spec) => edit((l) => addRow(l, scope, spec)),
    remove: (id) => edit((l) => removeRow(l, id)),
    move: (id, to) => edit((l) => moveRow(l, id, to)),
    reorder: (ids) => edit((l) => reorderRows(l, ids)),
    setHidden: (id, hidden) => edit((l) => setRowHidden(l, id, hidden)),
    setOptions: (id, o) => edit((l) => setRowOptions(l, id, o)),
  }
}
