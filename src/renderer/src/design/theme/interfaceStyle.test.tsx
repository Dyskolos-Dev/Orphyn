import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { paletteFromPixels } from './ambient'
import {
  INTERFACE_STYLE_STORAGE_KEYS,
  INTERFACE_STYLES,
  interfaceStyleStore,
} from './interfaceStyle'
import { useInterfaceStyle, useTheme } from './ThemeProvider'
import { themeStore } from './theme'

const html = () => document.documentElement

function StyleProbe() {
  const { style, reduceTransparency, setStyle, setReduceTransparency } = useInterfaceStyle()
  const { accent } = useTheme()
  return (
    <div>
      <span data-testid="state">{`${style}/${String(reduceTransparency)}/${accent}`}</span>
      <button onClick={() => setStyle('glass')}>glass</button>
      <button onClick={() => setReduceTransparency(!reduceTransparency)}>transparency</button>
    </div>
  )
}

describe('interface style', () => {
  beforeEach(() => {
    localStorage.clear()
    html().removeAttribute('data-style')
    html().removeAttribute('data-reduce-transparency')
    interfaceStyleStore.reload()
  })

  it('defaults to classic, with transparency', () => {
    expect(interfaceStyleStore.getState()).toEqual({ style: 'classic', reduceTransparency: false })
    expect(html().dataset['style']).toBe('classic')
    expect(html().hasAttribute('data-reduce-transparency')).toBe(false)
  })

  it('offers the five styles', () => {
    expect(INTERFACE_STYLES).toEqual(['classic', 'glass', 'cinema', 'minimal', 'compact'])
  })

  it.each(INTERFACE_STYLES)('applies and persists %s', (style) => {
    act(() => interfaceStyleStore.setStyle(style))
    expect(html().dataset['style']).toBe(style)
    expect(localStorage.getItem(INTERFACE_STYLE_STORAGE_KEYS.style)).toBe(style)

    // A relaunch reads it back.
    html().removeAttribute('data-style')
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().style).toBe(style)
    expect(html().dataset['style']).toBe(style)
  })

  it('ignores an unknown stored style', () => {
    localStorage.setItem(INTERFACE_STYLE_STORAGE_KEYS.style, 'neon')
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().style).toBe('classic')
    act(() => interfaceStyleStore.setStyle('neon' as never))
    expect(html().dataset['style']).toBe('classic')
  })

  it('persists "Reduce transparency" as a data attribute', () => {
    act(() => interfaceStyleStore.setReduceTransparency(true))
    expect(html().dataset['reduceTransparency']).toBe('true')
    expect(localStorage.getItem(INTERFACE_STYLE_STORAGE_KEYS.reduceTransparency)).toBe('true')
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().reduceTransparency).toBe(true)
    act(() => interfaceStyleStore.setReduceTransparency(false))
    expect(html().hasAttribute('data-reduce-transparency')).toBe(false)
  })

  it('updates through useInterfaceStyle() and leaves the theme alone', async () => {
    themeStore.setAccent('blue')
    render(<StyleProbe />)
    expect(screen.getByTestId('state')).toHaveTextContent('classic/false/blue')
    await userEvent.click(screen.getByText('glass'))
    await userEvent.click(screen.getByText('transparency'))
    expect(screen.getByTestId('state')).toHaveTextContent('glass/true/blue')
    expect(html().dataset['accent']).toBe('blue')
    expect(html().dataset['style']).toBe('glass')
  })

  it('cross-fades with View Transitions when available', () => {
    const start = vi.fn((update: () => void) => {
      update()
      return { finished: Promise.resolve() }
    })
    Object.defineProperty(document, 'startViewTransition', { value: start, configurable: true })
    try {
      act(() => interfaceStyleStore.setStyle('minimal'))
      expect(start).toHaveBeenCalledTimes(1)
      expect(html().dataset['style']).toBe('minimal')
    } finally {
      Reflect.deleteProperty(document, 'startViewTransition')
    }
  })
})

describe('ambient palette', () => {
  const pixels = (...colors: [number, number, number, number][]) =>
    colors.flatMap((c) => Array.from({ length: 10 }, () => c)).flat()

  it('picks the saturated mid-tone over black bars and white text', () => {
    const p = paletteFromPixels(
      pixels([0, 0, 0, 255], [0, 0, 0, 255], [255, 255, 255, 255], [200, 60, 40, 255]),
    )
    expect(p?.primary).toEqual([200, 60, 40])
  })

  it('returns a distinct secondary colour', () => {
    const p = paletteFromPixels(pixels([200, 60, 40, 255], [200, 60, 40, 255], [40, 60, 200, 255]))
    expect(p?.secondary).toEqual([40, 60, 200])
  })

  it('returns null for transparent images', () => {
    expect(paletteFromPixels(pixels([10, 10, 10, 0]))).toBeNull()
  })
})
