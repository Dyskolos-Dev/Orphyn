import type { CastMedia } from '@shared/cast'
import { buildDidl, formatUpnpTime, parseUpnpTime } from '../didl'
import { errorMessage, log } from '../log'
import {
  AVTRANSPORT_SERVICE,
  RENDERING_CONTROL_SERVICE,
  soapCall,
  type SoapCallOptions,
} from '../soap'
import type { CastTransport, TransportHandlers, TransportUpdate } from '../transport'

export interface UpnpEndpoints {
  avTransportControlUrl: string
  renderingControlUrl?: string
}

export type SoapFn = (opts: SoapCallOptions) => Promise<Record<string, string>>

export interface UpnpOptions {
  soap?: SoapFn
  pollIntervalMs?: number
  /** Consecutive poll failures before the link is considered dead. */
  maxPollFailures?: number
}

/** Maps a UPnP TransportState to a session status ('stopped' = STOPPED / NO_MEDIA_PRESENT). */
export function mapTransportState(
  state: string,
): TransportUpdate['status'] | 'stopped' | undefined {
  switch (state.toUpperCase()) {
    case 'PLAYING':
      return 'playing'
    case 'PAUSED_PLAYBACK':
    case 'PAUSED_RECORDING':
      return 'paused'
    case 'TRANSITIONING':
      return 'buffering'
    case 'STOPPED':
    case 'NO_MEDIA_PRESENT':
      return 'stopped'
    default:
      return undefined
  }
}

export class UpnpTransport implements CastTransport {
  private handlers: TransportHandlers | null = null
  private timer: ReturnType<typeof setInterval> | null = null
  private polling = false
  private failures = 0
  private closed = false
  /** True once the device reported PLAYING for the current media: only then STOPPED means "ended". */
  private sawPlaying = false
  private ended = false
  private tick = 0
  private readonly soap: SoapFn
  private readonly pollMs: number
  private readonly maxFailures: number

  constructor(
    private readonly endpoints: UpnpEndpoints,
    opts: UpnpOptions = {},
  ) {
    this.soap = opts.soap ?? soapCall
    this.pollMs = opts.pollIntervalMs ?? 1000
    this.maxFailures = opts.maxPollFailures ?? 5
  }

  setHandlers(handlers: TransportHandlers): void {
    this.handlers = handlers
  }

  private av(
    action: string,
    args: Array<[string, string | number]> = [],
  ): Promise<Record<string, string>> {
    return this.soap({
      controlUrl: this.endpoints.avTransportControlUrl,
      serviceType: AVTRANSPORT_SERVICE,
      action,
      args: [['InstanceID', 0], ...args],
    })
  }

  private rc(
    action: string,
    args: Array<[string, string | number]> = [],
  ): Promise<Record<string, string>> {
    const controlUrl = this.endpoints.renderingControlUrl
    if (!controlUrl) return Promise.reject(new Error('Device has no RenderingControl service'))
    return this.soap({
      controlUrl,
      serviceType: RENDERING_CONTROL_SERVICE,
      action,
      args: [['InstanceID', 0], ...args],
    })
  }

  async connect(): Promise<void> {
    await this.av('GetTransportInfo')
    this.closed = false
    // Initial volume, best effort.
    void this.pollVolume()
  }

  async load(media: CastMedia): Promise<void> {
    this.stopPolling()
    this.sawPlaying = false
    this.ended = false
    // Some renderers refuse a new URI while playing; stop first and ignore a failure.
    await this.av('Stop').catch(() => undefined)
    await this.av('SetAVTransportURI', [
      ['CurrentURI', media.url],
      ['CurrentURIMetaData', buildDidl(media)],
    ])
    await this.av('Play', [['Speed', 1]])
    if (media.startPositionSec && media.startPositionSec > 1) {
      // Renderers accept Seek only once playing; failure is not fatal.
      await this.av('Seek', [
        ['Unit', 'REL_TIME'],
        ['Target', formatUpnpTime(media.startPositionSec)],
      ]).catch((e: unknown) => log.warn('initial seek failed:', errorMessage(e)))
    }
    this.startPolling()
  }

  async play(): Promise<void> {
    await this.av('Play', [['Speed', 1]])
  }

  async pause(): Promise<void> {
    await this.av('Pause')
  }

  async seek(positionSec: number): Promise<void> {
    await this.av('Seek', [
      ['Unit', 'REL_TIME'],
      ['Target', formatUpnpTime(positionSec)],
    ])
  }

  async stop(): Promise<void> {
    // A user stop is not an "ended": drop the flag before the device reports STOPPED.
    this.sawPlaying = false
    this.ended = true
    this.stopPolling()
    await this.av('Stop')
  }

  async setVolume(volume: number): Promise<void> {
    await this.rc('SetVolume', [
      ['Channel', 'Master'],
      ['DesiredVolume', Math.round(volume * 100)],
    ])
  }

  async setMuted(muted: boolean): Promise<void> {
    await this.rc('SetMute', [
      ['Channel', 'Master'],
      ['DesiredMute', muted ? 1 : 0],
    ])
  }

  async close(): Promise<void> {
    this.closed = true
    this.stopPolling()
    this.handlers = null
  }

  private startPolling(): void {
    this.stopPolling()
    this.failures = 0
    this.tick = 0
    this.timer = setInterval(() => void this.poll(), this.pollMs)
  }

  private stopPolling(): void {
    if (this.timer) clearInterval(this.timer)
    this.timer = null
  }

  /** One poll cycle; public for tests (drives the ended/closed logic without timers). */
  async poll(): Promise<void> {
    if (this.polling || this.closed) return
    this.polling = true
    try {
      const info = await this.av('GetTransportInfo')
      const pos = await this.av('GetPositionInfo')
      if (this.closed) return
      this.failures = 0
      const mapped = mapTransportState(info.CurrentTransportState ?? '')
      const update: TransportUpdate = {
        positionSec: parseUpnpTime(pos.RelTime),
        durationSec: parseUpnpTime(pos.TrackDuration),
      }
      if (mapped === 'stopped') {
        if (this.sawPlaying && !this.ended) {
          this.ended = true
          this.stopPolling()
          this.handlers?.onUpdate({ ended: true })
          return
        }
      } else if (mapped) {
        if (mapped === 'playing') this.sawPlaying = true
        update.status = mapped
      }
      this.handlers?.onUpdate(update)
      if (++this.tick % 5 === 0) void this.pollVolume()
    } catch (err) {
      if (this.closed) return
      this.failures++
      log.warn(`poll failed (${this.failures}/${this.maxFailures}):`, errorMessage(err))
      if (this.failures >= this.maxFailures) {
        this.stopPolling()
        this.handlers?.onClosed(err instanceof Error ? err : new Error(String(err)))
      }
    } finally {
      this.polling = false
    }
  }

  private async pollVolume(): Promise<void> {
    if (!this.endpoints.renderingControlUrl) return
    try {
      const vol = await this.rc('GetVolume', [['Channel', 'Master']])
      const mute = await this.rc('GetMute', [['Channel', 'Master']])
      if (this.closed) return
      const level = Number(vol.CurrentVolume)
      this.handlers?.onUpdate({
        ...(Number.isFinite(level) ? { volume: Math.min(1, Math.max(0, level / 100)) } : {}),
        muted: mute.CurrentMute === '1' || mute.CurrentMute?.toLowerCase() === 'true',
      })
    } catch {
      // volume control is optional on some renderers
    }
  }
}
