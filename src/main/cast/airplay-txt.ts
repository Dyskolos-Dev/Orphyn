import type { CastDevice } from '@shared/cast'
import { pickAddress, normalizeTxt, type MdnsService } from './cast-txt'

/** AirPlay "features" TXT value: "0xLOW,0xHIGH" or a single hex number, as a BigInt bit set. */
export function parseAirplayFeatures(value: string | undefined): bigint {
  if (!value) return 0n
  const parts = value.split(',').map((p) => p.trim())
  try {
    const low = BigInt(parts[0] ?? '0')
    const high = parts[1] ? BigInt(parts[1]) : 0n
    return low | (high << 32n)
  } catch {
    return 0n
  }
}

const FEATURE_VIDEO = 0n // bit 0: video (URL playback over HTTP)

/** "AA:BB:CC:DD:EE:FF" or "AABBCCDDEEFF@Name" -> "AABBCCDDEEFF". */
export function normalizeMac(value: string): string {
  return value
    .split('@')[0]!
    .replace(/[^0-9a-fA-F]/g, '')
    .toUpperCase()
}

/**
 * Builds a device from `_airplay._tcp` (full support check) or `_raop._tcp` (audio receiver
 * announcement only). Only `_airplay` devices advertising the video/URL-play feature and no
 * password are marked playable; everything else is listed with no capabilities so the UI can
 * explain why it cannot be used.
 */
export function airplayDeviceFromService(
  svc: MdnsService,
  source: 'airplay' | 'raop',
): CastDevice | null {
  const txt = normalizeTxt(svc.txt)
  const address = pickAddress(svc.addresses) ?? svc.host
  if (!address) return null
  const mac = normalizeMac(source === 'airplay' ? (txt.deviceid ?? svc.name) : svc.name)
  const name = source === 'raop' ? svc.name.replace(/^[^@]*@/, '') : svc.name
  let playable = false
  if (source === 'airplay') {
    const features = parseAirplayFeatures(txt.features ?? txt.ft)
    const needsPassword = txt.pw === 'true' || txt.pw === '1'
    // macOS AirPlay receivers and HomePods advertise the video bit but only speak AirPlay 2
    // (no legacy HTTP /play), so only Apple TV-class receivers are marked playable.
    const model = txt.model ?? ''
    const legacyCapable = !/^(Mac|iMac|AudioAccessory|HomePod)/i.test(model)
    playable = ((features >> FEATURE_VIDEO) & 1n) === 1n && !needsPassword && legacyCapable
  }
  return {
    id: `airplay:${mac || svc.name}`,
    name,
    kind: 'airplay',
    model: txt.model ?? txt.am ?? 'AirPlay',
    manufacturer: 'Apple',
    capabilities: { audio: playable, video: playable },
    address,
    port: source === 'airplay' ? svc.port : 7000,
    wiim: false,
  }
}
