import type { CastDevice } from '@shared/cast'

/** Live set of discovered devices with last-seen expiry. */
export class DeviceRegistry {
  private readonly devices = new Map<string, { device: CastDevice; seenAt: number }>()

  constructor(
    private readonly onChange: (devices: CastDevice[]) => void,
    private readonly ttlMs = 90_000,
  ) {}

  list(): CastDevice[] {
    return [...this.devices.values()]
      .map((e) => e.device)
      .sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id))
  }

  get(id: string): CastDevice | undefined {
    return this.devices.get(id)?.device
  }

  /** Adds or refreshes a device; notifies only when something visible changed. */
  upsert(device: CastDevice, now = Date.now()): void {
    const prev = this.devices.get(device.id)
    this.devices.set(device.id, { device, seenAt: now })
    if (!prev || JSON.stringify(prev.device) !== JSON.stringify(device)) this.onChange(this.list())
  }

  /** Marks a known device as still alive without changing it. */
  touch(id: string, now = Date.now()): boolean {
    const e = this.devices.get(id)
    if (e) e.seenAt = now
    return !!e
  }

  remove(id: string): void {
    if (this.devices.delete(id)) this.onChange(this.list())
  }

  /** Drops devices not seen within the TTL. */
  sweep(now = Date.now()): void {
    let changed = false
    for (const [id, e] of this.devices) {
      if (now - e.seenAt > this.ttlMs) {
        this.devices.delete(id)
        changed = true
      }
    }
    if (changed) this.onChange(this.list())
  }

  clear(): void {
    if (this.devices.size === 0) return
    this.devices.clear()
    this.onChange([])
  }
}
