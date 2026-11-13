import { PlaybackReporter, secondsToTicks, streamUrlForCast } from '@renderer/jellyfin'
import type { CastDevice, CastMedia, CastSessionState } from '@shared/cast'
import { getCastApi } from './api'
import { useCastState } from './store'

export interface VideoCastRequest {
  itemId: string
  title: string
  subtitle?: string
  artworkUrl?: string
  startSec: number
  durationSec: number
}

interface Active {
  deviceId: string
  itemId: string
  reporter: PlaybackReporter | null
  last: CastSessionState | null
  offs: (() => void)[]
}

let active: Active | null = null

export const activeVideoDeviceId = (): string | null => active?.deviceId ?? null

/** H.264/AAC HLS stream the receiver can read (original files are rarely playable on a TV). */
export function videoCastMedia(r: VideoCastRequest): CastMedia {
  return {
    url: streamUrlForCast(r.itemId, 'video', { transcode: true }),
    mimeType: 'application/x-mpegURL',
    kind: 'video',
    title: r.title,
    artist: r.subtitle,
    artworkUrl: r.artworkUrl,
    durationSec: r.durationSec || undefined,
    startPositionSec: r.startSec,
  }
}

function makeReporter(itemId: string, state: () => { position: number; paused: boolean }) {
  try {
    return new PlaybackReporter(
      {
        itemId,
        mediaSourceId: itemId,
        playSessionId: `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 10)}`,
        playMethod: 'Transcode',
      },
      () => {
        const s = state()
        return { positionTicks: secondsToTicks(s.position), isPaused: s.paused }
      },
    )
  } catch {
    return null // no active session
  }
}

/**
 * Starts the video on `device` at `request.startSec`. Rejects (and leaves nothing behind) when the
 * device refuses it. `onEnded` / `onFailure` are called once when the device finishes or gives up.
 */
export async function startVideoCast(
  device: CastDevice,
  request: VideoCastRequest,
  hooks: { onEnded(positionSec: number): void; onFailure(message: string): void },
): Promise<void> {
  const cast = getCastApi()
  if (!cast) throw new Error('Casting is not available')
  if (!device.capabilities.video) throw new Error('This device cannot play video')
  if (active) await stopVideoCast()

  const session: Active = {
    deviceId: device.id,
    itemId: request.itemId,
    reporter: null,
    last: null,
    offs: [],
  }
  session.reporter = makeReporter(request.itemId, () => ({
    position: session.last?.positionSec ?? request.startSec,
    paused: session.last?.status !== 'playing',
  }))
  session.offs.push(
    cast.onSessionChanged((s) => {
      if (s.deviceId !== device.id) return
      const was = session.last?.status
      session.last = s
      if (s.status === 'error') hooks.onFailure(s.error ?? 'Cast error')
      else if (was !== s.status && s.status === 'paused') void session.reporter?.pause()
      else if (was !== s.status && s.status === 'playing') void session.reporter?.resume()
    }),
    cast.onEnded((e) => {
      if (e.deviceId === device.id) hooks.onEnded(session.last?.durationSec ?? request.durationSec)
    }),
  )
  active = session
  try {
    await cast.load(device.id, videoCastMedia(request))
  } catch (e) {
    active = null
    session.offs.forEach((o) => o())
    await cast.disconnect(device.id).catch(() => undefined)
    throw e
  }
  void session.reporter?.start()
  useCastState.setState({
    target: { deviceId: device.id, name: device.name, kind: device.kind, scope: 'video' },
  })
}

/** Stops casting and returns the position the device had reached (null if nothing was cast). */
export async function stopVideoCast(): Promise<number | null> {
  const session = active
  if (!session) return null
  active = null
  const position = session.last?.positionSec ?? 0
  session.offs.forEach((o) => o())
  useCastState.setState({ target: null })
  const cast = getCastApi()
  await session.reporter?.stop()
  await cast?.disconnect(session.deviceId).catch(() => undefined)
  return position
}
