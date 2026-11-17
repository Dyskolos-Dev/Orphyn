import { create } from 'zustand'

interface PaletteUi {
  open: boolean
  show(): void
  hide(): void
  toggle(): void
}

/** Whether the command palette is showing. Anything can open it: `openPalette()`. */
export const usePaletteUi = create<PaletteUi>()((set) => ({
  open: false,
  show: () => set({ open: true }),
  hide: () => set({ open: false }),
  toggle: () => set((s) => ({ open: !s.open })),
}))

export const openPalette = (): void => usePaletteUi.getState().show()
export const closePalette = (): void => usePaletteUi.getState().hide()
