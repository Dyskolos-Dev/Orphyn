import type { CastMedia } from '@shared/cast'
import { escapeXml } from './xml'

/** DLNA.ORG_FLAGS: streaming transfer mode, background transfer, connection stall, DLNA v1.5. */
const DLNA_FLAGS = '01700000000000000000000000000000'

export function dlnaProfileFor(mimeType: string): string | undefined {
  switch (mimeType.toLowerCase()) {
    case 'audio/mpeg':
    case 'audio/mp3':
      return 'MP3'
    case 'audio/mp4':
    case 'audio/x-m4a':
    case 'audio/aac':
      return 'AAC_ISO_320'
    default:
      return undefined
  }
}

/** protocolInfo "<protocol>:<network>:<contentFormat>:<additionalInfo>" for an http-get resource. */
export function buildProtocolInfo(mimeType: string): string {
  const profile = dlnaProfileFor(mimeType)
  const pn = profile ? `DLNA.ORG_PN=${profile};` : ''
  return `http-get:*:${mimeType}:${pn}DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=${DLNA_FLAGS}`
}

/** "H:MM:SS" (UPnP REL_TIME / duration format). */
export function formatUpnpTime(totalSec: number): string {
  const s = Math.max(0, Math.round(totalSec))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = s % 60
  return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
}

/** Parses "H:MM:SS(.fff)" into seconds; "NOT_IMPLEMENTED" and garbage give 0. */
export function parseUpnpTime(value: string | undefined): number {
  if (!value) return 0
  const m = /^(-?)(\d+):(\d{1,2}):(\d{1,2})(?:\.(\d+))?$/.exec(value.trim())
  if (!m || m[1] === '-') return 0
  return Number(m[2]) * 3600 + Number(m[3]) * 60 + Number(m[4]) + (m[5] ? Number(`0.${m[5]}`) : 0)
}

/** DIDL-Lite document for one item, with title/artist/album/albumArtURI and a protocolInfo resource. */
export function buildDidl(media: CastMedia): string {
  const klass =
    media.kind === 'video' ? 'object.item.videoItem.movie' : 'object.item.audioItem.musicTrack'
  const parts: string[] = [
    `<dc:title>${escapeXml(media.title)}</dc:title>`,
    `<upnp:class>${klass}</upnp:class>`,
  ]
  if (media.artist) {
    parts.push(`<dc:creator>${escapeXml(media.artist)}</dc:creator>`)
    parts.push(`<upnp:artist>${escapeXml(media.artist)}</upnp:artist>`)
  }
  if (media.album) parts.push(`<upnp:album>${escapeXml(media.album)}</upnp:album>`)
  if (media.artworkUrl) {
    parts.push(`<upnp:albumArtURI>${escapeXml(media.artworkUrl)}</upnp:albumArtURI>`)
  }
  const duration =
    media.durationSec && media.durationSec > 0
      ? ` duration="${formatUpnpTime(media.durationSec)}"`
      : ''
  parts.push(
    `<res protocolInfo="${escapeXml(buildProtocolInfo(media.mimeType))}"${duration}>${escapeXml(media.url)}</res>`,
  )
  return (
    '<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" ' +
    'xmlns:dc="http://purl.org/dc/elements/1.1/" ' +
    'xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" ' +
    'xmlns:dlna="urn:schemas-dlna-org:metadata-1-0/">' +
    `<item id="0" parentID="-1" restricted="1">${parts.join('')}</item></DIDL-Lite>`
  )
}
