import type { BaseItemDto } from '@renderer/jellyfin'
import { artistKey } from './artistGroups'

const MAX = 16

/**
 * Jellyfin's similar artists first, then the artists Deezer relates to this one that exist in the
 * library (matched by name). The artist itself and its merged ids are left out.
 */
export function mergeSimilar(
  fromServer: readonly BaseItemDto[],
  relatedNames: readonly string[],
  library: readonly BaseItemDto[],
  selfIds: readonly string[],
  selfName: string,
): BaseItemDto[] {
  const selfKey = artistKey(selfName)
  const out: BaseItemDto[] = []
  const seen = new Set<string>()
  const add = (a: BaseItemDto) => {
    const key = artistKey(a.Name ?? '')
    if (!a.Id || !key || key === selfKey || selfIds.includes(a.Id) || seen.has(key)) return
    seen.add(key)
    out.push(a)
  }
  for (const a of fromServer) if (a.Type === 'MusicArtist' || !a.Type) add(a)
  const byKey = new Map<string, BaseItemDto>()
  for (const a of library) {
    const key = artistKey(a.Name ?? '')
    if (key && !byKey.has(key)) byKey.set(key, a)
  }
  for (const name of relatedNames) {
    const hit = byKey.get(artistKey(name))
    if (hit) add(hit)
  }
  return out.slice(0, MAX)
}
