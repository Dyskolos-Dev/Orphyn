import { expect, test, type Page } from '@playwright/test'
import { SKIP_REASON, credentials, expectCleanLogs, screenshotBothThemes } from './support/app'
import { JellyfinProbe, type Item } from './support/jellyfin'
import { launchSignedIn, requireCredentials } from './support/session'

test.skip(!credentials(), SKIP_REASON)

let probe: JellyfinProbe
test.beforeAll(async () => {
  probe = new JellyfinProbe(requireCredentials())
  await probe.login()
})
test.afterAll(async () => {
  await probe?.logout()
})

async function search(page: Page, term: string): Promise<void> {
  const field = page.getByRole('searchbox', { name: 'Search Jellyra' })
  await field.fill(term)
  await expect(page).toHaveURL(/#\/search\?q=/)
  await expect(page.getByText(`Results for “${term}”`)).toBeVisible()
}

/** A song with lyrics (to see the lyrics view filled), else any song. */
async function pickSong(): Promise<Item & { HasLyrics?: boolean }> {
  const songs = (await probe.items({
    includeItemTypes: 'Audio',
    sortBy: 'SortName',
    limit: 1500,
  })) as (Item & { HasLyrics?: boolean })[]
  const song = songs.find((s) => s.HasLyrics && s.Name.length >= 4) ?? songs[0]
  if (!song) throw new Error('the server has no song')
  return song
}

test('search: a movie, an artist and a song, then play the song and read its lyrics', async () => {
  const [movie] = await probe.items({ includeItemTypes: 'Movie', sortBy: 'Random', limit: 1 })
  const artists = await probe.get<{ Items: Item[] }>('/Artists', {
    userId: probe.userId,
    sortBy: 'SortName',
    limit: 50,
  })
  const artist = artists.Items.find((a) => a.Name.length >= 4) ?? artists.Items[0]
  const song = await pickSong()
  if (!movie || !artist) throw new Error('the server lacks movies or artists')

  const { page, logs, close } = await launchSignedIn()
  try {
    // Empty state.
    await page.keyboard.press('Control+f')
    await expect(page.getByRole('searchbox', { name: 'Search Jellyra' })).toBeFocused()

    await search(page, movie.Name)
    const movies = page.locator('.jr-shelf', { has: page.getByRole('heading', { name: 'Movies' }) })
    await expect(
      page.getByRole('region', { name: 'Top Result' }).or(movies).getByText(movie.Name).first(),
    ).toBeVisible()
    await screenshotBothThemes(page, 'search-movie-en')

    await search(page, artist.Name)
    await expect(page.getByText(artist.Name, { exact: true }).first()).toBeVisible()
    await screenshotBothThemes(page, 'search-artist-en')

    await search(page, song.Name)
    const songs = page.getByRole('list', { name: 'Songs' })
    await expect(songs).toBeVisible()
    await screenshotBothThemes(page, 'search-song-en')

    // Play it from the results.
    await songs.locator('.jr-track').first().dblclick()
    const mini = page.locator('.jrm-mini')
    await expect(mini).toContainText(song.Name)
    const seek = mini.getByRole('slider', { name: 'Seek' })
    await expect
      .poll(async () => Number(await seek.getAttribute('aria-valuenow')), { timeout: 30_000 })
      .toBeGreaterThan(1)

    if (song.HasLyrics) {
      await mini.getByRole('button', { name: 'Lyrics' }).click()
      const sheet = page.getByRole('dialog', { name: 'Now Playing' })
      await expect(sheet.getByRole('region', { name: 'Lyrics' })).toBeVisible()
      await expect(sheet.getByRole('heading', { name: 'No lyrics' })).toBeHidden()
      await expect(sheet.locator('.jrm-lyric, .jrm-lyric-plain').first()).toBeVisible()
      await screenshotBothThemes(page, 'music-lyrics-en')
      await sheet.getByRole('button', { name: 'Pause' }).click()
      await sheet.getByRole('button', { name: 'Close' }).click()
    } else {
      await mini.getByRole('button', { name: 'Pause' }).click()
    }

    // No result.
    await search(page, 'zzqxwv-no-such-thing')
    await expect(page.getByText('No Results for “zzqxwv-no-such-thing”')).toBeVisible()
    await screenshotBothThemes(page, 'search-no-results-en')

    expectCleanLogs(logs)
  } finally {
    await close()
  }
})
