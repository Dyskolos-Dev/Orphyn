/**
 * One profile shared by the serial scenarios: onboarding signs in once, every later scenario
 * relaunches the app on the same user data directory (which also proves the session persists).
 */
import { expect, type Page } from '@playwright/test'
import { credentials, launchApp, newProfileDir, type Credentials, type RunningApp } from './app'

let profile: string | null = null

export function sharedProfile(): string {
  profile ??= process.env['ORPHYN_E2E_PROFILE'] || newProfileDir()
  return profile
}

export function requireCredentials(): Credentials {
  const c = credentials()
  if (!c) throw new Error('missing e2e credentials')
  return c
}

/** Fills the three-step connect flow (onboarding page or "Add Server" sheet). */
export async function connect(page: Page, c: Credentials, name: string): Promise<void> {
  await page.getByLabel('Server address').fill(c.url)
  await page.getByRole('button', { name: 'Connect' }).click()
  await expect(page.getByText('Server found')).toBeVisible()
  await page.getByRole('button', { name: 'Continue' }).click()
  await page.getByLabel('Username').fill(c.user)
  await page.getByLabel('Password', { exact: true }).fill(c.password)
  await page.getByRole('button', { name: 'Sign In' }).click()
  const nameField = page.getByLabel('Server name')
  await expect(nameField).toBeVisible()
  await nameField.fill(name)
  await page.getByRole('button', { name: 'Start Watching' }).click()
}

/** Launches the app on the shared profile, already signed in (onboarding ran first). */
export async function launchSignedIn(lang: 'en-US' | 'fr-FR' = 'en-US'): Promise<RunningApp> {
  const running = await launchApp({ profileDir: sharedProfile(), lang })
  await expect(running.page.locator('.jr-shell')).toBeVisible()
  await expect(running.page).not.toHaveURL(/onboarding/)
  return running
}
