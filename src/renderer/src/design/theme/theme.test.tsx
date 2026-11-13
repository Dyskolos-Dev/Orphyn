import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { mockColorScheme } from '../test-utils'
import { THEME_STORAGE_KEYS, contrastOn, resolveTheme, themeStore } from './theme'
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
    expect(themeStore.getState()).toEqual({ mode: 'system', resolvedTheme: 'light', accent: 'red' })
    expect(html().dataset['theme']).toBe('light')
    expect(html().dataset['accent']).toBe('red')
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
    expect(html().style.getPropertyValue('--accent')).toBe('#ffd60a')
    expect(html().style.getPropertyValue('--accent-contrast')).toBe('#000000')
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

  it('resolves and computes contrast helpers', () => {
    expect(resolveTheme('system', true)).toBe('dark')
    expect(resolveTheme('light', true)).toBe('light')
    expect(contrastOn('#007aff')).toBe('#ffffff')
    expect(contrastOn('#ffffff')).toBe('#000000')
  })
})
