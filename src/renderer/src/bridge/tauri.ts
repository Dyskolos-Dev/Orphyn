/** Thin typed helpers over `@tauri-apps/api` used by every bridge file. */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/** Calls a Rust command. Arguments use camelCase keys (Tauri maps them to snake_case). */
export function call<T = void>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args)
}

/**
 * Subscribes to an event emitted by the Rust side and returns a synchronous unsubscribe
 * (`listen` itself resolves asynchronously).
 */
export function subscribe<T>(event: string, cb: (payload: T) => void): () => void {
  let unlisten: (() => void) | null = null
  let cancelled = false
  void listen<T>(event, (e) => cb(e.payload)).then(
    (off) => {
      if (cancelled) off()
      else unlisten = off
    },
    () => undefined,
  )
  return () => {
    cancelled = true
    unlisten?.()
    unlisten = null
  }
}
