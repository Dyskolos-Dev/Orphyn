import { afterEach, describe, expect, it } from 'vitest'
import { currentWebviewEngine, getShellInfo, mediaPolicyFor, restrictSupport } from '.'

describe('webview media policy', () => {
  afterEach(() => {
    globalThis.__ORPHYN_SHELL__ = undefined
  })

  it('is Chromium outside the Tauri shell', () => {
    expect(getShellInfo()).toBeNull()
    expect(currentWebviewEngine()).toBe('chromium')
  })

  it('follows the engine injected by the Tauri shell', () => {
    globalThis.__ORPHYN_SHELL__ = { platform: 'darwin', webview: 'wkwebview' }
    expect(currentWebviewEngine()).toBe('wkwebview')
  })

  it('keeps VP9 and Opus out of WKWebView and prefers transcoding on WebKitGTK', () => {
    const mac = mediaPolicyFor('wkwebview')
    expect(mac.nativeHls).toBe(true)
    expect(mac.mp4AudioCodecs).toContain('alac')
    expect(mac.mp4AudioCodecs).not.toContain('opus')
    const all = { h264: true, hevc: true, vp9: true, av1: true, ac3: true }
    expect(restrictSupport(all, mac)).toMatchObject({ hevc: true, vp9: false })
    const linux = mediaPolicyFor('webkitgtk')
    expect(linux.preferTranscode).toBe(true)
    expect(restrictSupport(all, linux)).toEqual({
      h264: true,
      hevc: false,
      vp9: false,
      av1: false,
      ac3: false,
    })
    expect(restrictSupport(all, mediaPolicyFor('webview2'))).toEqual(all)
  })
})
