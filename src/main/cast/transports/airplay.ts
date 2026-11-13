import { randomUUID } from 'node:crypto'
import http from 'node:http'
import type { CastMedia } from '@shared/cast'
import { errorMessage, log } from '../log'
import type { CastTransport, TransportHandlers, TransportUpdate } from '../transport'

/**
 * Legacy AirPlay (HTTP video, port 7000): POST /play with Content-Location, GET /playback-info,
 * POST /rate, /scrub, /stop. Works on many Apple TVs (and some AirPlay-video TVs/receivers) that do
 * not require a pairing PIN. There is no remote volume control in this protocol. AirPlay 2 audio
 * (HomePod, AirPlay 2 speakers) needs HomeKit pairing + encrypted RTSP and is NOT supported.
 */
export interface AirplayPlaybackInfo {
  rate: number
  position: number
  duration: number
  readyToPlay: boolean
}

/** Extracts the few fields we need from the /playback-info XML plist (no plist dependency). */
export function parsePlaybackInfo(plist: string): AirplayPlaybackInfo {
  const num = (key: string): number => {
    const m = new RegExp(`<key>${key}</key>\\s*<(?:real|integer)>([^<]+)</(?:real|integer)>`).exec(
      plist,
    )
    return m ? Number(m[1]) : 0
  }
  return {
    rate: num('rate'),
    position: num('position'),
    duration: num('duration'),
    readyToPlay: /<key>readyToPlay<\/key>\s*<true\s*\/>/.test(plist),
  }
}

/** Body of POST /play (text/parameters). */
export function buildPlayBody(url: string, startFraction: number): string {
  return `Content-Location: ${url}\nStart-Position: ${startFraction}\n`
}

interface HttpResult {
  status: number
  body: string
}

export class AirplayTransport implements CastTransport {
  private handlers: TransportHandlers | null = null
  private timer: ReturnType<typeof setInterval> | null = null
  private failures = 0
  private closed = false
  private sawPlaying = false
  private ended = false
  private polling = false
  private readonly session = randomUUID()
  private lastRate = 1

  constructor(private readonly target: { host: string; port: number }) {}

  setHandlers(handlers: TransportHandlers): void {
    this.handlers = handlers
  }

  private request(
    method: string,
    path: string,
    body?: string,
    contentType?: string,
  ): Promise<HttpResult> {
    return new Promise((resolve, reject) => {
      const req = http.request(
        {
          host: this.target.host,
          port: this.target.port,
          path,
          method,
          timeout: 5000,
          headers: {
            'User-Agent': 'MediaControl/1.0',
            'X-Apple-Session-ID': this.session,
            ...(body !== undefined
              ? {
                  'Content-Type': contentType ?? 'text/parameters',
                  'Content-Length': Buffer.byteLength(body),
                }
              : {}),
          },
        },
        (res) => {
          const chunks: Buffer[] = []
          res.on('data', (c: Buffer) => chunks.push(c))
          res.on('error', reject)
          res.on('end', () =>
            resolve({ status: res.statusCode ?? 0, body: Buffer.concat(chunks).toString('utf8') }),
          )
        },
      )
      req.on('timeout', () => req.destroy(new Error(`AirPlay ${path} timed out`)))
      req.on('error', reject)
      req.end(body)
    })
  }

  private async ok(method: string, path: string, body?: string): Promise<HttpResult> {
    const res = await this.request(method, path, body)
    if (res.status >= 400) throw new Error(`AirPlay ${path} failed with HTTP ${res.status}`)
    return res
  }

  async connect(): Promise<void> {
    this.closed = false
    const res = await this.request('GET', '/server-info')
    if (res.status >= 400) throw new Error(`AirPlay server-info failed with HTTP ${res.status}`)
  }

  async load(media: CastMedia): Promise<void> {
    this.stopPolling()
    this.sawPlaying = false
    this.ended = false
    // Start-Position is a fraction (0..1) of the duration.
    const frac =
      media.startPositionSec && media.durationSec
        ? Math.min(0.999, media.startPositionSec / media.durationSec)
        : 0
    await this.ok('POST', '/play', buildPlayBody(media.url, frac))
    this.timer = setInterval(() => void this.poll(), 1000)
  }

  async play(): Promise<void> {
    await this.ok('POST', `/rate?value=${this.lastRate || 1}`)
  }

  async pause(): Promise<void> {
    await this.ok('POST', '/rate?value=0')
  }

  async seek(positionSec: number): Promise<void> {
    await this.ok('POST', `/scrub?position=${positionSec}`)
  }

  async stop(): Promise<void> {
    this.ended = true
    this.stopPolling()
    await this.ok('POST', '/stop')
  }

  setVolume(): Promise<void> {
    return Promise.reject(new Error('Volume control is not available over legacy AirPlay'))
  }

  setMuted(): Promise<void> {
    return Promise.reject(new Error('Mute is not available over legacy AirPlay'))
  }

  async close(): Promise<void> {
    this.closed = true
    this.stopPolling()
    this.handlers = null
  }

  private stopPolling(): void {
    if (this.timer) clearInterval(this.timer)
    this.timer = null
  }

  async poll(): Promise<void> {
    if (this.polling || this.closed) return
    this.polling = true
    try {
      const res = await this.request('GET', '/playback-info')
      if (this.closed) return
      this.failures = 0
      const info = parsePlaybackInfo(res.body)
      const update: TransportUpdate = { positionSec: info.position, durationSec: info.duration }
      if (info.rate > 0) {
        this.sawPlaying = true
        this.lastRate = info.rate
        update.status = 'playing'
      } else if (info.readyToPlay || info.duration > 0) {
        update.status = 'paused'
      } else if (this.sawPlaying && !this.ended) {
        // playback-info has no item any more: the item finished.
        this.ended = true
        this.stopPolling()
        this.handlers?.onUpdate({ ended: true })
        return
      }
      this.handlers?.onUpdate(update)
    } catch (err) {
      if (this.closed) return
      this.failures++
      log.warn(`airplay poll failed (${this.failures}/5):`, errorMessage(err))
      if (this.failures >= 5) {
        this.stopPolling()
        this.handlers?.onClosed(err instanceof Error ? err : new Error(String(err)))
      }
    } finally {
      this.polling = false
    }
  }
}
