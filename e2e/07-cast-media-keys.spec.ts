import { expect, test } from '@playwright/test'
import { SKIP_REASON, credentials, expectCleanLogs, screenshotBothThemes } from './support/app'
import { JellyfinProbe, type Item } from './support/jellyfin'
import { launchSignedIn, requireCredentials } from './support/session'

test.skip(!credentials(), SKIP_REASON)

test('cast picker and system media controls (media keys / SMTC metadata)', async () => {
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

  const { app, page, logs, close } = await launchSignedIn()
  try {
    // The Chromium features behind media keys and the SMTC flyout are on.
    const features = await app.evaluate(({ app: electronApp }) =>
      electronApp.commandLine.getSwitchValue('enable-features'),
    )
    expect(features.split(',')).toEqual(
      expect.arrayContaining(['HardwareMediaKeyHandling', 'MediaSessionService']),
    )

    // Record the action handlers the app gives the media session: a media key or a button of
    // the Windows flyout makes Chromium call exactly these.
    await app.context().addInitScript(() => {
      const handlers: Record<string, ((d: unknown) => void) | null> = {}
      ;(window as unknown as { __mediaHandlers: typeof handlers }).__mediaHandlers = handlers
      const original = navigator.mediaSession.setActionHandler.bind(navigator.mediaSession)
      navigator.mediaSession.setActionHandler = (action, handler) => {
        handlers[action] = handler as ((d: unknown) => void) | null
        original(action, handler)
      }
    })
    await page.reload()
    await expect(page.locator('.jr-shell')).toBeVisible()
    const pressMediaKey = (action: string) =>
      page.evaluate((a) => {
        const h = (window as unknown as { __mediaHandlers: Record<string, (d: unknown) => void> })
          .__mediaHandlers[a]
        if (!h) throw new Error(`no media session handler for ${a}`)
        h({ action: a })
      }, action)

    await page.evaluate((id) => (location.hash = `#/music/album/${id}`), album.Id)
    await expect(page.locator('main .jr-track-list li').first()).toBeVisible()
    await page.locator('main').getByRole('button', { name: 'Play', exact: true }).first().click()
    const mini = page.locator('.jrm-mini')
    await expect(mini).toContainText(tracks[0]!.Name)

    // What Windows shows in the media flyout comes from navigator.mediaSession.
    const session = () =>
      page.evaluate(() => ({
        title: navigator.mediaSession.metadata?.title ?? null,
        artist: navigator.mediaSession.metadata?.artist ?? null,
        album: navigator.mediaSession.metadata?.album ?? null,
        artwork: navigator.mediaSession.metadata?.artwork.length ?? 0,
        state: navigator.mediaSession.playbackState,
      }))
    await expect.poll(session).toMatchObject({ title: tracks[0]!.Name, state: 'playing' })
    const meta = await session()
    expect(meta.album).toBe(album.Name)
    expect(meta.artist).toBeTruthy()
    expect(meta.artwork).toBeGreaterThan(0)

    // Next / pause / play / previous as the media keys and the flyout trigger them.
    await pressMediaKey('nexttrack')
    await expect(mini).toContainText(tracks[1]!.Name)
    await expect.poll(session).toMatchObject({ title: tracks[1]!.Name })
    await pressMediaKey('pause')
    await expect.poll(session).toMatchObject({ state: 'paused' })
    await pressMediaKey('play')
    await expect.poll(session).toMatchObject({ state: 'playing' })
    await pressMediaKey('previoustrack')
    await expect.poll(session, { timeout: 15_000 }).toMatchObject({ title: tracks[0]!.Name })
    await pressMediaKey('pause')
    await expect.poll(session).toMatchObject({ state: 'paused' })

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
