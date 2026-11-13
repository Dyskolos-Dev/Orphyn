import { render, type RenderOptions } from '@testing-library/react'
import type { ReactElement } from 'react'
import { MemoryRouter } from 'react-router'
import '@renderer/i18n'

/** Renders inside a MemoryRouter (cards and sidebar items use router links). */
export function renderWithRouter(
  ui: ReactElement,
  { route = '/', ...options }: RenderOptions & { route?: string } = {},
) {
  return render(<MemoryRouter initialEntries={[route]}>{ui}</MemoryRouter>, options)
}

type MqListener = (e: MediaQueryListEvent) => void

/** Installs a controllable `prefers-color-scheme: dark` media query on window. */
export function mockColorScheme(initialDark: boolean) {
  let dark = initialDark
  const listeners = new Set<MqListener>()
  const mql = {
    get matches() {
      return dark
    },
    media: '(prefers-color-scheme: dark)',
    onchange: null,
    addEventListener: (_: string, l: MqListener) => listeners.add(l),
    removeEventListener: (_: string, l: MqListener) => listeners.delete(l),
    addListener: (l: MqListener) => listeners.add(l),
    removeListener: (l: MqListener) => listeners.delete(l),
    dispatchEvent: () => true,
  }
  const original = window.matchMedia
  window.matchMedia = ((q: string) =>
    q.includes('prefers-color-scheme') ? mql : { ...mql, matches: false, media: q }) as never
  return {
    setDark(next: boolean) {
      dark = next
      listeners.forEach((l) => l({ matches: next } as MediaQueryListEvent))
    },
    listenerCount: () => listeners.size,
    restore() {
      window.matchMedia = original
    },
  }
}
