/**
 * `npm run dist`: release build and installers of the current OS (`tauri build`), with the
 * updater artifacts signed. The signing key comes from `TAURI_SIGNING_PRIVATE_KEY` (CI secret) or,
 * on a developer machine, from `~/.orphyn/updater.key` (never committed). Extra arguments are passed
 * to `tauri build` (for example `-- --target aarch64-apple-darwin`).
 */
import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

const env = { ...process.env }
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  const keyFile = join(homedir(), '.orphyn', 'updater.key')
  if (existsSync(keyFile)) {
    env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyFile, 'utf8').trim()
    env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= ''
  } else {
    console.error(
      'No updater signing key: set TAURI_SIGNING_PRIVATE_KEY or put the key in ~/.orphyn/updater.key.',
    )
    process.exit(1)
  }
}

const result = spawnSync('npx', ['tauri', 'build', ...process.argv.slice(2)], {
  stdio: 'inherit',
  env,
  shell: process.platform === 'win32',
})
process.exit(result.status ?? 1)
