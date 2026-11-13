import { EventEmitter } from 'node:events'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { canApplyInApp, UpdaterController, type UpdaterBackend, type UpdaterEnv } from './state'

class FakeBackend extends EventEmitter {
  autoDownload = true
  autoInstallOnAppQuit = false
  checkForUpdates = vi.fn(() => Promise.resolve(null))
  quitAndInstall = vi.fn()
}

function setup(over: Partial<UpdaterEnv> = {}) {
  const backend = new FakeBackend()
  const env: UpdaterEnv = {
    canApplyInApp: true,
    isPackaged: true,
    openExternal: vi.fn(),
    loadAutoUpdate: () => true,
    saveAutoUpdate: vi.fn(),
    ...over,
  }
  const c = new UpdaterController(backend as unknown as UpdaterBackend, env, '1.0.0')
  return { backend, env, c }
}

afterEach(() => vi.useRealTimers())

describe('UpdaterController', () => {
  it('walks checking -> downloading -> ready on in-app platforms', async () => {
    const { backend, c } = setup()
    expect(backend.autoDownload).toBe(true)
    backend.emit('checking-for-update')
    expect(c.getState().status).toBe('checking')
    backend.emit('update-available', { version: '1.1.0' })
    backend.emit('download-progress', { percent: 41.6 })
    expect(c.getState()).toMatchObject({ status: 'downloading', percent: 42, version: '1.1.0' })
    backend.emit('update-downloaded', { version: '1.1.0' })
    expect(c.getState().status).toBe('ready')
    c.install()
    expect(backend.quitAndInstall).toHaveBeenCalled()
  })

  it('only notifies where the update cannot be applied (macOS, .deb)', () => {
    const { backend, env, c } = setup({ canApplyInApp: false })
    expect(backend.autoDownload).toBe(false)
    backend.emit('update-available', { version: '1.2.0' })
    expect(c.getState().status).toBe('available')
    c.openRelease()
    expect(env.openExternal).toHaveBeenCalledWith(
      'https://github.com/Dyskolos-Dev/Orphyn/releases/tag/v1.2.0',
    )
    c.install()
    expect(backend.quitAndInstall).not.toHaveBeenCalled()
  })

  it('reports up to date and errors', async () => {
    const { backend, c } = setup()
    backend.emit('update-not-available')
    expect(c.getState().status).toBe('up-to-date')
    backend.emit('error', new Error('boom'))
    expect(c.getState()).toMatchObject({ status: 'error', error: 'boom' })
    backend.checkForUpdates.mockRejectedValueOnce(new Error('offline'))
    await c.check()
    expect(c.getState().error).toBe('offline')
  })

  it('does not check or start timers when unpackaged', async () => {
    vi.useFakeTimers()
    const { backend, c } = setup({ isPackaged: false })
    c.start()
    await vi.advanceTimersByTimeAsync(10_000)
    await c.check()
    expect(backend.checkForUpdates).not.toHaveBeenCalled()
  })

  it('checks 10 s after launch then every 6 h, unless automatic updates are off', async () => {
    vi.useFakeTimers()
    const { backend, c } = setup()
    c.start()
    await vi.advanceTimersByTimeAsync(9_999)
    expect(backend.checkForUpdates).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(backend.checkForUpdates).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(6 * 60 * 60_000)
    expect(backend.checkForUpdates).toHaveBeenCalledTimes(2)
    c.setAutoUpdate(false)
    await vi.advanceTimersByTimeAsync(6 * 60 * 60_000)
    expect(backend.checkForUpdates).toHaveBeenCalledTimes(2)
    c.stop()
  })

  it('persists the toggle and ignores overlapping checks', async () => {
    const { backend, env, c } = setup()
    c.setAutoUpdate(false)
    expect(env.saveAutoUpdate).toHaveBeenCalledWith(false)
    backend.emit('checking-for-update')
    await c.check()
    expect(backend.checkForUpdates).not.toHaveBeenCalled()
  })

  it('detects platforms that self-update', () => {
    expect(canApplyInApp('win32', {})).toBe(true)
    expect(canApplyInApp('linux', { APPIMAGE: '/x.AppImage' })).toBe(true)
    expect(canApplyInApp('linux', {})).toBe(false)
    expect(canApplyInApp('darwin', {})).toBe(false)
  })
})
