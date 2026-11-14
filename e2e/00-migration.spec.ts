import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from '@playwright/test'
import { SKIP_REASON, credentials, expectCleanLogs, launchApp, newProfileDir } from './support/app'
import { JellyfinProbe } from './support/jellyfin'
import { requireCredentials } from './support/session'

test.skip(!credentials(), SKIP_REASON)

/**
 * Updating from 1.0.x (Electron): the first start imports the servers of the Electron profile
 * (here `<profile>/electron/Lumen/accounts.json`, see src-tauri/src/profile.rs) without their
 * token, and each one asks to sign in again. Runs on its own profile.
 */
test('migration: servers saved by 1.0.x come back and ask to sign in again', async () => {
  const c = requireCredentials()
  const probe = new JellyfinProbe(c)
  await probe.login()
  const info = await probe.get<{ Id: string; ServerName: string; Version: string }>(
    '/System/Info/Public',
  )
  const userId = probe.userId
  await probe.logout()

  const profile = newProfileDir()
  const legacy = join(profile, 'electron', 'Lumen')
  mkdirSync(legacy, { recursive: true })
  // An accounts.json as Electron 1.0.x wrote it: the token is safeStorage ciphertext, unusable here.
  writeFileSync(
    join(legacy, 'accounts.json'),
    JSON.stringify({
      version: 1,
      deviceId: 'e2e-legacy-device',
      activeId: 'legacy-1',
      accounts: [
        {
          id: 'legacy-1',
          name: 'Living Room',
          url: c.url,
          serverId: info.Id,
          serverName: info.ServerName,
          version: info.Version,
          userId,
          userName: c.user,
          addedAt: '2026-11-01T10:00:00.000Z',
          lastUsedAt: '2026-11-13T10:00:00.000Z',
          accessToken: 'djEwAAAAciphertextFromElectronSafeStorage==',
        },
      ],
    }),
  )

  const { page, logs, close } = await launchApp({ profileDir: profile })
  try {
    // Imported with its name, user and device id; no token carried over.
    const stored = JSON.parse(readFileSync(join(profile, 'accounts.json'), 'utf8')) as {
      deviceId: string
      accounts: { name: string; userName: string; accessToken: string | null }[]
    }
    expect(stored.deviceId).toBe('e2e-legacy-device')
    expect(stored.accounts.map((a) => [a.name, a.userName, a.accessToken])).toEqual([
      ['Living Room', c.user, null],
    ])

    // First start of 1.1: the "What's new" sheet comes first.
    const whatsNew = page.getByRole('dialog', { name: /^What's new in Orphyn/ })
    await expect(whatsNew).toBeVisible()
    await whatsNew.getByRole('button', { name: 'Continue' }).click()
    await expect(whatsNew).toBeHidden()

    // Not sent to onboarding: Settings › Servers lists the server, which asks to sign in again.
    await expect(page).not.toHaveURL(/onboarding/)
    const servers = page.getByRole('list', { name: 'Your servers' })
    await expect(servers).toContainText('Living Room')
    await expect(servers).toContainText('Sign in required')
    await servers.getByRole('button', { name: 'Sign In Again' }).click()
    const sheet = page.getByRole('dialog', { name: 'Sign in again' })
    await expect(sheet).toBeVisible()
    await sheet.getByLabel('Password', { exact: true }).fill(c.password)
    await sheet.getByRole('button', { name: 'Sign In' }).click()
    await expect(sheet).toBeHidden()
    await expect(servers).not.toContainText('Sign in required')
    await expect(page.getByRole('button', { name: 'Active server: Living Room' })).toBeVisible()

    expectCleanLogs(logs, [/401/])
  } finally {
    await close()
  }
})
