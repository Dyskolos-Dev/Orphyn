import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
  type RefObject,
} from 'react'
import { createPortal } from 'react-dom'

export interface ShellContextValue {
  /** The scrolling content element (pages scroll inside it, not the window). */
  scrollRef: RefObject<HTMLDivElement | null>
  nowPlayingTarget: HTMLElement | null
  serverSwitcherTarget: HTMLElement | null
  /** Number of mounted immersive requests (video player). */
  setImmersiveCount(update: (n: number) => number): void
}

export const ShellContext = createContext<ShellContextValue | null>(null)

/** Shell layout internals. `null` outside the shell (tests, storybook-like demos). */
export function useShellContext(): ShellContextValue | null {
  return useContext(ShellContext)
}

function SlotPortal({ target, children }: { target: HTMLElement | null; children: ReactNode }) {
  return target ? createPortal(children, target) : null
}

/**
 * Renders its children in the bottom now-playing bar while it is mounted (the bar appears when it
 * has content). For a player that must survive navigation, register `nowPlaying` with
 * `registerShellExtension` instead.
 */
export function ShellNowPlayingSlot({ children }: { children: ReactNode }) {
  return <SlotPortal target={useShellContext()?.nowPlayingTarget ?? null}>{children}</SlotPortal>
}

/**
 * Renders its children in the sidebar's server-switcher slot (above Settings) while mounted. The
 * servers feature usually registers `serverSwitcher` with `registerShellExtension` instead.
 */
export function ShellServerSwitcherSlot({ children }: { children: ReactNode }) {
  return (
    <SlotPortal target={useShellContext()?.serverSwitcherTarget ?? null}>{children}</SlotPortal>
  )
}

/**
 * While `active`, hides the sidebar, toolbars and now-playing bar so a page (video player) can
 * use the whole window. Combine with `useWindowState().setFullscreen(true)` for native fullscreen.
 */
export function useShellImmersive(active = true): void {
  const ctx = useShellContext()
  const set = ctx?.setImmersiveCount
  useEffect(() => {
    if (!active || !set) return
    set((n) => n + 1)
    return () => set((n) => n - 1)
  }, [active, set])
}

/** Component form of useShellImmersive. */
export function ShellImmersive({ active = true }: { active?: boolean }) {
  useShellImmersive(active)
  return null
}

/** Shell-internal state hook for the immersive counter. */
export function useImmersiveState() {
  const [count, setCount] = useState(0)
  return [count > 0, setCount] as const
}
