import { expect, test, type Page } from '@playwright/test'
import {
  SKIP_REASON,
  credentials,
  expectCleanLogs,
  launchApp,
  screenshotBothThemes,
} from './support/app'
import { connect, requireCredentials, sharedProfile } from './support/session'

test.skip(!credentials(), SKIP_REASON)

/** The logo, titles and fields share one vertical axis, centred in the window. */
async function expectCentred(page: Page): Promise<void> {
  await page.waitForTimeout(1500) // entrance animations
  const axes = await page.evaluate(() => {
    const mid = (el: Element | null) => {
      const r = el?.getBoundingClientRect()
      return r ? r.left + r.width / 2 : NaN
    }
    return {
      window: window.innerWidth / 2,
      logo: mid(document.querySelector('.jr-onboarding__logo')),
      welcome: mid(document.querySelector('.jr-onboarding__welcome')),
      field: mid(document.querySelector('.jr-connect input')),
      button: mid(document.querySelector('.jr-connect__actions button')),
    }
  })
  for (const key of ['logo', 'welcome', 'field', 'button'] as const) {
    expect(Math.abs(axes[key] - axes.window), key).toBeLessThanOrEqual(2)
  }
}

test('onboarding: bad address, wrong password, then success and naming the server', async () => {
  const c = requireCredentials()
  const { page, logs, close } = await launchApp({ profileDir: sharedProfile() })
  try {
    await expect(page).toHaveURL(/#\/onboarding$/)
    await expect(page.getByRole('heading', { name: 'Connect to your server' })).toBeVisible()
    // The title stays readable on the window surface in both themes (it used to be black on a
    // black immersive background in light mode).
    for (const scheme of ['light', 'dark'] as const) {
      await page.emulateMedia({ colorScheme: scheme })
      const colors = await page.evaluate(() => ({
        text: getComputedStyle(document.querySelector('h1')!).color,
        surface: getComputedStyle(document.querySelector('.jr-shell__main')!).backgroundColor,
      }))
      expect(colors.text).not.toBe(colors.surface)
    }
    await page.emulateMedia({ colorScheme: 'light' })
    await screenshotBothThemes(page, 'onboarding-address-en')
    await expectCentred(page)
    for (const width of [1000, 1920]) {
      await page.setViewportSize({ width, height: 900 })
      await expectCentred(page)
      await screenshotBothThemes(page, `onboarding-address-${width}-en`)
    }
    await page.setViewportSize({ width: 1440, height: 900 })

    // Unreachable address.
    const address = page.getByLabel('Server address')
    await address.fill('https://lumen-e2e.invalid')
    await page.getByRole('button', { name: 'Connect' }).click()
    await expect(page.getByText(/could not reach this server/)).toBeVisible({ timeout: 30_000 })
    await screenshotBothThemes(page, 'onboarding-unreachable-en')

    // Something that answers but is not Jellyfin.
    await address.fill(`${c.url}/not-a-jellyfin-base`)
    await page.getByRole('button', { name: 'Connect' }).click()
    await expect(page.getByText(/it is not a Jellyfin server/)).toBeVisible({ timeout: 30_000 })

    // A pasted web UI address is cut back to the server root. Then a wrong password.
    await address.fill(`${c.url}/web/#/home.html`)
    await page.getByRole('button', { name: 'Connect' }).click()
    await expect(page.getByText('Server found')).toBeVisible()
    await expectCentred(page)
    await screenshotBothThemes(page, 'onboarding-found-en')
    await page.getByRole('button', { name: 'Continue' }).click()
    await expect(page.getByLabel('Username')).toBeVisible()
    await expectCentred(page)
    await page.getByLabel('Username').fill(c.user)
    await page.getByLabel('Password', { exact: true }).fill(`${c.password}-wrong`)
    await page.getByRole('button', { name: 'Sign In' }).click()
    await expect(page.getByText('Wrong username or password.')).toBeVisible()
    // The password field is cleared after a failed attempt.
    await expect(page.getByLabel('Password', { exact: true })).toHaveValue('')
    await screenshotBothThemes(page, 'onboarding-wrong-password-en')

    // Back to the address step, then the full flow.
    await page.getByRole('button', { name: 'Back' }).click()
    await connect(page, c, 'Home Server')
    await expect(page).toHaveURL(/#\/video/)
    await expect(page.locator('.jr-shell')).toBeVisible()

    expectCleanLogs(logs, [
      // Expected failures of this scenario.
      /lumen-e2e\.invalid/,
      /\/not-a-jellyfin-base\//,
      /401 POST .*\/Users\/AuthenticateByName/,
    ])
  } finally {
    await close()
  }
})
