/**
 * Launches the Tauri debug build of Orphyn (`npm run build:e2e`: `dist/tauri-target/debug/`) on a
 * throwaway profile and attaches Playwright to its WebView2 over the DevTools protocol
 * (`connectOverCDP`). Records everything that could reveal a bug: renderer console errors /
 * warnings (React warnings included), uncaught exceptions, warnings and errors of the native core,
 * failed or 4xx/5xx requests. Windows only: WebView2 is the webview with a DevTools port.
 */
import { spawn, type ChildProcess } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { chromium, expect, type Browser, type BrowserContext, type Page } from '@playwright/test'

export const ROOT = resolve(__dirname, '..', '..')
export const SCREENSHOTS = join(ROOT, 'e2e', 'screenshots')

/** The debug build with the frontend embedded (`tauri build --debug --no-bundle`). */
export const BINARY =
  process.env['ORPHYN_E2E_BINARY'] ||
  join(ROOT, 'dist', 'tauri-target', 'debug', process.platform === 'win32' ? 'Orphyn.exe' : 'Orphyn')

/** DevTools protocol port of the app's WebView2. */
export const CDP_PORT =
  Number(process.env['ORPHYN_E2E_CDP_PORT']) ||
  (Number(process.env['HANGAR_PORT_BASE']) ? Number(process.env['HANGAR_PORT_BASE']) + 10 : 9333)

export interface Credentials {
  url: string
  user: string
  password: string
}

/** Test server credentials, from the environment only (never committed). */
export function credentials(): Credentials | null {
  const url = process.env['ORPHYN_E2E_URL']?.trim()
  const user = process.env['ORPHYN_E2E_USER']?.trim()
  const password = process.env['ORPHYN_E2E_PASSWORD']
  if (!url || !user || !password) return null
  return { url: url.replace(/\/+$/, ''), user, password }
}

export const SKIP_REASON =
  'ORPHYN_E2E_URL, ORPHYN_E2E_USER and ORPHYN_E2E_PASSWORD are not set: e2e suite skipped.'

export function newProfileDir(): string {
  return mkdtempSync(join(tmpdir(), 'orphyn-e2e-'))
}

export interface LogEntry {
  /** `main`: the native core's log (its stdout / stderr). */
  source: 'console' | 'pageerror' | 'main' | 'request'
  level: 'error' | 'warning' | 'info'
  text: string
}

export interface RunningApp {
  browser: Browser
  context: BrowserContext
  page: Page
  logs: LogEntry[]
  /** The Picture-in-Picture window (`index.html?pip=music|video`), waiting for it to open. */
  pipWindow(): Promise<Page>
  close(): Promise<void>
}

/** Removes access tokens from URLs before they end up in a log or an assertion message. */
export function redact(text: string): string {
  return text.replace(/(api_?key=)[^&\s"']+/gi, '$1***').replace(/(Token=\\?")[^"\\]+/g, '$1***')
}

export interface LaunchOptions {
  profileDir: string
  /** WebView2 UI language: drives `navigator.language` and so the "system" language choice. */
  lang?: 'en-US' | 'fr-FR'
}

const isPip = (url: string) => /[?&]pip=/.test(url)
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

async function waitForCdp(port: number, proc: ChildProcess): Promise<void> {
  const deadline = Date.now() + 60_000
  while (Date.now() < deadline) {
    if (proc.exitCode !== null) throw new Error(`Orphyn exited early (code ${proc.exitCode})`)
    try {
      const res = await fetch(`http://127.0.0.1:${port}/json/version`)
      if (res.ok) return
    } catch {
      // not listening yet
    }
    await sleep(250)
  }
  throw new Error(`WebView2 DevTools port ${port} never opened`)
}

/** Native core log lines (tauri-plugin-log): `[date][time][target][LEVEL] message`. */
function nativeLevel(line: string): LogEntry['level'] {
  if (/\]\[ERROR\]/.test(line) || /panicked at/.test(line)) return 'error'
  if (/\]\[WARN\]/.test(line)) return 'warning'
  return 'info'
}

function watchPage(page: Page, logs: LogEntry[]): void {
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
}

export async function launchApp(opts: LaunchOptions): Promise<RunningApp> {
  if (!existsSync(BINARY)) throw new Error(`${BINARY} not found: run npm run build:e2e first`)
  const proc = spawn(BINARY, [], {
    cwd: ROOT,
    env: {
      ...process.env,
      ORPHYN_USER_DATA_DIR: opts.profileDir,
      ORPHYN_CDP_PORT: String(CDP_PORT),
      ORPHYN_LANG: opts.lang ?? 'en-US',
      RUST_BACKTRACE: '1',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  const logs: LogEntry[] = []
  const exited = new Promise<void>((r) => proc.once('exit', () => r()))
  const record = (chunk: Buffer) => {
    for (const line of chunk.toString().split(/\r?\n/)) {
      if (line.trim()) logs.push({ source: 'main', level: nativeLevel(line), text: redact(line) })
    }
  }
  proc.stdout?.on('data', record)
  proc.stderr?.on('data', record)

  let browser: Browser
  try {
    await waitForCdp(CDP_PORT, proc)
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${CDP_PORT}`)
  } catch (err) {
    proc.kill()
    throw err
  }
  const context = browser.contexts()[0]!
  const findMain = () =>
    context.pages().find((p) => !isPip(p.url()) && /^(https?|tauri):/.test(p.url()))
  let page = findMain()
  for (const deadline = Date.now() + 30_000; !page && Date.now() < deadline; ) {
    await sleep(100)
    page = findMain()
  }
  if (!page) {
    proc.kill()
    throw new Error('the main window never loaded')
  }
  const main = page
  watchPage(main, logs)
  context.on('page', (p) => watchPage(p, logs))

  await main.waitForLoadState('domcontentloaded')
  await main.setViewportSize({ width: 1440, height: 900 }).catch(() => undefined)
  return {
    browser,
    context,
    page: main,
    logs,
    async pipWindow() {
      // Polled: a new window's page is announced while its URL is still about:blank.
      for (const deadline = Date.now() + 15_000; Date.now() < deadline; ) {
        const open = context.pages().find((p) => isPip(p.url()))
        if (open) return open
        await sleep(100)
      }
      const pages = context.pages().map((p) => p.url())
      const native = logs.filter((l) => /pip/i.test(l.text)).map((l) => l.text)
      throw new Error(
        `no Picture-in-Picture window after 15 s\npages: ${pages.join(', ')}\nlog: ${native.join('\n')}`,
      )
    },
    async close() {
      // Close the window (not a kill) so WebView2 writes localStorage to the profile.
      await main.evaluate(() => void window.orphyn?.window.close()).catch(() => undefined)
      const timedOut = await Promise.race([exited.then(() => false), sleep(15_000).then(() => true)])
      if (timedOut) proc.kill()
      await browser.close().catch(() => undefined)
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
  /^\s*$/,
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
