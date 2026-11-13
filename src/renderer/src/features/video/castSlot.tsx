import type { ComponentType } from 'react'
import { create } from 'zustand'
import type { VideoPlaybackController } from './controller'

/** What the cast feature needs to know about the video that is open in the player. */
export interface VideoCastProps {
  controller: VideoPlaybackController
  itemId: string
  title: string
  subtitle?: string
  artworkUrl?: string
  /** Called when a popover opens / closes so the controls do not auto-hide under it. */
  onHoldChange?(held: boolean): void
}

interface VideoCastSlots {
  Button: ComponentType<VideoCastProps> | null
  Screen: ComponentType<VideoCastProps> | null
}

const useSlots = create<VideoCastSlots>()(() => ({ Button: null, Screen: null }))

/** The cast feature registers its player button and its "Playing on" screen here (null removes). */
export function registerVideoCast(slots: Partial<VideoCastSlots>): void {
  useSlots.setState(slots)
}

/** Cast button of the player controls. Renders nothing until the cast feature registers it. */
export function VideoCastButtonSlot(props: VideoCastProps) {
  const Button = useSlots((s) => s.Button)
  return Button ? <Button {...props} /> : null
}

/** "Playing on <device>" screen shown over the player while casting. */
export function VideoCastScreenSlot(props: VideoCastProps) {
  const Screen = useSlots((s) => s.Screen)
  return Screen ? <Screen {...props} /> : null
}
