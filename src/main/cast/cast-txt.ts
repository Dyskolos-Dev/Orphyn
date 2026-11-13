import type { CastDevice } from '@shared/cast'

export interface CastTxt {
  id?: string
  fn?: string
  md?: string
  /** Capability bitfield. */
  ca?: number
  rs?: string
}

const CA_VIDEO_OUT = 1
const CA_AUDIO_OUT = 4

/** Normalizes a Bonjour TXT record (strings or Buffers) into a lowercase-keyed string map. */
export function normalizeTxt(txt: unknown): Record<string, string> {
  const out: Record<string, string> = {}
  if (!txt || typeof txt !== 'object') return out
  for (const [k, v] of Object.entries(txt as Record<string, unknown>)) {
    const key = k.toLowerCase()
    if (typeof v === 'string') out[key] = v
    else if (Buffer.isBuffer(v)) out[key] = v.toString('utf8')
    else if (v === true) out[key] = ''
    else if (typeof v === 'number') out[key] = String(v)
  }
  return out
}

export function parseCastTxt(txt: unknown): CastTxt {
  const t = normalizeTxt(txt)
  const ca = t.ca !== undefined ? Number.parseInt(t.ca, 10) : undefined
  return {
    id: t.id,
    fn: t.fn,
    md: t.md,
    ca: ca !== undefined && Number.isFinite(ca) ? ca : undefined,
    rs: t.rs,
  }
}

/** Video capability from the ca bit field (bit 0 = video out); otherwise guessed from the model. */
export function castCapabilities(txt: CastTxt): CastDevice['capabilities'] {
  if (txt.ca !== undefined) {
    const video = (txt.ca & CA_VIDEO_OUT) !== 0
    const audio = (txt.ca & CA_AUDIO_OUT) !== 0
    return { audio: audio || video, video }
  }
  const md = txt.md ?? ''
  const audioOnly =
    /audio|home\s*(mini|max)?|nest\s*(mini|audio)|speaker|group|wiim|chromecast audio/i.test(md)
  return { audio: true, video: !audioOnly }
}

export function pickAddress(addresses: string[] | undefined): string | undefined {
  if (!addresses?.length) return undefined
  return addresses.find((a) => /^\d+\.\d+\.\d+\.\d+$/.test(a)) ?? addresses[0]
}

export interface MdnsService {
  name: string
  host?: string
  port: number
  txt?: unknown
  addresses?: string[]
}

export function castDeviceFromService(svc: MdnsService): CastDevice | null {
  const txt = parseCastTxt(svc.txt)
  const address = pickAddress(svc.addresses) ?? svc.host
  if (!address) return null
  return {
    id: `chromecast:${txt.id ?? svc.name}`,
    name: txt.fn || svc.name,
    kind: 'chromecast',
    model: txt.md ?? 'Google Cast',
    manufacturer: 'Google',
    capabilities: castCapabilities(txt),
    address,
    port: svc.port,
    wiim: /wiim|linkplay/i.test(txt.md ?? ''),
  }
}
