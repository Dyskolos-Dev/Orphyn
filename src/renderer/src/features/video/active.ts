import type { PlaybackController } from './controller'

let active: PlaybackController | null = null
const listeners = new Set<() => void>()

/**
 * Registers the controller of the player that is mounted. Returns a function that unregisters it.
 * The cast feature reads it with `getActivePlaybackController()` to route playback elsewhere.
 */
export function setActiveController(controller: PlaybackController): () => void {
  active = controller
  listeners.forEach((l) => l())
  return () => {
    if (active === controller) {
      active = null
      listeners.forEach((l) => l())
    }
  }
}

/** The controller of the video that is currently open in the player, or null. */
export const getActivePlaybackController = (): PlaybackController | null => active

/** Subscribe to the controller appearing / disappearing (player opened / closed). */
export function subscribeActiveController(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}
