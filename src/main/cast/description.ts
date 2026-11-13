import { findAll, findNode, parseXml, textOf, type XmlNode } from './xml'

export interface UpnpDescription {
  udn: string
  friendlyName: string
  manufacturer: string
  modelName: string
  deviceType: string
  /** Absolute AVTransport control URL. */
  avTransportControlUrl?: string
  /** Absolute RenderingControl control URL. */
  renderingControlUrl?: string
  isWiim: boolean
}

export function isWiimManufacturer(manufacturer: string, model = ''): boolean {
  return /linkplay|wiim/i.test(manufacturer) || /wiim/i.test(model)
}

function resolveUrl(path: string | undefined, base: string): string | undefined {
  if (!path) return undefined
  try {
    return new URL(path, base).toString()
  } catch {
    return undefined
  }
}

/** Parses a UPnP device description; `location` is the URL it was fetched from (base for relative URLs). */
export function parseDeviceDescription(xml: string, location: string): UpnpDescription | null {
  const root = parseXml(xml)
  const device = findNode(root, 'device')
  if (!device) return null
  const base = textOf(root, 'URLBase') || location
  const manufacturer = textOf(device, 'manufacturer') ?? ''
  const modelName = textOf(device, 'modelName') ?? ''
  const result: UpnpDescription = {
    udn: textOf(device, 'UDN') ?? '',
    friendlyName: textOf(device, 'friendlyName') ?? '',
    manufacturer,
    modelName,
    deviceType: textOf(device, 'deviceType') ?? '',
    isWiim: isWiimManufacturer(manufacturer, modelName),
  }
  const services: XmlNode[] = findAll(device, 'service')
  for (const svc of services) {
    const type = textOf(svc, 'serviceType') ?? ''
    const control = resolveUrl(textOf(svc, 'controlURL'), base)
    if (type.includes(':service:AVTransport:')) result.avTransportControlUrl = control
    else if (type.includes(':service:RenderingControl:')) result.renderingControlUrl = control
  }
  return result
}
