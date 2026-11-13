import http from 'node:http'
import https from 'node:https'
import { escapeXml, findNode, parseXml } from './xml'

export const AVTRANSPORT_SERVICE = 'urn:schemas-upnp-org:service:AVTransport:1'
export const RENDERING_CONTROL_SERVICE = 'urn:schemas-upnp-org:service:RenderingControl:1'

/** Builds a SOAP envelope; argument values are XML-escaped (pass DIDL as a plain string). */
export function buildSoapEnvelope(
  serviceType: string,
  action: string,
  args: Array<[string, string | number]>,
): string {
  const body = args.map(([k, v]) => `<${k}>${escapeXml(String(v))}</${k}>`).join('')
  return (
    '<?xml version="1.0" encoding="utf-8"?>' +
    '<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" ' +
    's:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">' +
    `<s:Body><u:${action} xmlns:u="${serviceType}">${body}</u:${action}></s:Body></s:Envelope>`
  )
}

export function soapActionHeader(serviceType: string, action: string): string {
  return `"${serviceType}#${action}"`
}

export class SoapFaultError extends Error {
  constructor(
    message: string,
    readonly code?: string,
  ) {
    super(message)
    this.name = 'SoapFaultError'
  }
}

/** Parses a SOAP response body into its output arguments; throws SoapFaultError on a fault. */
export function parseSoapResponse(xml: string): Record<string, string> {
  const root = parseXml(xml)
  const fault = findNode(root, 'Fault')
  if (fault) {
    const code = findNode(fault, 'errorCode')?.text.trim()
    const desc = findNode(fault, 'errorDescription')?.text.trim()
    throw new SoapFaultError(`UPnP fault${code ? ` ${code}` : ''}${desc ? `: ${desc}` : ''}`, code)
  }
  const body = findNode(root, 'Body')
  const response = body?.children[0]
  const out: Record<string, string> = {}
  for (const child of response?.children ?? []) out[child.name] = child.text.trim()
  return out
}

export interface SoapCallOptions {
  controlUrl: string
  serviceType: string
  action: string
  args?: Array<[string, string | number]>
  timeoutMs?: number
}

/** Sends one SOAP action over HTTP with a hard timeout. */
export function soapCall(opts: SoapCallOptions): Promise<Record<string, string>> {
  const payload = buildSoapEnvelope(opts.serviceType, opts.action, opts.args ?? [])
  const timeout = opts.timeoutMs ?? 5000
  return new Promise((resolve, reject) => {
    let url: URL
    try {
      url = new URL(opts.controlUrl)
    } catch (e) {
      reject(e instanceof Error ? e : new Error(String(e)))
      return
    }
    const lib = url.protocol === 'https:' ? https : http
    let settled = false
    const done = (fn: () => void): void => {
      if (!settled) {
        settled = true
        fn()
      }
    }
    const req = lib.request(
      url,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'text/xml; charset="utf-8"',
          'Content-Length': Buffer.byteLength(payload),
          SOAPACTION: soapActionHeader(opts.serviceType, opts.action),
        },
        timeout,
      },
      (res) => {
        const chunks: Buffer[] = []
        res.on('data', (c: Buffer) => chunks.push(c))
        res.on('error', (e) => done(() => reject(e)))
        res.on('end', () => {
          const text = Buffer.concat(chunks).toString('utf8')
          done(() => {
            try {
              // Faults arrive with HTTP 500 but a parseable body.
              const out = parseSoapResponse(text)
              if ((res.statusCode ?? 200) >= 400) {
                reject(new Error(`HTTP ${res.statusCode} from ${opts.action}`))
              } else resolve(out)
            } catch (e) {
              reject(e instanceof Error ? e : new Error(String(e)))
            }
          })
        })
      },
    )
    req.on('timeout', () =>
      req.destroy(new Error(`SOAP ${opts.action} timed out after ${timeout} ms`)),
    )
    req.on('error', (e) => done(() => reject(e)))
    req.end(payload)
  })
}
