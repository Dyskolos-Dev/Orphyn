// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { Discovery, type MdnsBrowserLike, type MdnsLike } from './discovery'
import type { MdnsService } from './cast-txt'
import { DeviceRegistry } from './registry'

const svc: MdnsService = {
  name: 'Chromecast-abc',
  port: 8009,
  addresses: ['192.168.1.20'],
  txt: { id: 'abc', fn: 'Living Room', md: 'Chromecast', ca: '5' },
}

/** Fake bonjour: like the real Browser, 'up' fires once; later announcements only land in `services`. */
function fakeMdns() {
  const known: MdnsService[] = []
  const handlers: Array<(s: MdnsService) => void> = []
  const mdns: MdnsLike = {
    find(opts, onup) {
      const isCast = opts.type === 'googlecast'
      const browser: MdnsBrowserLike = {
        on: () => browser,
        stop: () => undefined,
        update: () => undefined,
        get services() {
          return isCast ? known : []
        },
      }
      if (isCast) handlers.push(onup)
      return browser
    },
    destroy: () => undefined,
  }
  return {
    mdns,
    announce: (s: MdnsService) => {
      known.push(s)
      handlers.forEach((h) => h(s))
    },
  }
}

describe('Discovery mDNS expiry', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('keeps mDNS devices alive past the registry TTL while they stay on the network', async () => {
    const { mdns, announce } = fakeMdns()
    const registry = new DeviceRegistry(() => undefined, 90_000)
    const discovery = new Discovery(registry, { createMdns: () => mdns, enableSsdp: false })
    discovery.start()
    announce(svc) // the only 'up' the real Browser will ever emit for this service
    expect(registry.list()).toHaveLength(1)
    await vi.advanceTimersByTimeAsync(300_000)
    expect(registry.list().map((d) => d.name)).toEqual(['Living Room'])
    discovery.stop()
  })

  it('drops a device that left the network (no longer in browser.services)', async () => {
    const { mdns, announce } = fakeMdns()
    const registry = new DeviceRegistry(() => undefined, 90_000)
    const discovery = new Discovery(registry, { createMdns: () => mdns, enableSsdp: false })
    discovery.start()
    announce(svc)
    ;(mdns.find({ type: 'googlecast' }, () => undefined).services as MdnsService[]).length = 0
    await vi.advanceTimersByTimeAsync(120_000)
    expect(registry.list()).toHaveLength(0)
    discovery.stop()
  })
})
