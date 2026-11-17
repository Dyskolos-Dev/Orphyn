import { AnimatePresence } from 'framer-motion'
import { useEffect } from 'react'
import { createPortal } from 'react-dom'
import { getPlatform } from '@renderer/design'
import { PalettePanel } from './PalettePanel'
import { usePaletteUi } from './store'

/** True for Cmd+K on macOS, Ctrl+K elsewhere (either one in a plain browser). */
export function isPaletteShortcut(e: KeyboardEvent, platform = getPlatform()): boolean {
  if (e.key.toLowerCase() !== 'k' || e.shiftKey || e.altKey) return false
  if (platform === 'darwin') return e.metaKey && !e.ctrlKey
  if (platform === 'web') return e.metaKey !== e.ctrlKey
  return e.ctrlKey && !e.metaKey
}

/** Always mounted by the shell: listens for the shortcut and shows the palette over the app. */
export function PaletteHost() {
  const open = usePaletteUi((s) => s.open)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!isPaletteShortcut(e)) return
      e.preventDefault()
      e.stopPropagation()
      usePaletteUi.getState().toggle()
    }
    // Capture: works from inside text fields and the video player too.
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [])

  return createPortal(
    <AnimatePresence>{open && <PalettePanel key="palette" />}</AnimatePresence>,
    document.body,
  )
}
