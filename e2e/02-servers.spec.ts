import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { SKIP_REASON, credentials, expectCleanLogs, screenshotBothThemes } from './support/app'
import { connect, launchSignedIn, requireCredentials, sharedProfile } from './support/session'

test.skip(!credentials(), SKIP_REASON)

interface StoredAccounts {
  activeId: string | null
  accounts: { id: string; name: string; userName: string; accessToken: string | null }[]
}
const readAccounts = (): StoredAccounts =>
  JSON.parse(readFileSync(join(sharedProfile(), 'accounts.json'), 'utf8')) as StoredAccounts

async function openServersPane(page: Page): Promise<void> {
  await page.getByRole('link', { name: 'Settings' }).click()
  await page.getByRole('navigation', { name: 'Settings sections' }).getByText('Servers').click()
  await expect(page.getByRole('list', { name: 'Your servers' })).toBeVisible()
}

test('servers: the session persists across restarts, with no token or password on disk', async () => {
  const c = requireCredentials()
  const stored = readAccounts()
  expect(stored.accounts).toHaveLength(1)
  const [account] = stored.accounts
  // The token lives in the OS keychain: accounts.json never holds it, in any form.
  expect(account!.accessToken).toBeNull()
  const raw = readFileSync(join(sharedProfile(), 'accounts.json'), 'utf8')
  expect(raw).not.toMatch(/"accessToken"\s*:\s*"/)
  expect(raw).not.toMatch(/"(password|pw)"/i)
  // (Only meaningful when the password differs from the user name, which is stored.)
  if (c.password !== c.user) expect(raw).not.toContain(c.password)

  const { page, logs, close } = await launchSignedIn()
  try {
    await expect(page.getByRole('button', { name: 'Active server: Home Server' })).toBeVisible()
    expectCleanLogs(logs)
  } finally {
    await close()
  }
})

test('servers: add the same server again, rename it, remove it, sign back in', async () => {
  const c = requireCredentials()
  const { page, logs, close } = await launchSignedIn()
  try {
    // Re-adding the same server and user updates the existing entry (Jellyfin revokes the
    // previous token of this device anyway) under the new name, and the session keeps working.
    await page.getByRole('button', { name: /^Active server:/ }).click()
    await page.getByRole('menuitem', { name: 'Add Server…' }).click()
    const sheet = page.getByRole('dialog', { name: 'Add a Server' })
    await expect(sheet).toBeVisible()
    await screenshotBothThemes(page, 'servers-add-sheet-en')
    await connect(page, c, 'Second Name')
    await expect(sheet).toBeHidden()
    await expect(page.getByRole('button', { name: 'Active server: Second Name' })).toBeVisible()
    expect(readAccounts().accounts.map((a) => a.name)).toEqual(['Second Name'])

    // Sign in again but abandon the sheet at the naming step: the new token is already the
    // only valid one, the app must keep working with it (no "sign in required").
    await page.getByRole('button', { name: /^Active server:/ }).click()
    await page.getByRole('menuitem', { name: 'Add Server…' }).click()
    await page.getByLabel('Server address').fill(c.url)
    await page.getByRole('button', { name: 'Connect' }).click()
    await page.getByRole('button', { name: 'Continue' }).click()
    await page.getByLabel('Username').fill(c.user)
    await page.getByLabel('Password', { exact: true }).fill(c.password)
    await page.getByRole('button', { name: 'Sign In' }).click()
    await expect(page.getByLabel('Server name')).toBeVisible({ timeout: 8000 })
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog', { name: 'Add a Server' })).toBeHidden()
    await page.getByRole('link', { name: 'Movies' }).click()
    await expect(page.getByText(/\d+ items/)).toBeVisible()
    await expect(page).toHaveURL(/#\/video\/library\/movies/)

    // Switcher menu lists the server, checked.
    await page.getByRole('button', { name: /^Active server:/ }).click()
    await expect(page.getByRole('menuitemradio', { name: 'Second Name' })).toHaveAttribute(
      'aria-checked',
      'true',
    )
    await screenshotBothThemes(page, 'servers-switcher-menu-en')
    await page.keyboard.press('Escape')

    // Rename from Settings › Servers.
    await openServersPane(page)
    await screenshotBothThemes(page, 'settings-servers-en')
    await page.getByRole('button', { name: 'Actions for Second Name' }).click()
    await page.getByRole('menuitem', { name: 'Rename' }).click()
    const rename = page.getByRole('dialog', { name: 'Rename Server' })
    await rename.getByLabel('Server name').fill('Salon')
    await rename.getByRole('button', { name: 'Save' }).click()
    await expect(rename).toBeHidden()
    await expect(page.getByRole('list', { name: 'Your servers' })).toContainText('Salon')
    await expect(page.getByRole('button', { name: 'Active server: Salon' })).toBeVisible()
    expect(readAccounts().accounts.map((a) => a.name)).toEqual(['Salon'])

    // Remove: signs out and goes back to onboarding (it was the only server).
    await page.getByRole('button', { name: 'Actions for Salon' }).click()
    await page.getByRole('menuitem', { name: 'Sign Out and Remove…' }).click()
    const confirm = page.getByRole('dialog', { name: 'Remove “Salon”?' })
    await expect(confirm).toBeVisible()
    await screenshotBothThemes(page, 'settings-remove-server-en')
    await confirm.getByRole('button', { name: 'Remove Server' }).click()
    await expect(page).toHaveURL(/#\/onboarding$/)
    expect(readAccounts().accounts).toEqual([])

    // Sign back in for the following scenarios.
    await connect(page, c, 'Home Server')
    await expect(page).toHaveURL(/#\/video/)
    expect(readAccounts().accounts.map((a) => a.name)).toEqual(['Home Server'])

    expectCleanLogs(logs)
  } finally {
    await close()
  }
})
