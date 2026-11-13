import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, it } from 'vitest'
import { shortcutAction } from './app'
import { isDragTarget } from './dragRegions'
import { createTauriApi, installTauriBridge } from '.'

describe('Tauri bridge', () => {
  afterEach(() => {
    clearMocks()
    globalThis.__ORPHYN_SHELL__ = undefined
    document.body.innerHTML = ''
  })

  it('does nothing outside the Tauri shell', () => {
    expect(installTauriBridge()).toBe(false)
  })

  it('maps the OrphynApi onto Rust commands with camelCase arguments', async () => {
    const calls: Array<[string, unknown]> = []
    mockIPC((cmd, args) => {
      calls.push([cmd, args])
      if (cmd === 'accounts_get_device_id') return 'device-1'
      if (cmd === 'window_get_state') {
        return { isFullscreen: true, isMaximized: false, isFocused: true, material: 'mica' }
      }
      return null
    })
    const api = createTauriApi('win32')
    expect(api.platform).toBe('win32')
    expect(await api.accounts.getDeviceId()).toBe('device-1')
    expect(await api.window.isFullscreen()).toBe(true)
    await api.accounts.rename('a1', 'Home')
    await api.window.setFullscreen(false)
    await api.cast.seek('chromecast:1', 42)
    await api.updater.setAutoUpdate(false)
    expect(calls).toEqual([
      ['accounts_get_device_id', {}],
      ['window_get_state', {}],
      ['accounts_rename', { id: 'a1', name: 'Home' }],
      ['window_set_fullscreen', { fullscreen: false }],
      ['cast_seek', { deviceId: 'chromecast:1', positionSec: 42 }],
      ['updater_set_auto_update', { enabled: false }],
    ])
  })

  it('drags from .jr-drag areas but not from their controls', () => {
    document.body.innerHTML =
      '<div class="jr-drag"><span id="t">Title</span><button id="b"><svg id="i"></svg></button>' +
      '<div class="jr-no-drag"><span id="n"></span></div></div><p id="o"></p>'
    const el = (id: string) => document.getElementById(id)
    expect(isDragTarget(el('t'))).toBe(true)
    expect(isDragTarget(el('b'))).toBe(false)
    expect(isDragTarget(el('i'))).toBe(false)
    expect(isDragTarget(el('n'))).toBe(false)
    expect(isDragTarget(el('o'))).toBe(false)
  })

  it('maps the former menu accelerators per platform', () => {
    const key = (init: KeyboardEventInit) => new KeyboardEvent('keydown', init)
    expect(shortcutAction(key({ key: 'r', ctrlKey: true }), 'win32')).toBe('refresh')
    expect(shortcutAction(key({ key: ',', metaKey: true }), 'darwin')).toBe('openSettings')
    expect(shortcutAction(key({ key: 'r', ctrlKey: true }), 'darwin')).toBeNull()
    expect(shortcutAction(key({ key: 'r', ctrlKey: true, shiftKey: true }), 'linux')).toBeNull()
  })
})
