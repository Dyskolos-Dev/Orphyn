/**
 * Launches the built Lumen app with Playwright's Electron driver and records everything that
 * could reveal a bug: renderer console errors / warnings (React warnings included), uncaught
 * exceptions, main-process stderr, failed or 4xx/5xx requests.
 */
import { mkdirSync, mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import {
  _electron as electron,
  expect,
  type ElectronApplication,
  type Page,
} from '@playwright/test'

export const ROOT = resolve(__dirname, '..', '..')
export const SCREENSHOTS = join(ROOT, 'e2e', 'screenshots')

export interface Credentials {
  url: string
  user: string
  password: string
}

/** Test server credentials, from the environment only (never committed). */
export function credentials(): Credentials | null {
  const url = process.env['LUMEN_E2E_URL']?.trim()
  const user = process.env['LUMEN_E2E_USER']?.trim()
  const password = process.env['LUMEN_E2E_PASSWORD']
  if (!url || !user || !password) return null
  return { url: url.replace(/\/+$/, ''), user, password }
}

export const SKIP_REASON =
  'LUMEN_E2E_URL, LUMEN_E2E_USER and LUMEN_E2E_PASSWORD are not set: e2e suite skipped.'

export function newProfileDir(): string {
  return mkdtempSync(join(tmpdir(), 'lumen-e2e-'))
}

export interface LogEntry {
  source: 'console' | 'pageerror' | 'main' | 'request'
  level: 'error' | 'warning' | 'info'
  text: string
}

export interface RunningApp {
  app: ElectronApplication
  page: Page
  logs: LogEntry[]
  close(): Promise<void>
}

/** Removes access tokens from URLs before they end up in a log or an assertion message. */
export function redact(text: string): string {
  return text.replace(/(api_?key=)[^&\s"']+/gi, '$1***').replace(/(Token=\\?")[^"\\]+/g, '$1***')
}

export interface LaunchOptions {
  profileDir: string
  /** Chromium UI language: drives `navigator.language` and so the "system" language choice. */
  lang?: 'en-US' | 'fr-FR'
}

export async function launchApp(opts: LaunchOptions): Promise<RunningApp> {
  const app = await electron.launch({
    args: [join(ROOT, 'out', 'main', 'index.js'), `--lang=${opts.lang ?? 'en-US'}`],
    cwd: ROOT,
    env: {
      ...process.env,
      LUMEN_USER_DATA_DIR: opts.profileDir,
      ELECTRON_ENABLE_LOGGING: '1',
      // Never load a dev server: the suite always tests the production build.
      ELECTRON_RENDERER_URL: '',
    },
  })
  const logs: LogEntry[] = []

  const proc = app.process()
  proc.stderr?.on('data', (chunk: Buffer) => {
    for (const line of chunk.toString().split(/\r?\n/)) {
      if (line.trim()) logs.push({ source: 'main', level: 'error', text: redact(line) })
    }
  })
  proc.stdout?.on('data', (chunk: Buffer) => {
    for (const line of chunk.toString().split(/\r?\n/)) {
      if (line.trim()) logs.push({ source: 'main', level: 'info', text: redact(line) })
    }
  })

  const page = await app.firstWindow()
  page.on('console', (msg) => {
    const type = msg.type()
    if (type === 'error' || type === 'warning') {
      logs.push({
        source: 'console',
        level: type === 'error' ? 'error' : 'warning',
        text: redact(msg.text()),
      })
    }
  })
  page.on('pageerror', (err) => {
    logs.push({ source: 'pageerror', level: 'error', text: redact(`${err.name}: ${err.message}`) })
  })
  page.on('requestfailed', (req) => {
    const failure = req.failure()?.errorText ?? 'failed'
    // Aborted requests are normal (query cancelled, media element reloaded).
    if (/ERR_ABORTED/.test(failure)) return
    logs.push({ source: 'request', level: 'error', text: redact(`${failure} ${req.url()}`) })
  })
  page.on('response', (res) => {
    if (res.status() >= 400) {
      logs.push({
        source: 'request',
        level: 'error',
        text: redact(`${res.status()} ${res.request().method()} ${res.url()}`),
      })
    }
  })

  await page.waitForLoadState('domcontentloaded')
  await page.setViewportSize({ width: 1440, height: 900 }).catch(() => undefined)
  return {
    app,
    page,
    logs,
    async close() {
      await app.close().catch(() => undefined)
    },
  }
}

/**
 * Asserts that nothing unexpected was logged. `allowed` patterns cover expected failures
 * (wrong password → 401, track without lyrics → 404...).
 */
export function expectCleanLogs(logs: LogEntry[], allowed: RegExp[] = []): void {
  const unexpected = logs.filter(
    (l) =>
      l.level !== 'info' &&
      !allowed.some((re) => re.test(l.text)) &&
      !ALWAYS_ALLOWED.some((re) => re.test(l.text)),
  )
  expect(
    unexpected.map((l) => `[${l.source}/${l.level}] ${l.text}`),
    'unexpected errors or warnings in the logs',
  ).toEqual([])
}

/** Noise that is not the app's doing. */
const ALWAYS_ALLOWED: RegExp[] = [
  // Chromium's own startup chatter on stderr (GPU / cache / DevTools).
  /DevTools listening on/,
  /^\s*$/,
  /gpu_(process_host|init|channel)|GpuProcessHost|ANGLE|dxgi|d3d11|viz_main/i,
  /disk_cache|cache_util|Gpu Cache Creation failed|Unable to move the cache/i,
  /CONSOLE\(\d+\)/, // ELECTRON_ENABLE_LOGGING echo of renderer console, already captured
  /Electron Security Warning/,
  /Autofill\.(enable|setAddresses)/,
  // Chromium echoes every failed request on the console without its URL; the `request` entries
  // already record them with the URL, so they are the ones checked.
  /^Failed to load resource: /,
]

/**
 * A page capture cannot see the native Mica / vibrancy material behind the transparent window, so
 * it would come out black. Paint the window surface colour instead, as the material looks.
 */
async function paintMaterial(page: Page): Promise<void> {
  await page.evaluate(() => {
    if (document.getElementById('jr-e2e-material')) return
    const style = document.createElement('style')
    style.id = 'jr-e2e-material'
    style.textContent =
      "html[data-material='mica'], html[data-material='vibrancy'] { background: var(--surface-window) !important; }"
    document.head.appendChild(style)
  })
}

export async function screenshot(page: Page, name: string): Promise<string> {
  mkdirSync(SCREENSHOTS, { recursive: true })
  await paintMaterial(page)
  const path = join(SCREENSHOTS, `${name}.png`)
  // Let transitions and image fades settle.
  await page.waitForTimeout(600)
  await page.screenshot({ path })
  return path
}

/**
 * Screenshots the current screen in light then dark (theme mode "system" follows the emulated
 * colour scheme), then restores light.
 */
export async function screenshotBothThemes(page: Page, name: string): Promise<void> {
  for (const scheme of ['light', 'dark'] as const) {
    await page.emulateMedia({ colorScheme: scheme })
    await screenshot(page, `${name}-${scheme}`)
  }
  await page.emulateMedia({ colorScheme: 'light' })
}
