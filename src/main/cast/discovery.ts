import http from 'node:http'
import Bonjour from 'bonjour-service'
import { Client as SsdpClient } from 'node-ssdp'
import type { CastDevice } from '@shared/cast'
import { airplayDeviceFromService } from './airplay-txt'
import { castDeviceFromService, type MdnsService } from './cast-txt'
import { parseDeviceDescription } from './description'
import { errorMessage, log } from './log'
import type { DeviceRegistry } from './registry'
import type { UpnpEndpoints } from './transports/upnp'

const MEDIA_RENDERER = 'urn:schemas-upnp-org:device:MediaRenderer:1'
const RESCAN_MS = 20_000
const SWEEP_MS = 10_000
const MAX_DESCRIPTION_BYTES = 512 * 1024

export function fetchText(url: string, timeoutMs = 4000): Promise<string> {
  return new Promise((resolve, reject) => {
    let settled = false
    const done = (fn: () => void): void => {
      if (!settled) {
        settled = true
        fn()
      }
    }
    try {
      const req = http.get(url, { timeout: timeoutMs }, (res) => {
        if ((res.statusCode ?? 0) >= 400) {
          res.resume()
          done(() => reject(new Error(`HTTP ${res.statusCode}`)))
          return
        }
        const chunks: Buffer[] = []
        let size = 0
        res.on('data', (c: Buffer) => {
          size += c.length
          if (size > MAX_DESCRIPTION_BYTES) req.destroy(new Error('Description too large'))
          else chunks.push(c)
        })
        res.on('error', (e) => done(() => reject(e)))
        res.on('end', () => done(() => resolve(Buffer.concat(chunks).toString('utf8'))))
      })
      req.on('timeout', () => req.destroy(new Error('Description request timed out')))
      req.on('error', (e) => done(() => reject(e)))
    } catch (e) {
      reject(e instanceof Error ? e : new Error(String(e)))
    }
  })
}

/** The slice of bonjour-service's Browser/Bonjour that Discovery uses (injectable for tests). */
export interface MdnsBrowserLike {
  on(event: 'up' | 'down', cb: (s: MdnsService) => void): unknown
  stop(): void
  update(): void
  readonly services: MdnsService[]
}
export interface MdnsLike {
  find(opts: { type: string }, onup: (s: MdnsService) => void): MdnsBrowserLike
  destroy(): void
}
export interface DiscoveryOptions {
  createMdns?: (onError: (err: Error) => void) => MdnsLike
  enableSsdp?: boolean
  rescanMs?: number
  sweepMs?: number
}

/** Runs mDNS (Cast, AirPlay) and SSDP (UPnP MediaRenderer) discovery into a DeviceRegistry. */
export class Discovery {
  private bonjour: MdnsLike | null = null
  private watchers: Array<{ browser: MdnsBrowserLike; up: (s: MdnsService) => void }> = []
  private ssdp: SsdpClient | null = null
  private rescanTimer: ReturnType<typeof setInterval> | null = null
  private sweepTimer: ReturnType<typeof setInterval> | null = null
  private running = false
  private readonly upnpEndpoints = new Map<string, UpnpEndpoints>()
  /** SSDP LOCATION -> device id, to skip re-fetching descriptions for known devices. */
  private readonly locations = new Map<string, string>()
  private readonly inflight = new Set<string>()
  private readonly airplayIds = new Set<string>()

  constructor(
    private readonly registry: DeviceRegistry,
    private readonly opts: DiscoveryOptions = {},
  ) {}

  isRunning(): boolean {
    return this.running
  }

  getUpnpEndpoints(deviceId: string): UpnpEndpoints | undefined {
    return this.upnpEndpoints.get(deviceId)
  }

  start(): void {
    if (this.running) return
    this.running = true
    log.info('discovery started')
    this.startMdns()
    if (this.opts.enableSsdp !== false) this.startSsdp()
    this.rescanTimer = setInterval(() => this.rescan(), this.opts.rescanMs ?? RESCAN_MS)
    this.sweepTimer = setInterval(() => this.registry.sweep(), this.opts.sweepMs ?? SWEEP_MS)
  }

  stop(): void {
    if (!this.running) return
    this.running = false
    log.info('discovery stopped')
    if (this.rescanTimer) clearInterval(this.rescanTimer)
    if (this.sweepTimer) clearInterval(this.sweepTimer)
    this.rescanTimer = this.sweepTimer = null
    for (const { browser: b } of this.watchers) {
      try {
        b.stop()
      } catch (e) {
        log.warn('mdns browser stop failed:', errorMessage(e))
      }
    }
    this.watchers = []
    try {
      this.bonjour?.destroy()
    } catch (e) {
      log.warn('mdns destroy failed:', errorMessage(e))
    }
    this.bonjour = null
    try {
      this.ssdp?.stop()
    } catch (e) {
      log.warn('ssdp stop failed:', errorMessage(e))
    }
    this.ssdp = null
    this.inflight.clear()
    // Drop anything that expired while running; stale entries must not linger after stop.
    this.registry.sweep()
  }

  private rescan(): void {
    for (const { browser, up } of this.watchers) {
      // bonjour-service emits 'up' only for the first sighting of a service, so refresh the
      // registry's last-seen time from the services the browser still knows about.
      this.safe(() => {
        for (const s of browser.services) up(s)
      })
      try {
        browser.update()
      } catch (e) {
        log.warn('mdns update failed:', errorMessage(e))
      }
    }
    this.searchSsdp()
  }

  private startMdns(): void {
    try {
      const onError = (err: Error): void => log.warn('mdns error:', errorMessage(err))
      const bonjour: MdnsLike =
        this.opts.createMdns?.(onError) ?? (new Bonjour(undefined, onError) as unknown as MdnsLike)
      this.bonjour = bonjour
      const watch = (
        type: string,
        up: (s: MdnsService) => void,
        down: (s: MdnsService) => void,
      ): void => {
        const browser = bonjour.find({ type }, (s) => this.safe(() => up(s)))
        browser.on('up', (s) => this.safe(() => up(s)))
        browser.on('down', (s) => this.safe(() => down(s)))
        this.watchers.push({ browser, up })
      }
      watch(
        'googlecast',
        (s) => this.onCast(s),
        (s) => this.onCastDown(s),
      )
      watch(
        'airplay',
        (s) => this.onAirplay(s, 'airplay'),
        (s) => this.onAirplayDown(s, 'airplay'),
      )
      watch(
        'raop',
        (s) => this.onAirplay(s, 'raop'),
        (s) => this.onAirplayDown(s, 'raop'),
      )
    } catch (e) {
      log.error('mdns start failed:', errorMessage(e))
    }
  }

  private startSsdp(): void {
    try {
      const client = new SsdpClient({})
      this.ssdp = client
      client.on('response', (headers: Record<string, unknown>) => {
        this.safe(() => this.onSsdpResponse(headers))
      })
      void client
        .start()
        .then(() => this.searchSsdp())
        .catch((e: unknown) => log.warn('ssdp start failed:', errorMessage(e)))
    } catch (e) {
      log.error('ssdp start failed:', errorMessage(e))
    }
  }

  private searchSsdp(): void {
    try {
      void Promise.resolve(this.ssdp?.search(MEDIA_RENDERER)).catch((e: unknown) =>
        log.warn('ssdp search failed:', errorMessage(e)),
      )
    } catch (e) {
      log.warn('ssdp search failed:', errorMessage(e))
    }
  }

  /** Network callbacks must never throw into the event loop. */
  private safe(fn: () => void): void {
    try {
      fn()
    } catch (e) {
      log.warn('discovery handler failed:', errorMessage(e))
    }
  }

  private onCast(s: MdnsService): void {
    const device = castDeviceFromService(s)
    if (device) this.registry.upsert(device)
  }

  private onCastDown(s: MdnsService): void {
    const device = castDeviceFromService(s)
    if (device) this.registry.remove(device.id)
  }

  private onAirplay(s: MdnsService, source: 'airplay' | 'raop'): void {
    const device = airplayDeviceFromService(s, source)
    if (!device) return
    if (source === 'airplay') this.airplayIds.add(device.id)
    // _raop only announces an audio receiver: never let it overwrite the richer _airplay entry.
    else if (this.airplayIds.has(device.id)) return
    this.registry.upsert(device)
  }

  private onAirplayDown(s: MdnsService, source: 'airplay' | 'raop'): void {
    const device = airplayDeviceFromService(s, source)
    if (!device) return
    if (source === 'airplay') this.airplayIds.delete(device.id)
    else if (this.airplayIds.has(device.id)) return
    this.registry.remove(device.id)
  }

  private onSsdpResponse(headers: Record<string, unknown>): void {
    const location = typeof headers.LOCATION === 'string' ? headers.LOCATION : undefined
    if (!location) return
    const known = this.locations.get(location)
    if (known && this.registry.touch(known)) return
    if (this.inflight.has(location)) return
    this.inflight.add(location)
    void this.fetchRenderer(location)
      .catch((e: unknown) => log.warn(`description fetch failed (${location}):`, errorMessage(e)))
      .finally(() => this.inflight.delete(location))
  }

  private async fetchRenderer(location: string): Promise<void> {
    const xml = await fetchText(location)
    if (!this.running) return
    const desc = parseDeviceDescription(xml, location)
    if (!desc || !desc.avTransportControlUrl) return // not a usable MediaRenderer
    const url = new URL(location)
    const id = `upnp:${desc.udn || location}`
    const device: CastDevice = {
      id,
      name: desc.friendlyName || url.hostname,
      kind: 'upnp',
      model: desc.modelName,
      manufacturer: desc.manufacturer,
      // DLNA renderers are mostly audio; WiiM/LinkPlay are audio-only streamers.
      capabilities: {
        audio: true,
        video: !desc.isWiim && /tv|video|display/i.test(desc.deviceType + desc.modelName),
      },
      address: url.hostname,
      port: Number(url.port) || 80,
      wiim: desc.isWiim,
    }
    this.upnpEndpoints.set(id, {
      avTransportControlUrl: desc.avTransportControlUrl,
      renderingControlUrl: desc.renderingControlUrl,
    })
    this.locations.set(location, id)
    this.registry.upsert(device)
  }
}
