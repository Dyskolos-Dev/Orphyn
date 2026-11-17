import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { mockColorScheme } from '../test-utils'
import { contrast, parseHex, type Rgb } from './color'
import {
  AA,
  ACCENT_PRESETS,
  THEME_STORAGE_KEYS,
  accentHex,
  accentTokens,
  contrastOn,
  resolveTheme,
  themeStore,
} from './theme'
import { ThemeProvider, useTheme } from './ThemeProvider'

const html = () => document.documentElement

function ThemeProbe() {
  const { mode, resolvedTheme, accent, setMode, setAccent } = useTheme()
  return (
    <div>
      <span data-testid="state">{`${mode}/${resolvedTheme}/${accent}`}</span>
      <button onClick={() => setMode('dark')}>dark</button>
      <button onClick={() => setMode('light')}>light</button>
      <button onClick={() => setMode('system')}>system</button>
      <button onClick={() => setAccent('blue')}>blue</button>
    </div>
  )
}

describe('theme', () => {
  let scheme: ReturnType<typeof mockColorScheme>

  beforeEach(() => {
    localStorage.clear()
    html().removeAttribute('data-theme')
    html().removeAttribute('data-accent')
    html().removeAttribute('style')
    scheme = mockColorScheme(false)
    themeStore.reload()
  })
  afterEach(() => scheme.restore())

  it('defaults to system mode and the red accent', () => {
    expect(themeStore.getState()).toEqual({
      mode: 'system',
      resolvedTheme: 'light',
      accent: 'red',
      followArtwork: false,
      artworkColor: null,
    })
    expect(html().dataset['theme']).toBe('light')
    expect(html().dataset['accent']).toBe('red')
    expect(html().style.getPropertyValue('--accent-light')).toBe(
      accentTokens('#fa2d48', 'light').accent,
    )
  })

  it('persists the mode in localStorage and restores it on the next launch', () => {
    themeStore.setMode('dark')
    expect(localStorage.getItem(THEME_STORAGE_KEYS.mode)).toBe('dark')
    expect(html().dataset['theme']).toBe('dark')

    // Simulate a relaunch: wipe the DOM state, re-read storage.
    html().removeAttribute('data-theme')
    themeStore.reload()
    expect(themeStore.getState().mode).toBe('dark')
    expect(html().dataset['theme']).toBe('dark')
  })

  it('persists the accent, presets and custom colours', () => {
    themeStore.setAccent('blue')
    expect(localStorage.getItem(THEME_STORAGE_KEYS.accent)).toBe('blue')
    expect(html().dataset['accent']).toBe('blue')

    themeStore.setAccent('#ffd60a')
    expect(html().dataset['accent']).toBeUndefined()
    // Bright yellow keeps its fill (black text) in dark; text uses a darker yellow in light.
    expect(html().style.getPropertyValue('--accent-fill-dark')).toBe('#ffd60a')
    expect(html().style.getPropertyValue('--accent-fill-contrast-dark')).toBe('#000000')
    expect(html().style.getPropertyValue('--accent-light')).not.toBe('#ffd60a')
    themeStore.reload()
    expect(themeStore.getState().accent).toBe('#ffd60a')
  })

  it('ignores invalid stored values', () => {
    localStorage.setItem(THEME_STORAGE_KEYS.mode, 'sepia')
    localStorage.setItem(THEME_STORAGE_KEYS.accent, 'javascript:alert(1)')
    themeStore.reload()
    expect(themeStore.getState()).toMatchObject({ mode: 'system', accent: 'red' })
  })

  it('follows the OS appearance in system mode, and only then', () => {
    render(
      <ThemeProvider>
        <ThemeProbe />
      </ThemeProvider>,
    )
    expect(screen.getByTestId('state')).toHaveTextContent('system/light/red')
    act(() => scheme.setDark(true))
    expect(screen.getByTestId('state')).toHaveTextContent('system/dark/red')
    expect(html().dataset['theme']).toBe('dark')

    act(() => themeStore.setMode('light'))
    act(() => scheme.setDark(false))
    act(() => scheme.setDark(true))
    expect(html().dataset['theme']).toBe('light')
  })

  it('switches instantly through useTheme()', async () => {
    const user = userEvent.setup()
    render(
      <ThemeProvider>
        <ThemeProbe />
      </ThemeProvider>,
    )
    await user.click(screen.getByText('dark'))
    expect(html().dataset['theme']).toBe('dark')
    expect(screen.getByTestId('state')).toHaveTextContent('dark/dark/red')
    await user.click(screen.getByText('blue'))
    expect(screen.getByTestId('state')).toHaveTextContent('dark/dark/blue')
    await user.click(screen.getByText('system'))
    expect(screen.getByTestId('state')).toHaveTextContent('system/light/blue')
  })

  it('stops listening to the OS when the provider unmounts', () => {
    const { unmount } = render(<ThemeProvider />)
    expect(scheme.listenerCount()).toBe(1)
    unmount()
    expect(scheme.listenerCount()).toBe(0)
  })

  it('pushes the appearance to the native window when running in Electron', () => {
    const setAppearance = vi.fn(() => Promise.resolve())
    const setTitleBarOverlay = vi.fn(() => Promise.resolve())
    ;(window as unknown as { orphyn: unknown }).orphyn = {
      window: { setAppearance, setTitleBarOverlay },
    }
    try {
      themeStore.setMode('dark')
      expect(setAppearance).toHaveBeenCalledWith('dark')
      expect(setTitleBarOverlay).toHaveBeenCalledWith({
        color: '#00000000',
        symbolColor: '#ffffff',
      })
    } finally {
      delete (window as unknown as { orphyn?: unknown }).orphyn
    }
  })

  it('follows the artwork colour when asked, and shares it with other windows', () => {
    themeStore.setArtworkColor('#3366cc')
    // Not following yet: the preset stays.
    expect(html().dataset['accentSource']).toBeUndefined()
    expect(html().style.getPropertyValue('--accent-fill-light')).toBe(
      accentTokens('#fa2d48', 'light').fill,
    )
    themeStore.setFollowArtwork(true)
    expect(html().dataset['accentSource']).toBe('artwork')
    expect(html().style.getPropertyValue('--accent-fill-light')).toBe(
      accentTokens('#3366cc', 'light').fill,
    )
    expect(localStorage.getItem(THEME_STORAGE_KEYS.followArtwork)).toBe('true')
    expect(localStorage.getItem(THEME_STORAGE_KEYS.artworkColor)).toBe('#3366cc')
    // A PiP window re-reads storage and gets the same accent.
    themeStore.reload()
    expect(themeStore.getState()).toMatchObject({ followArtwork: true, artworkColor: '#3366cc' })
    themeStore.setArtworkColor('not a colour')
    expect(themeStore.getState().artworkColor).toBe('#3366cc')
    themeStore.setFollowArtwork(false)
    expect(html().dataset['accentSource']).toBeUndefined()
  })

  it('resolves and computes contrast helpers', () => {
    expect(resolveTheme('system', true)).toBe('dark')
    expect(resolveTheme('light', true)).toBe('light')
    expect(contrastOn('#007aff')).toBe('#ffffff')
    expect(contrastOn('#ffffff')).toBe('#000000')
  })
})

describe('accessible accent tokens', () => {
  const rgb = (hex: string) => parseHex(hex) as Rgb
  // The surfaces accent text sits on: windows, sidebars, grouped lists, elevated sheets, glass
  // over its wash, and the 14 % accent tint of tinted buttons.
  const surfaces = {
    light: ['#ffffff', '#f5f5f7', '#f2f2f7', '#eef0f6', '#e5e5ea'],
    dark: ['#000000', '#1d1d1f', '#262628', '#2c2c2e', '#3a3a3c', '#121318', '#0b0b0d'],
  } as const
  const window = { light: '#ffffff', dark: '#1d1d1f' } as const
  // Deterministic pseudo-random custom colours, plus the extremes.
  const customs = ['#ffffff', '#000000', '#808080', '#ffff00', '#00ffff', '#ff00ff', '#7f7f00']
  let seed = 7
  for (let i = 0; i < 60; i++) {
    seed = (seed * 16807) % 2147483647
    customs.push(`#${(seed % 0xffffff).toString(16).padStart(6, '0')}`)
  }

  it.each(['light', 'dark'] as const)(
    'reach AA in %s for every preset and custom colour',
    (theme) => {
      const colours = [...ACCENT_PRESETS.map((p) => accentHex(p, theme)), ...customs]
      for (const c of colours) {
        const t = accentTokens(c, theme)
        for (const s of surfaces[theme]) {
          expect(contrast(rgb(t.accent), rgb(s)), `${c} text on ${s}`).toBeGreaterThanOrEqual(AA)
        }
        expect(
          contrast(rgb(t.accentContrast), rgb(t.accent)),
          `${c} on accent`,
        ).toBeGreaterThanOrEqual(AA)
        expect(contrast(rgb(t.fillContrast), rgb(t.fill)), `${c} on fill`).toBeGreaterThanOrEqual(
          AA,
        )
        // A filled control stands out from the window (non-text contrast).
        expect(contrast(rgb(t.fill), rgb(window[theme])), `${c} fill`).toBeGreaterThanOrEqual(3)
      }
    },
  )

  it('keeps the hue and changes nothing that already passes', () => {
    expect(accentTokens('#5856d6', 'light')).toMatchObject({ accent: '#5856d6', fill: '#5856d6' })
    const red = accentTokens('#fa2d48', 'light')
    const [r, g, b] = rgb(red.accent)
    expect(r).toBeGreaterThan(g + b)
  })
})
