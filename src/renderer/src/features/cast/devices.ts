import type { CastDevice } from '@shared/cast'

/** What the picker is opened for: audio for the music player, video for the video player. */
export type CastScope = 'music' | 'video'

/** Devices able to play this kind of media (AirPlay 2 / RAOP-only devices have neither). */
export function devicesForScope(devices: readonly CastDevice[], scope: CastScope): CastDevice[] {
  return devices.filter((d) => (scope === 'video' ? d.capabilities.video : d.capabilities.audio))
}

export type DeviceGroup = 'speaker' | 'tv'

/** Video-capable receivers are TVs, everything else (WiiM included) a speaker. */
export function groupOf(device: CastDevice): DeviceGroup {
  return !device.wiim && device.capabilities.video ? 'tv' : 'speaker'
}

export function groupDevices(
  devices: readonly CastDevice[],
): { group: DeviceGroup; devices: CastDevice[] }[] {
  const byName = (a: CastDevice, b: CastDevice) => a.name.localeCompare(b.name)
  return (['speaker', 'tv'] as const)
    .map((group) => ({ group, devices: devices.filter((d) => groupOf(d) === group).sort(byName) }))
    .filter((g) => g.devices.length > 0)
}
