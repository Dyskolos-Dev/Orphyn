import type { ComponentType } from 'react'
import { create } from 'zustand'

interface CastSlotState {
  Component: ComponentType | null
}

const useCastSlot = create<CastSlotState>()(() => ({ Component: null }))

/**
 * The cast feature calls this once (from its own module) to put its AirPlay-like button into the
 * music player; pass null to remove it:
 *
 *   registerCastButton(CastButton)
 */
export function registerCastButton(Component: ComponentType | null): void {
  useCastSlot.setState({ Component })
}

/**
 * Slot of the player bar and of the Now Playing sheet reserved for the cast button. Renders
 * nothing until the cast feature registers its component with `registerCastButton`.
 */
export function CastButtonSlot() {
  const Component = useCastSlot((s) => s.Component)
  return Component ? <Component /> : null
}
