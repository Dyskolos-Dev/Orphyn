import { expect, test, type Page } from '@playwright/test'
import {
  SKIP_REASON,
  credentials,
  expectCleanLogs,
  screenshot,
  screenshotBothThemes,
} from './support/app'
import { JellyfinProbe } from './support/jellyfin'
import { launchSignedIn, requireCredentials } from './support/session'

test.skip(!credentials(), SKIP_REASON)

const storage = (page: Page, key: string) => page.evaluate((k) => localStorage.getItem(k), key)
const html = (page: Page, attr: string) =>
  page.evaluate((a) => document.documentElement.getAttribute(a), attr)

async function openPane(page: Page, name: string): Promise<void> {
  await page
    .getByRole('navigation', { name: /Settings sections|Sections des réglages/ })
    .getByText(name, { exact: true })
    .click()
  await expect(page.locator('.jr-settings__pane-title')).toHaveText(name)
}

async function pickValue(page: Page, menu: string, value: string): Promise<void> {
  await page.getByRole('button', { name: menu }).click()
  await page
    .getByRole('menu', { name: menu })
    .getByRole('menuitemcheckbox', { name: value })
    .click()
}

test('settings: appearance, accent colour and playback preferences', async () => {
  const { page, logs, close } = await launchSignedIn()
  try {
    await page.getByRole('link', { name: 'Settings' }).click()
    await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible()
    await openPane(page, 'General')
    await screenshotBothThemes(page, 'settings-general-en')

    // Appearance: explicit dark, then light, then back to following the system.
    const appearance = page.getByRole('radiogroup', { name: 'Appearance' })
    await appearance.getByRole('radio', { name: 'Dark' }).click()
    await expect.poll(() => html(page, 'data-theme')).toBe('dark')
    expect(await storage(page, 'lumen.theme.mode')).toBe('dark')
    await page.emulateMedia({ colorScheme: 'light' })
    expect(await html(page, 'data-theme')).toBe('dark') // the choice wins over the OS
    await appearance.getByRole('radio', { name: 'Light' }).click()
    await expect.poll(() => html(page, 'data-theme')).toBe('light')
    await appearance.getByRole('radio', { name: 'System' }).click()
    await page.emulateMedia({ colorScheme: 'dark' })
    await expect.poll(() => html(page, 'data-theme')).toBe('dark')
    await page.emulateMedia({ colorScheme: 'light' })
    await expect.poll(() => html(page, 'data-theme')).toBe('light')

    // Accent colour.
    const accents = page.getByRole('radiogroup', { name: 'Accent colour' })
    await accents.getByRole('radio', { name: 'Blue' }).click()
    await expect.poll(() => storage(page, 'lumen.theme.accent')).toBe('blue')
    await expect.poll(() => html(page, 'data-accent')).toBe('blue')
    await screenshotBothThemes(page, 'settings-accent-blue-en')
    await accents.getByRole('radio', { name: 'Red' }).click()
    await expect.poll(() => storage(page, 'lumen.theme.accent')).toBe('red')

    // Playback preferences.
    await openPane(page, 'Playback')
    await pickValue(page, 'Maximum streaming quality', '8 Mbps')
    await pickValue(page, 'Preferred audio language', 'French')
    await pickValue(page, 'Preferred subtitle language', 'English')
    await page.getByRole('switch', { name: 'Play next episode automatically' }).click()
    const prefs = JSON.parse((await storage(page, 'lumen.playback.preferences')) ?? '{}')
    expect(prefs).toMatchObject({
      maxVideoBitrate: 8_000_000,
      preferredAudioLanguage: 'fre',
      preferredSubtitleLanguage: 'eng',
      autoPlayNext: false,
    })
    await screenshotBothThemes(page, 'settings-playback-en')
    await page.getByRole('button', { name: 'Reset to Defaults' }).click()
    await expect
      .poll(async () => JSON.parse((await storage(page, 'lumen.playback.preferences')) ?? '{}'))
      .toMatchObject({ maxVideoBitrate: null, autoPlayNext: true })

    // Music quality.
    await openPane(page, 'Music')
    await pickValue(page, 'Streaming quality', '320 kbps')
    await expect
      .poll(async () => JSON.parse((await storage(page, 'lumen.playback.preferences')) ?? '{}'))
      .toMatchObject({ audioQuality: 320_000 })
    await screenshotBothThemes(page, 'settings-music-en')
    await pickValue(page, 'Streaming quality', 'Original')

    await openPane(page, 'About')
    await screenshotBothThemes(page, 'settings-about-en')

    expectCleanLogs(logs)
  } finally {
    await close()
  }
})

test('settings: French interface, applied at once and kept after a restart', async () => {
  const probe = new JellyfinProbe(requireCredentials())
  await probe.login()
  const [movie] = await probe.items({ includeItemTypes: 'Movie', sortBy: 'SortName', limit: 1 })
  const [series] = await probe.items({ includeItemTypes: 'Series', sortBy: 'SortName', limit: 1 })
  const [album] = await probe.items({
    includeItemTypes: 'MusicAlbum',
    sortBy: 'SortName',
    limit: 1,
  })
  await probe.logout()

  let running = await launchSignedIn()
  try {
    const { page } = running
    await page.getByRole('link', { name: 'Settings' }).click()
    await page
      .getByRole('radiogroup', { name: 'Language' })
      .getByRole('radio', { name: 'Français' })
      .click()
    await expect(page.getByRole('heading', { name: 'Réglages', level: 1 })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Films' })).toBeVisible()
    expect(await storage(page, 'lumen.language')).toBe('fr')
    expectCleanLogs(running.logs)
  } finally {
    await running.close()
  }

  // Restart: still French, even with an English OS.
  running = await launchSignedIn('en-US')
  try {
    const { page, logs } = running
    await expect(page.getByRole('link', { name: 'Réglages' })).toBeVisible()
    await screenshotBothThemes(page, 'video-home-fr')
    await page.getByRole('link', { name: 'Films' }).click()
    await expect(page.getByText(/\d+ éléments/)).toBeVisible()
    await screenshotBothThemes(page, 'video-library-movies-fr')
    if (movie) {
      await page.evaluate((id) => (location.hash = `#/video/item/${id}`), movie.Id)
      await expect(page.getByRole('button', { name: /^(Lire|Reprendre)/ }).first()).toBeVisible()
      await screenshotBothThemes(page, 'video-movie-detail-fr')
    }
    if (series) {
      await page.evaluate((id) => (location.hash = `#/video/item/${id}`), series.Id)
      await expect(
        page.getByRole('region', { name: 'Épisodes' }).getByRole('link').first(),
      ).toBeVisible()
      await screenshotBothThemes(page, 'video-series-detail-fr')
    }
    await page.getByRole('navigation').getByRole('link', { name: 'Accueil' }).nth(1).click()
    await expect(page.getByRole('heading', { name: 'À l’écoute' })).toBeVisible()
    await screenshotBothThemes(page, 'music-home-fr')
    if (album) {
      await page.evaluate((id) => (location.hash = `#/music/album/${id}`), album.Id)
      await expect(page.locator('.jr-track-list li').first()).toBeVisible()
      await screenshotBothThemes(page, 'music-album-fr')
    }
    await page.getByRole('link', { name: 'Réglages' }).click()
    await openPane(page, 'Lecture')
    await screenshotBothThemes(page, 'settings-playback-fr')
    await openPane(page, 'Serveurs')
    await screenshotBothThemes(page, 'settings-servers-fr')
    await openPane(page, 'Général')
    await screenshotBothThemes(page, 'settings-general-fr')
    await page.getByRole('searchbox').fill('zzqxwv')
    await expect(page.getByText(/Aucun résultat/)).toBeVisible()
    await screenshot(page, 'search-no-results-fr-light')

    // Back to following the system language for the other scenarios.
    await page.getByRole('link', { name: 'Réglages' }).click()
    await openPane(page, 'Général')
    await page
      .getByRole('radiogroup', { name: 'Langue' })
      .getByRole('radio', { name: 'Système' })
      .click()
    await expect(page.getByRole('link', { name: 'Settings' })).toBeVisible()
    expectCleanLogs(logs)
  } finally {
    await running.close()
  }
})
