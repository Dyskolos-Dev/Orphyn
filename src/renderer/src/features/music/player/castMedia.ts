import type { CastMedia } from '@shared/cast'
import { streamUrlForCast } from '@renderer/jellyfin'
import { getAudioMaxBitrate } from './quality'
import { currentItem } from './queue'
import { usePlayerStore } from './store'
import { trackArt, trackArtist, trackDuration, trackTitle } from './track'

/**
 * Cast description (device-reachable stream URL, metadata, artwork) of the track the player is
 * loading right now, or null when there is none / no session. Used by the cast feature's
 * RemoteAudioOutput.
 */
export function describeCurrentTrackForCast(): CastMedia | null {
  const entry = currentItem(usePlayerStore.getState())
  const track = entry?.track
  if (!track?.Id) return null
  let url: string
  try {
    url = streamUrlForCast(track.Id, 'audio', { maxBitrate: getAudioMaxBitrate() })
  } catch {
    return null // no active session
  }
  return {
    url,
    mimeType: 'audio/mpeg',
    kind: 'audio',
    title: trackTitle(track),
    artist: trackArtist(track) || undefined,
    album: track.Album ?? undefined,
    artworkUrl: trackArt(track, 640),
    durationSec: trackDuration(track) || undefined,
  }
}
