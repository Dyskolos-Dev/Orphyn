import { expect, test } from '@playwright/test'
import { SKIP_REASON, credentials, expectCleanLogs, screenshotBothThemes } from './support/app'
import { JellyfinProbe, type Item } from './support/jellyfin'
import { launchSignedIn, requireCredentials } from './support/session'

test.skip(!credentials(), SKIP_REASON)

test('cast picker and system media controls (media keys, OS media panel)', async () => {
  const probe = new JellyfinProbe(requireCredentials())
  await probe.login()
  const albums = await probe.items({
    includeItemTypes: 'MusicAlbum',
    sortBy: 'SortName',
    limit: 20,
  })
  let tracks: Item[] = []
  let album: Item | undefined
  for (const a of albums) {
    tracks = await probe.items({ parentId: a.Id, includeItemTypes: 'Audio', sortBy: 'IndexNumber' })
    if (tracks.length >= 2) {
      album = a
      break
    }
  }
  await probe.logout()
  if (!album) throw new Error('no album with two tracks')

  const { page, logs, close } = await launchSignedIn()
  try {
    // A media key or a button of the OS media panel (Windows SMTC, macOS Now Playing, Linux MPRIS)
    // reaches the app as the native core's `orphyn:media-controls:action` event: emit exactly
    // that event, as the native side does.
    const pressMediaKey = (type: string) =>
      page.evaluate(async (t) => {
        const internals = (
          window as unknown as {
            __TAURI_INTERNALS__: { invoke(cmd: string, args: unknown): Promise<unknown> }
          }
        ).__TAURI_INTERNALS__
        await internals.invoke('plugin:event|emit', {
          event: 'orphyn:media-controls:action',
          payload: { type: t },
        })
      }, type)
    // The app publishes to the OS through the native controls (one entry for the app), not
    // through the webview's own navigator.mediaSession.
    expect(await page.evaluate(() => typeof window.orphyn.mediaControls?.setMetadata)).toBe(
      'function',
    )

    await page.evaluate((id) => (location.hash = `#/music/album/${id}`), album.Id)
    await expect(page.locator('main .jr-track-list li').first()).toBeVisible()
    await page.locator('main').getByRole('button', { name: 'Play', exact: true }).first().click()
    const mini = page.locator('.jrm-mini')
    await expect(mini).toContainText(tracks[0]!.Name)
    // The transport button says "Pause" while the player plays, "Play" while it is paused.
    const playing = () =>
      mini
        .getByRole('button', { name: 'Pause', exact: true })
        .first()
        .isVisible()
    await expect.poll(playing).toBe(true)

    // Next / pause / play / previous as the media keys and the OS panel trigger them.
    await pressMediaKey('next')
    await expect(mini).toContainText(tracks[1]!.Name)
    await pressMediaKey('pause')
    await expect.poll(playing).toBe(false)
    await pressMediaKey('play')
    await expect.poll(playing).toBe(true)
    await pressMediaKey('toggle')
    await expect.poll(playing).toBe(false)
    await pressMediaKey('toggle')
    await expect.poll(playing).toBe(true)
    await pressMediaKey('previous')
    await expect(mini).toContainText(tracks[0]!.Name, { timeout: 15_000 })
    await pressMediaKey('pause')
    await expect.poll(playing).toBe(false)

    // Cast picker from the music player: "This computer" is listed and selected.
    await mini.getByRole('button', { name: 'Cast to a device' }).first().click()
    const picker = page.getByRole('dialog', { name: 'Play on' })
    await expect(picker).toBeVisible()
    const local = picker.getByRole('menuitemradio', { name: /This computer/ })
    await expect(local).toBeVisible()
    await expect(local).toHaveAttribute('aria-checked', 'true')
    await page.waitForTimeout(2500) // let discovery report what it finds
    // Still entirely inside the window once devices have been added to it.
    const box = await picker.boundingBox()
    const viewport = await page.evaluate(() => ({ w: innerWidth, h: innerHeight }))
    expect(box).not.toBeNull()
    expect(box!.y).toBeGreaterThanOrEqual(0)
    expect(box!.y + box!.height).toBeLessThanOrEqual(viewport.h)
    await screenshotBothThemes(page, 'cast-picker-music-en')
    await page.keyboard.press('Escape')
    await expect(picker).toBeHidden()

    expectCleanLogs(logs)
  } finally {
    await close()
  }
})
