import type { BaseItemDto } from '@renderer/jellyfin'

/** A trailer the detail page can offer. */
export type TrailerSource =
  /** A file on the server, played in Orphyn's own player. */
  | { kind: 'local'; key: string; name: string; itemId: string }
  /** A YouTube video, played in the in-app modal. */
  | { kind: 'youtube'; key: string; name: string; videoId: string; embedUrl: string }
  /** Any other web page: opened in the default browser. */
  | { kind: 'external'; key: string; name: string; url: string }

const YOUTUBE_ID = /^[\w-]{11}$/

/** The video id of a youtube.com / youtu.be / embed / shorts link, else undefined. */
export function youtubeId(url: string | null | undefined): string | undefined {
  if (!url) return undefined
  let u: URL
  try {
    u = new URL(url.trim())
  } catch {
    return undefined
  }
  const host = u.hostname.replace(/^(www|m|music)\./, '')
  let id: string | null | undefined
  if (host === 'youtu.be') id = u.pathname.split('/')[1]
  else if (host === 'youtube.com' || host === 'youtube-nocookie.com') {
    if (u.pathname === '/watch') id = u.searchParams.get('v')
    else {
      const [, kind, rest] = u.pathname.split('/')
      if (kind === 'embed' || kind === 'shorts' || kind === 'v') id = rest
    }
  }
  return id && YOUTUBE_ID.test(id) ? id : undefined
}

/** Privacy-friendly embed: autoplay, no related videos from other channels, no cookies. */
export function youtubeEmbedUrl(videoId: string): string {
  const q = new URLSearchParams({
    autoplay: '1',
    rel: '0',
    modestbranding: '1',
    playsinline: '1',
  })
  return `https://www.youtube-nocookie.com/embed/${videoId}?${q}`
}

const isWebUrl = (url: string | null | undefined): url is string => /^https?:\/\//i.test(url ?? '')

/**
 * Every trailer of an item, best first: files on the server (they play in Orphyn), then the
 * YouTube links, then other links. Duplicates (same video listed twice) are dropped.
 */
export function pickTrailerSources(
  local: readonly BaseItemDto[] | undefined,
  remote: BaseItemDto['RemoteTrailers'],
): TrailerSource[] {
  const out: TrailerSource[] = []
  for (const t of local ?? []) {
    if (t.Id) out.push({ kind: 'local', key: `local:${t.Id}`, name: t.Name ?? '', itemId: t.Id })
  }
  const seen = new Set<string>()
  const others: TrailerSource[] = []
  for (const r of remote ?? []) {
    const url = r.Url?.trim()
    if (!isWebUrl(url)) continue
    const videoId = youtubeId(url)
    const key = videoId ? `youtube:${videoId}` : `url:${url}`
    if (seen.has(key)) continue
    seen.add(key)
    const name = r.Name ?? ''
    if (videoId) {
      out.push({ kind: 'youtube', key, name, videoId, embedUrl: youtubeEmbedUrl(videoId) })
    } else others.push({ kind: 'external', key, name, url })
  }
  return [...out, ...others]
}
