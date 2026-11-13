import { expect, test, type CDPSession, type Locator, type Page } from '@playwright/test'
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

/** An album with at least four tracks, none of which has lyrics (the "no lyrics" state). */
async function pickAlbum(): Promise<{ album: Item; tracks: Item[] }> {
  const albums = await probe.items({
    includeItemTypes: 'MusicAlbum',
    sortBy: 'SortName',
    limit: 60,
    fields: 'ChildCount',
  })
  for (const album of albums) {
    const tracks = await probe.items({
      parentId: album.Id,
      includeItemTypes: 'Audio',
      sortBy: 'ParentIndexNumber,IndexNumber,SortName',
    })
    if (tracks.length >= 4 && tracks.every((t) => !(t as { HasLyrics?: boolean }).HasLyrics)) {
      return { album, tracks }
    }
  }
  throw new Error('no album with four tracks on the server')
}

/** Emulates a window size and display scale (deviceScaleFactor) inside the renderer. */
const sessions = new WeakMap<Page, CDPSession>()
async function emulate(page: Page, width: number, height: number, scale: number): Promise<void> {
  // The override lives as long as its session, so one session is kept per page.
  let cdp = sessions.get(page)
  if (!cdp) {
    cdp = await page.context().newCDPSession(page)
    sessions.set(page, cdp)
  }
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: scale,
    mobile: false,
  })
}
async function clearEmulation(page: Page): Promise<void> {
  await sessions.get(page)?.send('Emulation.clearDeviceMetricsOverride')
}

/** The mini player's seek slider value (seconds), as the user sees it. */
const miniPlayer = (page: Page) => page.locator('.jrm-mini')
const position = async (slider: Locator) => Number(await slider.getAttribute('aria-valuenow'))

test('music: home, albums, album page, playback, queue, Now Playing and lyrics', async () => {
  const { album, tracks } = await pickAlbum()
  const { page, logs, close } = await launchSignedIn()
  try {
    const sidebar = page.getByRole('navigation')
    await sidebar.getByRole('link', { name: 'Home' }).nth(1).click()
    await expect(page.getByRole('heading', { name: 'Listen Now' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Recently Added' })).toBeVisible()
    await screenshotBothThemes(page, 'music-home-en')

    await sidebar.getByRole('link', { name: 'Albums' }).click()
    await expect(page.getByRole('heading', { name: 'Albums', level: 1 })).toBeVisible()
    await screenshotBothThemes(page, 'music-albums-en')

    await sidebar.getByRole('link', { name: 'Artists' }).click()
    await expect(page.getByRole('heading', { name: 'Artists', level: 1 })).toBeVisible()
    await screenshotBothThemes(page, 'music-artists-en')

    await sidebar.getByRole('link', { name: 'Songs' }).click()
    await expect(page.getByRole('heading', { name: 'Songs', level: 1 })).toBeVisible()
    await screenshotBothThemes(page, 'music-songs-en')

    // Album page.
    await page.evaluate((id) => (location.hash = `#/music/album/${id}`), album.Id)
    await expect(page.getByText(album.Name, { exact: true }).first()).toBeVisible()
    const trackRows = page.locator('.jr-track-list li')
    await expect(trackRows).toHaveCount(tracks.length)
    await screenshotBothThemes(page, 'music-album-en')

    // Play the album: the audio actually advances.
    await expect(page.locator('main .jr-track-list li').first()).toBeVisible()
    await page.locator('main').getByRole('button', { name: 'Play', exact: true }).first().click()
    const mini = miniPlayer(page)
    await expect(mini).toContainText(tracks[0]!.Name)
    const seek = mini.getByRole('slider', { name: 'Seek' })
    await expect.poll(() => position(seek), { timeout: 30_000 }).toBeGreaterThan(1)
    const p1 = await position(seek)
    await expect.poll(() => position(seek), { timeout: 15_000 }).toBeGreaterThan(p1 + 1)
    await screenshotBothThemes(page, 'music-album-playing-en')

    // Mini player layout at several window widths: nothing may be pushed out of the window.
    for (const width of [1000, 1440, 1920]) {
      await page.setViewportSize({ width, height: 900 })
      await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width)
      const lastButton = mini.getByRole('button', { name: 'Up Next' })
      await expect(lastButton).toBeVisible()
      const fits = await mini.evaluate((el) => {
        const bar = el.getBoundingClientRect()
        const queue = el.querySelector('[aria-label="Up Next"]')!.getBoundingClientRect()
        return {
          noScroll: el.scrollWidth <= el.clientWidth,
          queueInside: queue.right <= window.innerWidth - 8 && queue.right <= bar.right,
          capsule: el.querySelector('.jrm-mini__capsule')!.getBoundingClientRect().height,
        }
      })
      expect(fits).toEqual({ noScroll: true, queueInside: true, capsule: 52 })
      await screenshotBothThemes(page, `music-mini-player-${width}-en`)
    }
    await page.setViewportSize({ width: 1440, height: 900 })

    // The three groups of the bar never intersect and never exceed it, on any width and scale.
    for (const scale of [1, 1.25, 1.5]) {
      for (let width = 800; width <= 2600; width += 100) {
        await emulate(page, width, 900, scale)
        await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width)
        const boxes = await mini.evaluate((el) => {
          const box = (sel: string) => {
            const r = el.querySelector(sel)!.getBoundingClientRect()
            return { left: r.left, right: r.right }
          }
          const bar = el.getBoundingClientRect()
          return {
            bar: { left: bar.left, right: bar.right },
            parts: [box('.jrm-mini__left'), box('.jrm-mini__capsule'), box('.jrm-mini__right')],
          }
        })
        const label = `width ${width} @${scale}x`
        const [left, capsule, right] = boxes.parts as [
          { left: number; right: number },
          { left: number; right: number },
          { left: number; right: number },
        ]
        expect(left.right, label).toBeLessThanOrEqual(capsule.left + 0.5)
        expect(capsule.right, label).toBeLessThanOrEqual(right.left + 0.5)
        for (const part of boxes.parts) {
          expect(part.left, label).toBeGreaterThanOrEqual(boxes.bar.left - 0.5)
          expect(part.right, label).toBeLessThanOrEqual(boxes.bar.right + 0.5)
        }
      }
    }
    await clearEmulation(page)
    await page.setViewportSize({ width: 1440, height: 900 })

    // Next / previous.
    const controls = mini.getByRole('group', { name: 'Playback controls' })
    await controls.getByRole('button', { name: 'Next' }).click()
    await expect(mini).toContainText(tracks[1]!.Name)
    await expect.poll(() => position(seek), { timeout: 30_000 }).toBeGreaterThan(0)
    await controls.getByRole('button', { name: 'Previous' }).click()
    // Previous within the first seconds goes to the previous track (otherwise it restarts).
    await expect(mini)
      .toContainText(tracks[0]!.Name, { timeout: 10_000 })
      .catch(async () => {
        await controls.getByRole('button', { name: 'Previous' }).click()
        await expect(mini).toContainText(tracks[0]!.Name)
      })

    // Shuffle and repeat toggle.
    const shuffle = controls.getByRole('button', { name: 'Shuffle' })
    await shuffle.click()
    await expect(shuffle).toHaveAttribute('aria-pressed', 'true')
    await shuffle.click()
    await expect(shuffle).toHaveAttribute('aria-pressed', 'false')
    await controls.getByRole('button', { name: 'Repeat: off' }).click()
    await expect(controls.getByRole('button', { name: 'Repeat: all' })).toBeVisible()
    await controls.getByRole('button', { name: 'Repeat: all' }).click()
    await expect(controls.getByRole('button', { name: 'Repeat: one' })).toBeVisible()
    await controls.getByRole('button', { name: 'Repeat: one' }).click()
    await expect(controls.getByRole('button', { name: 'Repeat: off' })).toBeVisible()

    // Now Playing sheet.
    await mini.getByRole('button', { name: 'Open Now Playing' }).click()
    const sheet = page.getByRole('dialog', { name: 'Now Playing' })
    await expect(sheet).toBeVisible()
    await expect(sheet).toContainText(tracks[0]!.Name)
    await screenshotBothThemes(page, 'music-now-playing-en')

    // The segmented control, artwork and play button share the window's vertical axis.
    for (const scale of [1, 1.25]) {
      for (const width of [1000, 1600, 2400]) {
        await emulate(page, width, 900, scale)
        await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width)
        await page.waitForTimeout(500)
        const axes = await sheet.evaluate((el) => {
          const mid = (node: Element | null) => {
            const r = node?.getBoundingClientRect()
            return r ? r.left + r.width / 2 : NaN
          }
          return {
            window: window.innerWidth / 2,
            tabs: mid(el.querySelector('.jrm-np__tabs')),
            art: mid(el.querySelector('.jrm-np__art')),
            play: mid(
              el.querySelector(
                '.jrm-np__main [aria-label="Pause"], .jrm-np__main [aria-label="Play"]',
              ),
            ),
          }
        })
        for (const key of ['tabs', 'art', 'play'] as const) {
          expect(
            Math.abs(axes[key] - axes.window),
            `${key} @${width} ${scale}x`,
          ).toBeLessThanOrEqual(2)
        }
      }
    }
    await clearEmulation(page)
    await page.setViewportSize({ width: 1440, height: 900 })

    // Queue.
    await sheet.getByRole('radio', { name: 'Up Next' }).click()
    const panel = sheet.getByRole('region', { name: 'Up Next' })
    await expect(panel).toContainText(tracks[1]!.Name)
    await screenshotBothThemes(page, 'music-queue-en')

    // Lyrics: this album has none, the empty state shows (and no request is wasted on a 404).
    await sheet.getByRole('radio', { name: 'Lyrics' }).click()
    await expect(sheet.getByRole('heading', { name: 'No lyrics' })).toBeVisible()
    await screenshotBothThemes(page, 'music-no-lyrics-en')

    // Pause from the sheet, then close it.
    await sheet.getByRole('button', { name: 'Pause' }).click()
    await expect(sheet.getByRole('button', { name: 'Play', exact: true })).toBeVisible()
    await sheet.getByRole('button', { name: 'Close' }).click()
    await expect(sheet).toBeHidden()

    expectCleanLogs(logs)
  } finally {
    await close()
  }
})

test('music: lyrics stay between the top bar and the window bottom, header stays clickable', async () => {
  const albums = await probe.items({
    includeItemTypes: 'MusicAlbum',
    searchTerm: '4h16',
    limit: 10,
  })
  const album = albums.find((a) => a.Name === '4h16') ?? albums[0]
  test.skip(!album, 'album 4h16 is not on this server')
  const { page, logs, close } = await launchSignedIn()
  try {
    await page.evaluate((id) => (location.hash = `#/music/album/${id}`), album!.Id)
    const row = page.locator('.jr-track-list li').filter({ hasText: '2s (feat. sanderscore)' })
    await expect(row.first()).toBeVisible()
    await row.first().dblclick()
    await miniPlayer(page).getByRole('button', { name: 'Open Now Playing' }).click()
    const sheet = page.getByRole('dialog', { name: 'Now Playing' })
    await sheet.getByRole('radio', { name: 'Lyrics' }).click()
    await expect(sheet.locator('.jrm-lyric').first()).toBeVisible({ timeout: 20_000 })
    await page.waitForTimeout(1500) // let the active line scroll into place

    for (const width of [1000, 2000]) {
      await emulate(page, width, 900, 1.25)
      await expect.poll(() => page.evaluate(() => window.innerWidth)).toBe(width)
      await page.waitForTimeout(600)
      const result = await page.evaluate(() => {
        const hit = (el: Element | null) => {
          const r = el?.getBoundingClientRect()
          if (!r) return null
          return document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)
        }
        const tabs = [...document.querySelectorAll('.jrm-np__tabs [role="radio"]')]
        const controls: Record<string, Element | null> = {
          player: tabs[0] ?? null,
          lyrics: tabs[1] ?? null,
          queue: tabs[2] ?? null,
          close: document.querySelector('.jrm-np__header [aria-label="Close"]'),
          cast: document.querySelector('.jrm-np__header-end button'),
        }
        const hits: Record<string, boolean> = {}
        for (const [name, el] of Object.entries(controls)) {
          const top = hit(el)
          hits[name] = !el ? name === 'cast' : !!top && (top === el || el.contains(top))
          if (el && top?.closest('.jrm-lyrics')) hits[name] = false
        }
        const box = document.querySelector('.jrm-lyrics')!.getBoundingClientRect()
        const header = document.querySelector('.jrm-np__header')!.getBoundingClientRect()
        return {
          hits,
          boxTop: box.top,
          headerBottom: header.bottom,
          boxBottom: box.bottom,
          h: innerHeight,
        }
      })
      for (const [name, ok] of Object.entries(result.hits)) {
        expect(ok, `${name} @${width}`).toBe(true)
      }
      expect(result.boxTop, `lyrics box top @${width}`).toBeGreaterThanOrEqual(result.headerBottom)
      expect(result.boxBottom, `lyrics box bottom @${width}`).toBeLessThanOrEqual(result.h - 31)
    }
    await clearEmulation(page)
    expectCleanLogs(logs)
  } finally {
    await close()
  }
})
