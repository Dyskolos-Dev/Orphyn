import { act, render, renderHook, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import i18n from '@renderer/i18n'
import { interfaceStyleStore, themeStore } from '@renderer/design'
import {
  DEFAULT_PLAYBACK_PREFERENCES,
  PLAYBACK_PREFERENCES_KEY,
  getPlaybackPreferences,
  languageMatches,
  usePlaybackPreferences,
} from './index'
import { MusicPane } from './MusicPane'
import { reloadPlaybackPreferences } from './preferences'
import { AppearancePane } from './AppearancePane'
import { GeneralPane } from './GeneralPane'

beforeEach(() => {
  localStorage.clear()
  reloadPlaybackPreferences()
})

describe('usePlaybackPreferences', () => {
  it('returns the defaults when nothing is stored', () => {
    const { result } = renderHook(() => usePlaybackPreferences())
    expect(result.current).toMatchObject(DEFAULT_PLAYBACK_PREFERENCES)
  })

  it('persists updates under the documented key and merges partial patches', () => {
    const { result } = renderHook(() => usePlaybackPreferences())
    act(() => result.current.update({ maxVideoBitrate: 8_000_000, preferredAudioLanguage: 'fre' }))
    act(() => result.current.update({ autoPlayNext: false, audioQuality: 320_000 }))

    expect(result.current).toMatchObject({
      maxVideoBitrate: 8_000_000,
      preferredAudioLanguage: 'fre',
      preferredSubtitleLanguage: null,
      autoPlayNext: false,
      audioQuality: 320_000,
    })
    expect(JSON.parse(localStorage.getItem(PLAYBACK_PREFERENCES_KEY) ?? '{}')).toMatchObject({
      maxVideoBitrate: 8_000_000,
      autoPlayNext: false,
    })
  })

  it('shares changes between consumers and survives a reload from storage', () => {
    const a = renderHook(() => usePlaybackPreferences())
    const b = renderHook(() => usePlaybackPreferences())
    act(() => a.result.current.update({ preferredSubtitleLanguage: 'eng' }))
    expect(b.result.current.preferredSubtitleLanguage).toBe('eng')

    act(() => reloadPlaybackPreferences())
    expect(b.result.current.preferredSubtitleLanguage).toBe('eng')
  })

  it('ignores invalid stored values field by field', () => {
    localStorage.setItem(
      PLAYBACK_PREFERENCES_KEY,
      JSON.stringify({
        maxVideoBitrate: -5,
        preferredAudioLanguage: 'french please',
        preferredSubtitleLanguage: 'spa',
        autoPlayNext: 'yes',
        audioQuality: 'lossy',
      }),
    )
    reloadPlaybackPreferences()
    const { result } = renderHook(() => usePlaybackPreferences())
    expect(result.current).toMatchObject({
      ...DEFAULT_PLAYBACK_PREFERENCES,
      preferredSubtitleLanguage: 'spa',
    })
  })

  it('survives corrupt JSON and resets to the defaults', () => {
    localStorage.setItem(PLAYBACK_PREFERENCES_KEY, '{nope')
    reloadPlaybackPreferences()
    const { result } = renderHook(() => usePlaybackPreferences())
    expect(result.current.autoPlayNext).toBe(true)
    act(() => result.current.update({ autoPlayNext: false }))
    act(() => result.current.reset())
    expect(result.current).toMatchObject(DEFAULT_PLAYBACK_PREFERENCES)
  })
})

describe('MusicPane crossfade', () => {
  it('is off by default, reveals the length and album options, and applies live', async () => {
    const user = userEvent.setup()
    await act(() => i18n.changeLanguage('en'))
    expect(DEFAULT_PLAYBACK_PREFERENCES).toMatchObject({
      crossfade: false,
      crossfadeSkipAlbum: true,
    })
    render(<MusicPane />)
    expect(screen.queryByRole('slider')).toBeNull()

    await user.click(screen.getByRole('switch', { name: /^Crossfade/ }))
    expect(getPlaybackPreferences().crossfade).toBe(true)
    const slider = screen.getByRole('slider', { name: 'Crossfade length' })
    expect(slider).toHaveAttribute('aria-valuemin', '1')
    expect(slider).toHaveAttribute('aria-valuemax', '12')
    expect(slider).toHaveAttribute('aria-valuetext', '6 s')

    slider.focus()
    await user.keyboard('{ArrowRight}')
    expect(getPlaybackPreferences().crossfadeSeconds).toBe(7)
    expect(screen.getByText('7 s')).toBeInTheDocument()

    await user.click(screen.getByRole('switch', { name: /Don't crossfade within an album/ }))
    expect(getPlaybackPreferences().crossfadeSkipAlbum).toBe(false)
    expect(screen.getByText('Streaming quality')).toBeInTheDocument()
  })

  it('keeps the length within 1-12 s when reading storage', () => {
    localStorage.setItem(
      PLAYBACK_PREFERENCES_KEY,
      JSON.stringify({ crossfade: true, crossfadeSeconds: 99 }),
    )
    reloadPlaybackPreferences()
    expect(getPlaybackPreferences()).toMatchObject({ crossfade: true, crossfadeSeconds: 12 })
  })
})

describe('MusicPane artist grouping and online lyrics', () => {
  it('are on by default, persisted when switched off, and labelled in both languages', async () => {
    const user = userEvent.setup()
    await act(() => i18n.changeLanguage('en'))
    expect(DEFAULT_PLAYBACK_PREFERENCES).toMatchObject({ groupArtists: true, onlineLyrics: true })
    const { unmount } = render(<MusicPane />)
    await user.click(screen.getByRole('switch', { name: /^Group artists/ }))
    await user.click(screen.getByRole('switch', { name: /^Online lyrics/ }))
    expect(getPlaybackPreferences()).toMatchObject({ groupArtists: false, onlineLyrics: false })
    expect(localStorage.getItem(PLAYBACK_PREFERENCES_KEY)).toContain('"onlineLyrics":false')
    unmount()
    await act(() => i18n.changeLanguage('fr'))
    render(<MusicPane />)
    expect(screen.getByRole('switch', { name: /^Regrouper les artistes/ })).toBeInTheDocument()
    expect(screen.getByRole('switch', { name: /^Paroles en ligne/ })).toBeInTheDocument()
    expect(
      screen.getByRole('switch', { name: /^Chercher les images d’artistes/ }),
    ).toBeInTheDocument()
    await act(() => i18n.changeLanguage('en'))
  })

  it('artist image lookup is on by default and persisted when switched off', async () => {
    const user = userEvent.setup()
    await act(() => i18n.changeLanguage('en'))
    expect(DEFAULT_PLAYBACK_PREFERENCES.artistImages).toBe(true)
    render(<MusicPane />)
    await user.click(screen.getByRole('switch', { name: /^Look up artist images/ }))
    expect(getPlaybackPreferences().artistImages).toBe(false)
    expect(localStorage.getItem(PLAYBACK_PREFERENCES_KEY)).toContain('"artistImages":false')
  })
})

describe('languageMatches', () => {
  it('treats ISO 639-2 aliases as the same language', () => {
    expect(languageMatches('fre', 'fra')).toBe(true)
    expect(languageMatches('GER', 'deu')).toBe(true)
    expect(languageMatches('eng', 'fre')).toBe(false)
    expect(languageMatches(null, 'eng')).toBe(false)
  })
})

describe('GeneralPane', () => {
  it('switches the language instantly and remembers it', async () => {
    const user = userEvent.setup()
    await act(() => i18n.changeLanguage('en'))
    render(
      <MemoryRouter>
        <GeneralPane />
      </MemoryRouter>,
    )

    await user.click(screen.getByRole('radio', { name: 'Français' }))
    expect(i18n.language).toBe('fr')
    expect(localStorage.getItem('orphyn.language')).toBe('fr')
    expect((await screen.findAllByText('Langue')).length).toBeGreaterThan(0)

    await act(() => i18n.changeLanguage('en'))
  })
})

describe('AppearancePane', () => {
  const renderPane = async () => {
    await act(() => i18n.changeLanguage('en'))
    interfaceStyleStore.reload()
    themeStore.reload()
    return render(
      <MemoryRouter>
        <AppearancePane />
      </MemoryRouter>,
    )
  }
  afterEach(() => {
    act(() => {
      interfaceStyleStore.setStyle('classic')
      interfaceStyleStore.resetAppearance()
      themeStore.setMode('system')
      themeStore.setAccent('red')
      themeStore.setFollowArtwork(false)
    })
  })

  it('shows a live preview of the current style with its options', async () => {
    await renderPane()
    const live = document.querySelector('.jr-appearance__preview .jr-style-preview__window')
    expect(live).toHaveAttribute('data-style', 'classic')
    expect(live).toHaveAttribute('data-sidebar', 'expanded')
    act(() => interfaceStyleStore.setAppearance({ sidebar: 'icons', videoCards: 'landscape' }))
    expect(live).toHaveAttribute('data-sidebar', 'icons')
    expect(live).toHaveAttribute('data-cards', 'landscape')
  })

  it('switches light / dark and the accent instantly and remembers both', async () => {
    const user = userEvent.setup()
    await renderPane()
    const mode = screen.getByRole('radiogroup', { name: 'Appearance' })
    await user.click(within(mode).getByRole('radio', { name: 'Dark' }))
    expect(document.documentElement.dataset['theme']).toBe('dark')
    expect(localStorage.getItem('orphyn.theme.mode')).toBe('dark')

    const accents = screen.getByRole('radiogroup', { name: 'Accent colour' })
    await user.click(within(accents).getByRole('radio', { name: 'Blue' }))
    expect(localStorage.getItem('orphyn.theme.accent')).toBe('blue')
    expect(within(accents).getByRole('radio', { name: 'Blue' })).toBeChecked()

    await user.click(screen.getByRole('switch', { name: 'Follow artwork colour' }))
    expect(localStorage.getItem('orphyn.theme.followArtwork')).toBe('true')
  })

  it('explains when a custom colour was adjusted for legibility', async () => {
    await renderPane()
    act(() => themeStore.setAccent('#ffff00'))
    expect(screen.getByText(/Adjusted to stay readable/)).toBeInTheDocument()
  })

  it('picks an interface style from the gallery', async () => {
    const user = userEvent.setup()
    await renderPane()
    const gallery = screen.getByRole('radiogroup', { name: 'Interface style' })
    const cards = within(gallery).getAllByRole('radio')
    expect(cards).toHaveLength(5)
    expect(within(gallery).getByRole('radio', { name: /^Classic/ })).toBeChecked()
    // Each card holds a live miniature carrying its own style.
    expect(gallery.querySelectorAll('.jr-style-preview__window[data-style]')).toHaveLength(5)

    await user.click(within(gallery).getByRole('radio', { name: /^Cinema/ }))
    expect(document.documentElement.dataset['style']).toBe('cinema')
    expect(localStorage.getItem('orphyn.interface.style')).toBe('cinema')
    expect(within(gallery).getByRole('radio', { name: /^Cinema/ })).toBeChecked()
    // Cinema's own defaults show in the options.
    const sidebar = screen.getByRole('radiogroup', { name: 'Sidebar' })
    expect(within(sidebar).getByRole('radio', { name: 'Icons only' })).toBeChecked()
  })

  it('applies every option at once, then resets to the style defaults', async () => {
    const user = userEvent.setup()
    await renderPane()
    const pick = async (group: string, option: string) =>
      user.click(
        within(screen.getByRole('radiogroup', { name: group })).getByRole('radio', {
          name: option,
        }),
      )

    const reset = screen.getByRole('button', { name: 'Reset to Style Defaults' })
    expect(reset).toBeDisabled()

    await pick('Corners', 'Round')
    await pick('Density', 'Compact')
    await pick('Text size', 'Extra large')
    await pick('Sidebar', 'Auto-hide')
    await pick('Artwork backdrop', 'Immersive')
    await pick('Video shelves', 'Landscape')
    await pick('Motion', 'Reduced')
    const d = document.documentElement.dataset
    expect(d).toMatchObject({
      corners: 'round',
      density: 'compact',
      textSize: 'xl',
      sidebar: 'auto-hide',
      backdrop: 'immersive',
      videoCards: 'landscape',
      motion: 'reduced',
    })

    const slider = screen.getByRole('slider', { name: 'Transparency' })
    slider.focus()
    await user.keyboard('{Home}')
    expect(d['transparency']).toBe('solid')
    expect(d['reduceTransparency']).toBe('true')

    expect(JSON.parse(localStorage.getItem('orphyn.interface.appearance') ?? '{}')).toMatchObject({
      corners: 'round',
      transparency: 0,
    })

    await user.click(reset)
    expect(d).toMatchObject({ corners: 'standard', sidebar: 'expanded', transparency: 'full' })
    expect(localStorage.getItem('orphyn.interface.appearance')).toBe('{}')
  })

  it('disables the transparency slider on Minimal, which is always solid', async () => {
    await renderPane()
    act(() => interfaceStyleStore.setStyle('minimal'))
    expect(screen.getByText('Minimal is always solid.')).toBeInTheDocument()
    expect(screen.getByRole('slider', { name: 'Transparency' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
  })
})
