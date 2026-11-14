import { expect, test, type Page } from '@playwright/test'
import { SKIP_REASON, credentials, expectCleanLogs, screenshotBothThemes } from './support/app'
import { JellyfinProbe, type Item } from './support/jellyfin'
import { launchSignedIn, requireCredentials } from './support/session'

test.skip(!credentials(), SKIP_REASON)

/** An album with at least two tracks, found through the API. */
async function findAlbum(probe: JellyfinProbe): Promise<{ album: Item; tracks: Item[] }> {
  const albums = await probe.items({ includeItemTypes: 'MusicAlbum', sortBy: 'SortName', limit: 20 })
  for (const album of albums) {
    const tracks = await probe.items({
      parentId: album.Id,
      includeItemTypes: 'Audio',
      sortBy: 'IndexNumber',
    })
    if (tracks.length >= 2) return { album, tracks }
  }
  throw new Error('no album with two tracks')
}

const openAlbum = async (page: Page, id: string) => {
  await page.evaluate((i) => (location.hash = `#/music/album/${i}`), id)
  await expect(page.locator('main .jr-track-list li').first()).toBeVisible()
}

test('playlists: create one, add an album to it, open it, delete it', async () => {
  const probe = new JellyfinProbe(requireCredentials())
  await probe.login()
  const { album, tracks } = await findAlbum(probe)
  const name = `E2E ${Date.now()}`
  const { page, logs, close } = await launchSignedIn()
  try {
    // Create from the Playlists page.
    await page.evaluate(() => (location.hash = '#/music/playlists'))
    await page.locator('main').getByRole('button', { name: 'New Playlist' }).click()
    const sheet = page.getByRole('dialog', { name: 'New Playlist' })
    await expect(sheet).toBeVisible()
    await sheet.getByLabel('Name', { exact: true }).fill(name)
    await sheet.getByRole('button', { name: 'Create' }).click()
    await expect(sheet).toBeHidden()
    await expect(page.getByText(`Created ${name}`)).toBeVisible()

    // Add the whole album from its "More" menu.
    await openAlbum(page, album.Id)
    await page.locator('main').getByRole('button', { name: 'More', exact: true }).first().click()
    await page.getByRole('menuitem', { name: 'Add to Playlist…' }).click()
    await page.getByRole('menu', { name: 'Add to Playlist…' }).getByRole('menuitem', { name }).click()
    await expect(page.getByText(`Added to ${name}`)).toBeVisible()

    // The server holds it, with the album's tracks.
    const [created] = await probe.items({ includeItemTypes: 'Playlist', searchTerm: name })
    expect(created, 'playlist on the server').toBeTruthy()
    await expect
      .poll(async () => (await probe.items({ parentId: created!.Id })).length)
      .toBe(tracks.length)

    // Open it from the sidebar: its tracks are listed.
    await page.getByRole('link', { name }).click()
    await expect(page.locator('main .jr-track-list li')).toHaveCount(tracks.length)
    await expect(page.locator('main')).toContainText(tracks[0]!.Name)
    await screenshotBothThemes(page, 'music-playlist-en')

    // Delete it from its page.
    await page.locator('main').getByRole('button', { name: 'More', exact: true }).first().click()
    await page.getByRole('menuitem', { name: 'Delete Playlist…' }).click()
    const confirm = page.getByRole('dialog', { name: `Delete “${name}”?` })
    await confirm.getByRole('button', { name: 'Delete' }).click()
    await expect(page.getByText(`Deleted ${name}`)).toBeVisible()
    await expect(page.getByRole('link', { name })).toBeHidden()
    await expect
      .poll(async () => (await probe.items({ includeItemTypes: 'Playlist', searchTerm: name })).length)
      .toBe(0)

    expectCleanLogs(logs)
  } finally {
    await close()
    // Never leave a test playlist behind, whatever failed.
    for (const p of await probe.items({ includeItemTypes: 'Playlist', searchTerm: 'E2E ' })) {
      await probe.send('DELETE', `/Items/${p.Id}`)
    }
    await probe.logout()
  }
})

test('favourites: an album marked from its page shows in Favourites, then unmarked', async () => {
  const probe = new JellyfinProbe(requireCredentials())
  await probe.login()
  const { album } = await findAlbum(probe)
  const wasFavorite = await probe.isFavorite(album.Id)
  if (wasFavorite) await probe.send('DELETE', `/UserFavoriteItems/${album.Id}?userId=${probe.userId}`)
  const { page, logs, close } = await launchSignedIn()
  try {
    await openAlbum(page, album.Id)
    const header = page.locator('main')
    await header.getByRole('button', { name: 'Favorite', exact: true }).first().click()
    await expect(header.getByRole('button', { name: 'Remove from Favorites' }).first()).toBeVisible()
    await expect.poll(() => probe.isFavorite(album.Id)).toBe(true)

    await page.evaluate(() => (location.hash = '#/music/favorites'))
    await page.locator('main').getByRole('radio', { name: /^Albums/ }).click()
    await expect(page.locator('main')).toContainText(album.Name)
    await screenshotBothThemes(page, 'music-favorites-en')

    await openAlbum(page, album.Id)
    await header.getByRole('button', { name: 'Remove from Favorites' }).first().click()
    await expect.poll(() => probe.isFavorite(album.Id)).toBe(false)

    expectCleanLogs(logs)
  } finally {
    await close()
    if (wasFavorite) await probe.send('POST', `/UserFavoriteItems/${album.Id}?userId=${probe.userId}`)
    await probe.logout()
  }
})

test('interface styles: each of the five applies at once and survives a restart', async () => {
  const styles = [
    ['Classic', 'classic'],
    ['Glass', 'glass'],
    ['Cinema', 'cinema'],
    ['Minimal', 'minimal'],
    ['Compact', 'compact'],
  ] as const
  let running = await launchSignedIn()
  try {
    const { page } = running
    await page.getByRole('link', { name: 'Settings' }).click()
    const gallery = page.getByRole('radiogroup', { name: 'Interface style' })
    await expect(gallery).toBeVisible()
    for (const [label, id] of styles) {
      const card = gallery.getByRole('radio', { name: new RegExp(`^${label}`) })
      await card.click()
      await expect(card).toHaveAttribute('aria-checked', 'true')
      await expect(page.locator('html')).toHaveAttribute('data-style', id)
      // The shell stays laid out: sidebar and content side by side, nothing pushed off-screen.
      const layout = await page.evaluate(() => {
        const main = document.querySelector('main')!.getBoundingClientRect()
        return { left: main.left, right: main.right, width: window.innerWidth }
      })
      expect(layout.left).toBeGreaterThanOrEqual(0)
      expect(layout.right).toBeLessThanOrEqual(layout.width + 1)
      await screenshotBothThemes(page, `settings-style-${id}-en`)
    }
    expectCleanLogs(running.logs)
  } finally {
    await running.close()
  }

  // Restart: the last choice (Compact) is kept, then back to Classic for the other scenarios.
  running = await launchSignedIn()
  try {
    const { page } = running
    await expect(page.locator('html')).toHaveAttribute('data-style', 'compact')
    await page.getByRole('link', { name: 'Settings' }).click()
    await page
      .getByRole('radiogroup', { name: 'Interface style' })
      .getByRole('radio', { name: /^Classic/ })
      .click()
    await expect(page.locator('html')).toHaveAttribute('data-style', 'classic')
    expectCleanLogs(running.logs)
  } finally {
    await running.close()
  }
})

test('picture-in-picture: the music mini window shows the track and drives the player', async () => {
  const probe = new JellyfinProbe(requireCredentials())
  await probe.login()
  const { album, tracks } = await findAlbum(probe)
  await probe.logout()
  const running = await launchSignedIn()
  const { page, logs } = running
  try {
    await openAlbum(page, album.Id)
    await page.locator('main').getByRole('button', { name: 'Play', exact: true }).first().click()
    const mini = page.locator('.jrm-mini')
    await expect(mini).toContainText(tracks[0]!.Name)

    await mini.getByRole('button', { name: 'Mini player (Picture in Picture)' }).click()
    const pip = await running.pipWindow()
    await pip.waitForLoadState('domcontentloaded')
    await expect(pip.getByText(tracks[0]!.Name)).toBeVisible()

    // Its buttons drive the main player.
    await pip.getByRole('button', { name: 'Next' }).click()
    await expect(mini).toContainText(tracks[1]!.Name)
    await expect(pip.getByText(tracks[1]!.Name)).toBeVisible()
    await pip.getByRole('button', { name: 'Pause' }).click()
    await expect(pip.getByRole('button', { name: 'Play' })).toBeVisible()

    // Closing it leaves the main window playing on its own.
    await pip.getByRole('button', { name: 'Close Picture in Picture' }).click()
    await expect.poll(() => running.context.pages().some((p) => /[?&]pip=/.test(p.url()))).toBe(false)
    await expect(mini).toContainText(tracks[1]!.Name)

    expectCleanLogs(logs)
  } finally {
    await running.close()
  }
})
