import type { CastMedia } from '@shared/cast'

/** Partial device-side update pushed by a transport (poll result or device event). */
export interface TransportUpdate {
  status?: 'idle' | 'playing' | 'paused' | 'buffering'
  positionSec?: number
  durationSec?: number
  /** 0..1 */
  volume?: number
  muted?: boolean
  /** The current media finished naturally (not stopped by the user). */
  ended?: boolean
  /** The device reported a playback error (bad media, unsupported format). */
  error?: string
}

export interface TransportHandlers {
  onUpdate(update: TransportUpdate): void
  /** The link to the device dropped (device vanished, socket closed, repeated poll failures). */
  onClosed(err?: Error): void
}

/** One protocol connection to one device. All methods reject on failure and never throw synchronously. */
export interface CastTransport {
  setHandlers(handlers: TransportHandlers): void
  connect(): Promise<void>
  load(media: CastMedia): Promise<void>
  play(): Promise<void>
  pause(): Promise<void>
  seek(positionSec: number): Promise<void>
  stop(): Promise<void>
  setVolume(volume: number): Promise<void>
  setMuted(muted: boolean): Promise<void>
  /** Idempotent teardown: timers, sockets, listeners. */
  close(): Promise<void>
}

export function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${what} timed out after ${ms} ms`)), ms)
    promise.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e: unknown) => {
        clearTimeout(timer)
        reject(e instanceof Error ? e : new Error(String(e)))
      },
    )
  })
}
