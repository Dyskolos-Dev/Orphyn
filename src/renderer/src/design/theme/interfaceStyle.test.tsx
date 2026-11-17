import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { paletteFromPixels, tameAmbientColor } from './ambient'
import {
  APPEARANCE_KEYS,
  DENSITY_SCALE,
  RADIUS_SCALE,
  STYLE_DEFAULTS,
  TEXT_SCALE,
  TEXT_SIZE_OPTIONS,
  appearanceTokens,
  resolveAppearance,
  sanitizeOverrides,
} from './appearance'
import { rgbToHsl } from './color'
import {
  INTERFACE_STYLE_STORAGE_KEYS,
  INTERFACE_STYLES,
  interfaceStyleStore,
} from './interfaceStyle'
import { useInterfaceStyle, useTheme } from './ThemeProvider'
import { themeStore } from './theme'

const html = () => document.documentElement
const stored = () =>
  JSON.parse(localStorage.getItem(INTERFACE_STYLE_STORAGE_KEYS.appearance) ?? 'null') as unknown

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

  it('defaults to classic, with transparency and the style defaults', () => {
    expect(interfaceStyleStore.getState()).toEqual({
      style: 'classic',
      appearance: STYLE_DEFAULTS.classic,
      overrides: {},
      reduceTransparency: false,
    })
    expect(html().dataset['style']).toBe('classic')
    expect(html().hasAttribute('data-reduce-transparency')).toBe(false)
    expect(html().dataset['sidebar']).toBe('expanded')
    expect(html().style.getPropertyValue('--transparency')).toBe('1')
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

  it('"Reduce transparency" is a transparency of 0, persisted', () => {
    act(() => interfaceStyleStore.setReduceTransparency(true))
    expect(html().dataset['reduceTransparency']).toBe('true')
    expect(html().dataset['transparency']).toBe('solid')
    expect(html().style.getPropertyValue('--transparency')).toBe('0')
    expect(stored()).toEqual({ transparency: 0 })
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().reduceTransparency).toBe(true)
    act(() => interfaceStyleStore.setReduceTransparency(false))
    expect(html().hasAttribute('data-reduce-transparency')).toBe(false)
    expect(interfaceStyleStore.getState().appearance.transparency).toBe(100)
  })

  it('reads the 1.1 "Reduce transparency" switch once, then drops it', () => {
    localStorage.setItem(INTERFACE_STYLE_STORAGE_KEYS.reduceTransparency, 'true')
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().appearance.transparency).toBe(0)
    act(() => interfaceStyleStore.setAppearance({ corners: 'round' }))
    expect(localStorage.getItem(INTERFACE_STYLE_STORAGE_KEYS.reduceTransparency)).toBeNull()
    expect(stored()).toEqual({ transparency: 0, corners: 'round' })
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

describe('appearance options', () => {
  beforeEach(() => {
    localStorage.clear()
    interfaceStyleStore.reload()
  })

  it('applies every option at once on <html> and persists only what changed', () => {
    act(() =>
      interfaceStyleStore.setAppearance({
        transparency: 40,
        corners: 'sharp',
        density: 'compact',
        textSize: 'xl',
        sidebar: 'auto-hide',
        backdrop: 'subtle',
        motion: 'reduced',
        videoCards: 'landscape',
      }),
    )
    const d = html().dataset
    expect([d['corners'], d['density'], d['textSize'], d['sidebar']]).toEqual([
      'sharp',
      'compact',
      'xl',
      'auto-hide',
    ])
    expect([d['backdrop'], d['motion'], d['videoCards'], d['transparency']]).toEqual([
      'subtle',
      'reduced',
      'landscape',
      'partial',
    ])
    expect(html().style.getPropertyValue('--transparency')).toBe('0.4')
    expect(html().style.getPropertyValue('--radius-scale')).toBe(String(RADIUS_SCALE.sharp))
    expect(html().style.getPropertyValue('--text-scale')).toBe(String(TEXT_SCALE.xl))
    expect(stored()).toMatchObject({ transparency: 40, sidebar: 'auto-hide' })

    // A relaunch restores them.
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().appearance).toMatchObject({
      density: 'compact',
      videoCards: 'landscape',
    })
  })

  it('layers the user choices over each style defaults, kept across styles', () => {
    act(() => interfaceStyleStore.setStyle('cinema'))
    expect(interfaceStyleStore.getState().appearance).toMatchObject({
      sidebar: 'icons',
      backdrop: 'immersive',
    })
    act(() => interfaceStyleStore.setAppearance({ sidebar: 'expanded', textSize: 'l' }))
    act(() => interfaceStyleStore.setStyle('compact'))
    expect(interfaceStyleStore.getState().appearance).toMatchObject({
      sidebar: 'expanded',
      textSize: 'l',
      backdrop: 'off',
    })
  })

  it('resets to the current style defaults', () => {
    act(() => interfaceStyleStore.setStyle('cinema'))
    act(() => interfaceStyleStore.setAppearance({ sidebar: 'expanded', corners: 'round' }))
    act(() => interfaceStyleStore.resetAppearance())
    expect(interfaceStyleStore.getState().overrides).toEqual({})
    expect(interfaceStyleStore.getState().appearance).toEqual(STYLE_DEFAULTS.cinema)
    expect(html().dataset['sidebar']).toBe('icons')
    expect(stored()).toEqual({})
  })

  it('ignores invalid stored options field by field, and corrupt JSON', () => {
    localStorage.setItem(
      INTERFACE_STYLE_STORAGE_KEYS.appearance,
      JSON.stringify({ transparency: 250, corners: 'wavy', textSize: 'l', sidebar: 42 }),
    )
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().overrides).toEqual({ transparency: 100, textSize: 'l' })
    localStorage.setItem(INTERFACE_STYLE_STORAGE_KEYS.appearance, '{nope')
    interfaceStyleStore.reload()
    expect(interfaceStyleStore.getState().overrides).toEqual({})
  })

  it('options do not cross-fade, a new style does', () => {
    const start = vi.fn((update: () => void) => {
      update()
      return { finished: Promise.resolve() }
    })
    Object.defineProperty(document, 'startViewTransition', { value: start, configurable: true })
    try {
      act(() => interfaceStyleStore.setAppearance({ transparency: 30 }))
      expect(start).not.toHaveBeenCalled()
      act(() => interfaceStyleStore.setStyle('glass'))
      expect(start).toHaveBeenCalledTimes(1)
    } finally {
      Reflect.deleteProperty(document, 'startViewTransition')
    }
  })
})

describe('appearance token mapping', () => {
  it('maps each option to its attribute and variables', () => {
    const tokens = appearanceTokens({
      ...STYLE_DEFAULTS.classic,
      transparency: 0,
      density: 'comfortable',
    })
    expect(tokens.attributes).toMatchObject({
      transparency: 'solid',
      reduceTransparency: 'true',
      density: 'comfortable',
    })
    expect(tokens.properties).toMatchObject({
      '--transparency': '0',
      '--density': String(DENSITY_SCALE.comfortable.space),
      '--density-rows': String(DENSITY_SCALE.comfortable.rows),
      '--density-items': String(DENSITY_SCALE.comfortable.items),
    })
    expect(appearanceTokens(STYLE_DEFAULTS.classic).attributes['reduceTransparency']).toBeNull()
  })

  it('scales radii, type and spacing monotonically', () => {
    expect(RADIUS_SCALE.sharp).toBeLessThan(RADIUS_SCALE.standard)
    expect(RADIUS_SCALE.standard).toBeLessThan(RADIUS_SCALE.round)
    const sizes = TEXT_SIZE_OPTIONS.map((s) => TEXT_SCALE[s])
    expect([...sizes].sort((a, b) => a - b)).toEqual(sizes)
    expect(DENSITY_SCALE.compact.space).toBeLessThan(DENSITY_SCALE.comfortable.space)
    // Integers: they are added to the number of items a shelf shows.
    for (const d of Object.values(DENSITY_SCALE)) expect(Number.isInteger(d.items)).toBe(true)
  })

  it('gives every style a complete set of defaults', () => {
    for (const style of INTERFACE_STYLES) {
      expect(Object.keys(STYLE_DEFAULTS[style]).sort()).toEqual([...APPEARANCE_KEYS].sort())
      expect(resolveAppearance(style, {})).toEqual(STYLE_DEFAULTS[style])
    }
    expect(sanitizeOverrides(null)).toEqual({})
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

  it('tames neon and near-black artwork colours to a readable mid tone', () => {
    for (const c of [
      [255, 255, 0],
      [5, 5, 8],
      [250, 250, 250],
      [0, 255, 60],
    ] as const) {
      const l = rgbToHsl(tameAmbientColor(c)).l
      expect(l).toBeGreaterThanOrEqual(0.31)
      expect(l).toBeLessThanOrEqual(0.59)
    }
  })
})
