import type { UpdaterState } from '@shared/ipc/updater'

export const RELEASES_URL = 'https://github.com/Dyskolos-Dev/Lumen/releases'
export const FIRST_CHECK_DELAY_MS = 10_000
export const CHECK_INTERVAL_MS = 6 * 60 * 60_000

/** The slice of electron-updater's `AppUpdater` the controller uses (mockable in tests). */
export interface UpdaterBackend {
  autoDownload: boolean
  autoInstallOnAppQuit: boolean
  checkForUpdates(): Promise<unknown>
  quitAndInstall(): void
  on(event: string, listener: (...args: never[]) => void): unknown
}

export interface UpdaterEnv {
  /** Squirrel.Mac needs a signed app, and a .deb cannot replace itself: those only get notified. */
  canApplyInApp: boolean
  isPackaged: boolean
  openExternal(url: string): void
  loadAutoUpdate(): boolean
  saveAutoUpdate(enabled: boolean): void
}

/** True when the platform / package format can install an update by itself. */
export function canApplyInApp(platform: string, env: NodeJS.ProcessEnv = process.env): boolean {
  if (platform === 'win32') return true
  // Only an AppImage sets APPIMAGE; a .deb install does not update itself.
  if (platform === 'linux') return Boolean(env['APPIMAGE'])
  return false
}

export class UpdaterController {
  private state: UpdaterState
  private readonly listeners = new Set<(s: UpdaterState) => void>()
  private timers: NodeJS.Timeout[] = []

  constructor(
    private readonly backend: UpdaterBackend,
    private readonly env: UpdaterEnv,
    currentVersion: string,
  ) {
    this.state = {
      status: 'idle',
      currentVersion,
      version: null,
      percent: 0,
      error: null,
      autoUpdate: env.loadAutoUpdate(),
      supported: env.isPackaged,
    }
    const b = this.backend
    b.autoDownload = env.canApplyInApp
    b.autoInstallOnAppQuit = env.canApplyInApp
    b.on('checking-for-update', () => this.set({ status: 'checking', error: null }))
    b.on('update-available', ((info: { version: string }) =>
      this.set({
        status: this.env.canApplyInApp ? 'downloading' : 'available',
        version: info.version,
        percent: 0,
      })) as never)
    b.on('update-not-available', () => this.set({ status: 'up-to-date', version: null }))
    b.on('download-progress', ((p: { percent: number }) =>
      this.set({ status: 'downloading', percent: Math.round(p.percent) })) as never)
    b.on('update-downloaded', ((info: { version: string }) =>
      this.set({ status: 'ready', version: info.version, percent: 100 })) as never)
    b.on('error', ((err: Error) =>
      this.set({ status: 'error', error: err?.message ?? String(err) })) as never)
  }

  getState(): UpdaterState {
    return this.state
  }

  onChange(listener: (s: UpdaterState) => void): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  /** Manual or scheduled check. Ignored while a check or download runs. */
  async check(): Promise<void> {
    if (!this.state.supported) return
    if (this.state.status === 'checking' || this.state.status === 'downloading') return
    try {
      await this.backend.checkForUpdates()
    } catch (err) {
      this.set({ status: 'error', error: err instanceof Error ? err.message : String(err) })
    }
  }

  setAutoUpdate(enabled: boolean): void {
    this.env.saveAutoUpdate(enabled)
    this.set({ autoUpdate: enabled })
  }

  install(): void {
    if (this.state.status === 'ready') this.backend.quitAndInstall()
  }

  openRelease(): void {
    const v = this.state.version
    this.env.openExternal(v ? `${RELEASES_URL}/tag/v${v}` : `${RELEASES_URL}/latest`)
  }

  /** Check 10 s after launch, then every 6 h, when automatic updates are on. */
  start(): void {
    if (!this.state.supported) return
    const auto = () => {
      if (this.state.autoUpdate) void this.check()
    }
    this.timers.push(setTimeout(auto, FIRST_CHECK_DELAY_MS), setInterval(auto, CHECK_INTERVAL_MS))
  }

  stop(): void {
    this.timers.forEach((t) => {
      clearTimeout(t)
      clearInterval(t)
    })
    this.timers = []
  }

  private set(patch: Partial<UpdaterState>): void {
    this.state = { ...this.state, ...patch }
    this.listeners.forEach((l) => l(this.state))
  }
}
