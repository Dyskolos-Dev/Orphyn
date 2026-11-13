import { useSyncExternalStore } from 'react'

/** Recent searches, newest first, in localStorage under this key (JSON array of strings). */
export const RECENT_SEARCHES_KEY = 'orphyn.search.recent'
export const MAX_RECENT_SEARCHES = 8

function read(): string[] {
  try {
    const parsed: unknown = JSON.parse(
      globalThis.localStorage?.getItem(RECENT_SEARCHES_KEY) ?? '[]',
    )
    return Array.isArray(parsed)
      ? parsed.filter((s): s is string => typeof s === 'string').slice(0, MAX_RECENT_SEARCHES)
      : []
  } catch {
    return []
  }
}

let current = read()
const listeners = new Set<() => void>()

function commit(next: string[]) {
  current = next
  try {
    globalThis.localStorage?.setItem(RECENT_SEARCHES_KEY, JSON.stringify(next))
  } catch {
    // Storage unavailable: kept for this session only.
  }
  listeners.forEach((l) => l())
}

export function getRecentSearches(): string[] {
  return current
}

/** Remembers a search (case-insensitive de-duplication, newest first, capped). */
export function addRecentSearch(term: string): void {
  const t = term.trim()
  if (t.length < 2) return
  commit(
    [t, ...current.filter((s) => s.toLowerCase() !== t.toLowerCase())].slice(
      0,
      MAX_RECENT_SEARCHES,
    ),
  )
}

export function removeRecentSearch(term: string): void {
  commit(current.filter((s) => s !== term))
}

export function clearRecentSearches(): void {
  commit([])
}

export function reloadRecentSearches(): void {
  current = read()
  listeners.forEach((l) => l())
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useRecentSearches(): string[] {
  return useSyncExternalStore(subscribe, getRecentSearches, getRecentSearches)
}
