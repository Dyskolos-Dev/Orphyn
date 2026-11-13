import { create } from 'zustand'

export type NowPlayingView = 'player' | 'lyrics' | 'queue'

interface NowPlayingUi {
  open: boolean
  view: NowPlayingView
  show(view?: NowPlayingView): void
  close(): void
  /** Opens on `view`, or closes if that view is already showing (mini player buttons). */
  toggleView(view: NowPlayingView): void
}

/** Whether the full-screen Now Playing sheet is open, and which tab it shows. */
export const useNowPlayingUi = create<NowPlayingUi>()((set, get) => ({
  open: false,
  view: 'player',
  show: (view) => set({ open: true, view: view ?? get().view }),
  close: () => set({ open: false }),
  toggleView(view) {
    const s = get()
    set(s.open && s.view === view ? { open: false } : { open: true, view })
  },
}))
