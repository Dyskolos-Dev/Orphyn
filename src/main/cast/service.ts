import type { CastDevice } from '@shared/cast'
import { Discovery } from './discovery'
import { CastManager, type ManagerEvents } from './manager'
import { DeviceRegistry } from './registry'
import { AirplayTransport } from './transports/airplay'
import { ChromecastTransport } from './transports/chromecast'
import { UpnpTransport } from './transports/upnp'
import type { CastTransport } from './transport'

/** Wires registry + discovery + real protocol transports into a CastManager (no Electron here). */
export function createCastService(events: ManagerEvents): CastManager {
  const registry = new DeviceRegistry((devices) => events.onDevicesChanged(devices))
  const discovery = new Discovery(registry)
  const createTransport = (device: CastDevice): CastTransport | null => {
    switch (device.kind) {
      case 'chromecast':
        return new ChromecastTransport({ host: device.address, port: device.port })
      case 'upnp': {
        const endpoints = discovery.getUpnpEndpoints(device.id)
        return endpoints ? new UpnpTransport(endpoints) : null
      }
      case 'airplay':
        return new AirplayTransport({ host: device.address, port: device.port })
    }
  }
  return new CastManager({
    discovery,
    listDevices: () => registry.list(),
    getDevice: (id) => registry.get(id),
    createTransport,
    events,
  })
}
