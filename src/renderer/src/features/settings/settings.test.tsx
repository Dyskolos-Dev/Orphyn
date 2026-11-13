import { act, render, renderHook, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import i18n from '@renderer/i18n'
import { themeStore } from '@renderer/design'
import {
  DEFAULT_PLAYBACK_PREFERENCES,
  PLAYBACK_PREFERENCES_KEY,
  languageMatches,
  usePlaybackPreferences,
} from './index'
import { reloadPlaybackPreferences } from './preferences'
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

describe('languageMatches', () => {
  it('treats ISO 639-2 aliases as the same language', () => {
    expect(languageMatches('fre', 'fra')).toBe(true)
    expect(languageMatches('GER', 'deu')).toBe(true)
    expect(languageMatches('eng', 'fre')).toBe(false)
    expect(languageMatches(null, 'eng')).toBe(false)
  })
})

describe('GeneralPane', () => {
  it('switches the language and the theme instantly and remembers both', async () => {
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
    expect((await screen.findAllByText('Apparence')).length).toBeGreaterThan(0)

    await user.click(screen.getByRole('radio', { name: 'Sombre' }))
    expect(document.documentElement.dataset['theme']).toBe('dark')
    expect(localStorage.getItem('orphyn.theme.mode')).toBe('dark')

    await user.click(screen.getByRole('radio', { name: 'Bleu' }))
    expect(localStorage.getItem('orphyn.theme.accent')).toBe('blue')

    await act(() => i18n.changeLanguage('en'))
    themeStore.setMode('system')
  })
})
