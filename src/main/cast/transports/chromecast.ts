import castv2 from 'castv2-client'
import type { CastMedia } from '@shared/cast'
import { errorMessage, log } from '../log'
import type { CastTransport, TransportHandlers, TransportUpdate } from '../transport'

type Callback<T> = (err: Error | null, result?: T) => void

interface CastPlayerStatus {
  playerState?: 'IDLE' | 'BUFFERING' | 'PLAYING' | 'PAUSED'
  idleReason?: 'CANCELLED' | 'INTERRUPTED' | 'FINISHED' | 'ERROR'
  currentTime?: number
  media?: { duration?: number }
}

interface CastPlayer {
  on(event: string, cb: (...args: never[]) => void): void
  removeAllListeners(): void
  load(media: unknown, options: unknown, cb: Callback<CastPlayerStatus>): void
  getStatus(cb: Callback<CastPlayerStatus>): void
  play(cb: Callback<unknown>): void
  pause(cb: Callback<unknown>): void
  stop(cb: Callback<unknown>): void
  seek(sec: number, cb: Callback<unknown>): void
}

interface CastClient {
  connect(opts: { host: string; port: number }, cb: () => void): void
  launch(app: unknown, cb: Callback<CastPlayer>): void
  setVolume(v: { level?: number; muted?: boolean }, cb: Callback<unknown>): void
  getVolume(cb: Callback<{ level?: number; muted?: boolean }>): void
  on(event: string, cb: (...args: never[]) => void): void
  removeAllListeners(): void
  close(): void
}

function call<T>(fn: (cb: Callback<T>) => void): Promise<T | undefined> {
  return new Promise((resolve, reject) => {
    try {
      fn((err, res) => (err ? reject(err) : resolve(res)))
    } catch (e) {
      reject(e instanceof Error ? e : new Error(String(e)))
    }
  })
}

/** Builds the Cast media object (Music/Movie metadata, images) for the Default Media Receiver. */
export function buildCastMedia(media: CastMedia): Record<string, unknown> {
  const images = media.artworkUrl ? [{ url: media.artworkUrl }] : undefined
  const metadata =
    media.kind === 'video'
      ? { metadataType: 1, title: media.title, ...(images ? { images } : {}) }
      : {
          metadataType: 3,
          title: media.title,
          ...(media.artist ? { artist: media.artist } : {}),
          ...(media.album ? { albumName: media.album } : {}),
          ...(images ? { images } : {}),
        }
  return {
    contentId: media.url,
    contentType: media.mimeType,
    streamType: 'BUFFERED',
    ...(media.durationSec ? { duration: media.durationSec } : {}),
    metadata,
  }
}

export function mapCastStatus(s: CastPlayerStatus): TransportUpdate {
  const update: TransportUpdate = {}
  if (typeof s.currentTime === 'number') update.positionSec = s.currentTime
  if (typeof s.media?.duration === 'number') update.durationSec = s.media.duration
  switch (s.playerState) {
    case 'PLAYING':
      update.status = 'playing'
      break
    case 'PAUSED':
      update.status = 'paused'
      break
    case 'BUFFERING':
      update.status = 'buffering'
      break
    case 'IDLE':
      if (s.idleReason === 'FINISHED') update.ended = true
      else if (s.idleReason === 'ERROR') update.error = 'The device could not play this media'
      else update.status = 'idle'
      break
  }
  return update
}

export class ChromecastTransport implements CastTransport {
  private handlers: TransportHandlers | null = null
  private client: CastClient | null = null
  private player: CastPlayer | null = null
  private poll: ReturnType<typeof setInterval> | null = null
  private closed = false

  constructor(private readonly target: { host: string; port: number }) {}

  setHandlers(handlers: TransportHandlers): void {
    this.handlers = handlers
  }

  async connect(): Promise<void> {
    this.closed = false
    const client = new castv2.Client() as CastClient
    this.client = client
    client.on('error', ((err: Error) => {
      log.warn(`cast client error (${this.target.host}):`, errorMessage(err))
      this.fireClosed(err)
    }) as never)
    client.on('status', ((st: { volume?: { level?: number; muted?: boolean } }) => {
      if (st.volume) {
        this.handlers?.onUpdate({
          ...(typeof st.volume.level === 'number' ? { volume: st.volume.level } : {}),
          ...(typeof st.volume.muted === 'boolean' ? { muted: st.volume.muted } : {}),
        })
      }
    }) as never)
    await call<void>((cb) => client.connect(this.target, () => cb(null)))
    this.player =
      (await call<CastPlayer>((cb) => client.launch(castv2.DefaultMediaReceiver, cb))) ?? null
    if (!this.player) throw new Error('Default Media Receiver did not launch')
    this.attachPlayer(this.player)
  }

  private attachPlayer(player: CastPlayer): void {
    player.on('status', ((s: CastPlayerStatus) =>
      this.handlers?.onUpdate(mapCastStatus(s))) as never)
    player.on('close', (() => this.fireClosed(new Error('Receiver app closed'))) as never)
    this.poll = setInterval(() => {
      player.getStatus((err, st) => {
        if (err || !st) return // transient; the heartbeat reports a dead link
        this.handlers?.onUpdate(mapCastStatus(st))
      })
    }, 1000)
  }

  private fireClosed(err: Error): void {
    if (this.closed) return
    this.stopPoll()
    this.handlers?.onClosed(err)
  }

  private stopPoll(): void {
    if (this.poll) clearInterval(this.poll)
    this.poll = null
  }

  private requirePlayer(): CastPlayer {
    if (!this.player) throw new Error('Not connected')
    return this.player
  }

  async load(media: CastMedia): Promise<void> {
    const player = this.requirePlayer()
    await call<CastPlayerStatus>((cb) =>
      player.load(
        buildCastMedia(media),
        { autoplay: true, currentTime: media.startPositionSec ?? 0 },
        cb,
      ),
    )
  }

  async play(): Promise<void> {
    const p = this.requirePlayer()
    await call((cb) => p.play(cb))
  }

  async pause(): Promise<void> {
    const p = this.requirePlayer()
    await call((cb) => p.pause(cb))
  }

  async seek(positionSec: number): Promise<void> {
    const p = this.requirePlayer()
    await call((cb) => p.seek(positionSec, cb))
  }

  async stop(): Promise<void> {
    const p = this.requirePlayer()
    await call((cb) => p.stop(cb))
  }

  async setVolume(volume: number): Promise<void> {
    const c = this.client
    if (!c) throw new Error('Not connected')
    await call((cb) => c.setVolume({ level: volume }, cb))
  }

  async setMuted(muted: boolean): Promise<void> {
    const c = this.client
    if (!c) throw new Error('Not connected')
    await call((cb) => c.setVolume({ muted }, cb))
  }

  async close(): Promise<void> {
    this.closed = true
    this.stopPoll()
    this.handlers = null
    const { client, player } = this
    this.client = null
    this.player = null
    try {
      player?.removeAllListeners()
    } catch {
      // already torn down
    }
    try {
      client?.close()
    } catch (e) {
      log.warn('cast close failed:', errorMessage(e))
    }
    // Late socket errors after close must not crash the process.
    client?.on('error', (() => undefined) as never)
  }
}
