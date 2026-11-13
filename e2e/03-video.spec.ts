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

/** A movie with several audio tracks and text subtitles: the richest playback case. */
async function pickMovie(): Promise<Item> {
  const movies = await probe.items({
    includeItemTypes: 'Movie',
    fields: 'MediaStreams',
    sortBy: 'SortName',
    limit: 200,
  })
  const rich = movies.find((m) => {
    const s = m.MediaStreams ?? []
    return (
      s.filter((x) => x.Type === 'Audio').length > 1 &&
      s.some((x) => x.Type === 'Subtitle' && x.IsTextSubtitleStream)
    )
  })
  const movie = rich ?? movies[0]
  if (!movie) throw new Error('the server has no movie')
  return movie
}

const video = (page: Page) => page.locator('video')
const currentTime = (page: Page) => video(page).evaluate((v: HTMLVideoElement) => v.currentTime)

/** Shows the auto-hiding player controls. */
async function revealControls(page: Page): Promise<void> {
  await page.mouse.move(700, 450)
  await page.mouse.move(720, 460)
}

test('video: home, library grid, movie detail, series seasons and episodes', async () => {
  const { page, logs, close } = await launchSignedIn()
  try {
    await page.getByRole('link', { name: 'Home' }).first().click()
    await expect(page.getByRole('heading', { name: 'Watch Now' })).toBeVisible()
    await expect(page.locator('.jr-shelf').first()).toBeVisible()
    await screenshotBothThemes(page, 'video-home-en')

    await page.getByRole('link', { name: 'Movies' }).click()
    await expect(page.getByRole('heading', { name: 'Movies', level: 1 })).toBeVisible()
    await expect(page.getByText(/\d+ items/)).toBeVisible()
    await screenshotBothThemes(page, 'video-library-movies-en')

    const movie = await pickMovie()
    await page.evaluate((id) => (location.hash = `#/video/item/${id}`), movie.Id)
    await expect(page.getByRole('heading', { name: movie.Name }).first()).toBeAttached()
    await expect(page.getByRole('button', { name: /^(Play|Resume)/ }).first()).toBeVisible()
    await screenshotBothThemes(page, 'video-movie-detail-en')

    await page.getByRole('link', { name: 'TV Shows' }).click()
    await expect(page.getByRole('heading', { name: 'TV Shows', level: 1 })).toBeVisible()
    await screenshotBothThemes(page, 'video-library-series-en')

    const [series] = await probe.items({
      includeItemTypes: 'Series',
      sortBy: 'SortName',
      limit: 1,
    })
    if (!series) throw new Error('the server has no series')
    await page.evaluate((id) => (location.hash = `#/video/item/${id}`), series.Id)
    await expect(page.getByRole('heading', { name: series.Name }).first()).toBeAttached()
    const episodes = page.getByRole('region', { name: 'Episodes' })
    await expect(episodes.getByRole('link').first()).toBeVisible()
    await screenshotBothThemes(page, 'video-series-detail-en')

    expectCleanLogs(logs)
  } finally {
    await close()
  }
})

test('video: play a movie, seek, switch audio and subtitles, resume is reported', async () => {
  const movie = await pickMovie()
  await probe.markUnplayed(movie.Id)
  const { page, logs, close } = await launchSignedIn()
  try {
    await page.evaluate((id) => (location.hash = `#/video/item/${id}`), movie.Id)
    await page
      .getByRole('button', { name: /^(Play|Resume)/ })
      .first()
      .click()
    await expect(page).toHaveURL(/#\/video\/play\//)
    await expect(video(page)).toBeAttached()

    // It actually plays: currentTime advances.
    await expect.poll(() => currentTime(page), { timeout: 60_000 }).toBeGreaterThan(1)
    const t0 = await currentTime(page)
    await page.waitForTimeout(3000)
    expect(await currentTime(page)).toBeGreaterThan(t0 + 1)
    await revealControls(page)
    await screenshotBothThemes(page, 'video-player-en')

    // Seek forward 10 s with the control.
    await revealControls(page)
    const beforeSeek = await currentTime(page)
    await page.getByRole('button', { name: 'Forward 10 seconds' }).click()
    await expect.poll(() => currentTime(page), { timeout: 30_000 }).toBeGreaterThan(beforeSeek + 8)

    // Seek far into the film with the keyboard-accessible slider: jump to ~10 minutes.
    await page.evaluate(() => {
      const v = document.querySelector('video')
      if (v) v.currentTime = 600
    })
    await expect.poll(() => currentTime(page), { timeout: 60_000 }).toBeGreaterThan(599)
    await expect.poll(() => currentTime(page), { timeout: 60_000 }).toBeGreaterThan(602)

    // Switch the audio track: playback restarts from (about) the same position.
    await revealControls(page)
    await page.getByRole('button', { name: 'Audio' }).click()
    const audioOptions = page.getByRole('dialog', { name: 'Audio' }).getByRole('radio')
    expect(await audioOptions.count()).toBeGreaterThan(1)
    await screenshotBothThemes(page, 'video-player-audio-menu-en')
    await audioOptions.nth(1).click()
    await expect(audioOptions.nth(1)).toBeHidden()
    await expect.poll(() => currentTime(page), { timeout: 60_000 }).toBeGreaterThan(595)
    const afterAudio = await currentTime(page)
    await expect.poll(() => currentTime(page), { timeout: 30_000 }).toBeGreaterThan(afterAudio + 1)

    // Turn a text subtitle on.
    await revealControls(page)
    await page.getByRole('button', { name: 'Subtitles' }).click()
    const subtitleOptions = page.getByRole('dialog', { name: 'Subtitles' }).getByRole('radio')
    await subtitleOptions.nth(1).click()
    await expect
      .poll(
        () =>
          video(page).evaluate((v: HTMLVideoElement) =>
            Array.from(v.textTracks).some((t) => t.mode === 'showing'),
          ),
        { timeout: 30_000 },
      )
      .toBe(true)
    await expect.poll(() => currentTime(page), { timeout: 30_000 }).toBeGreaterThan(595)
    await page.waitForTimeout(2000)
    const position = await currentTime(page)
    await screenshotBothThemes(page, 'video-player-subtitles-en')

    // Leave the player: the stop report carries the position.
    await revealControls(page)
    await page.getByRole('button', { name: 'Back', exact: true }).click()
    await expect(page).not.toHaveURL(/#\/video\/play\//)
    await expect
      .poll(async () => ((await probe.item(movie.Id)).UserData?.PlaybackPositionTicks ?? 0) / 1e7, {
        timeout: 20_000,
      })
      .toBeGreaterThan(position - 15)

    // The detail page now offers to resume, and resuming starts near that position.
    await page.evaluate((id) => (location.hash = `#/video/item/${id}`), movie.Id)
    const resume = page.getByRole('button', { name: /^Resume/ }).first()
    await expect(resume).toBeVisible()
    await screenshotBothThemes(page, 'video-movie-resume-en')
    await resume.click()
    await expect.poll(() => currentTime(page), { timeout: 60_000 }).toBeGreaterThan(position - 15)
    await revealControls(page)
    await page.getByRole('button', { name: 'Back', exact: true }).click()

    expectCleanLogs(logs, [
      // Jellyfin 10.11 race: when a transcode starts mid-file (track switch, resume) hls.js asks
      // for the init segment and the first media segment in parallel and the server may answer
      // 500 on the init segment; hls.js retries it a second later and playback goes on.
      /^500 GET .*\/hls1\/main\/-1\.mp4\?/,
    ])
  } finally {
    await close()
    await probe.markUnplayed(movie.Id)
  }
})
