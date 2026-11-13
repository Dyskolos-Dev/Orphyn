import axios, { AxiosError, type AxiosInstance } from 'axios'
import { MIN_SERVER_VERSION, compareVersions } from '@shared/jellyfin'
import { urlCandidates } from './url'

export type ProbeErrorKind =
  /** Input is not a usable http(s) URL. */
  | 'invalid_url'
  /** No candidate answered (DNS, refused, timeout, offline). */
  | 'unreachable'
  /** Something answered but it is not a Jellyfin server. */
  | 'not_jellyfin'
  /** Certificate problem (expired, self-signed, wrong host). */
  | 'tls'
  /** Jellyfin, but older than 10.9. */
  | 'version_too_old'

export interface ProbeError {
  kind: ProbeErrorKind
  message: string
  /** Server version when known (version_too_old). */
  version?: string
}

export interface ServerInfo {
  /** Normalised base URL that answered. */
  url: string
  serverId: string
  serverName: string
  version: string
}

export type ProbeResult = { ok: true; info: ServerInfo } | { ok: false; error: ProbeError }

export interface ProbeOptions {
  http?: AxiosInstance
  timeoutMs?: number
  signal?: AbortSignal
}

interface PublicInfo {
  Id?: string
  ServerName?: string
  Version?: string
  ProductName?: string
}

const TLS_CODE =
  /^(CERT_|DEPTH_ZERO|SELF_SIGNED|UNABLE_TO_(VERIFY|GET_ISSUER)|ERR_TLS|ERR_CERT|ERR_SSL|HOSTNAME_MISMATCH)/
const TLS_MESSAGE = /certificate|ssl|tls/i

function classify(e: unknown): ProbeError {
  if (e instanceof AxiosError) {
    if (e.response) {
      return { kind: 'not_jellyfin', message: `Not a Jellyfin server (HTTP ${e.response.status}).` }
    }
    if ((e.code && TLS_CODE.test(e.code)) || TLS_MESSAGE.test(e.message)) {
      return { kind: 'tls', message: 'The server certificate could not be verified.' }
    }
  }
  return { kind: 'unreachable', message: 'The server could not be reached.' }
}

/** Higher wins when several candidates fail: a precise diagnosis beats "unreachable". */
const SEVERITY: Record<ProbeErrorKind, number> = {
  invalid_url: 0,
  unreachable: 1,
  not_jellyfin: 2,
  tls: 3,
  version_too_old: 4,
}

/**
 * Normalises `input`, then asks each candidate URL for `/System/Info/Public` (no auth needed).
 * Resolves to the first Jellyfin ≥ 10.9 found, otherwise the most informative error.
 */
export async function probeServer(input: string, opts: ProbeOptions = {}): Promise<ProbeResult> {
  const candidates = urlCandidates(input)
  if (candidates.length === 0) {
    return { ok: false, error: { kind: 'invalid_url', message: 'Enter a valid server address.' } }
  }
  const http = opts.http ?? axios.create()
  let best: ProbeError | null = null

  for (const url of candidates) {
    try {
      const res = await http.get<PublicInfo>(`${url}/System/Info/Public`, {
        timeout: opts.timeoutMs ?? 6000,
        signal: opts.signal,
        headers: { Accept: 'application/json' },
      })
      const d = res.data
      const looksLikeJellyfin =
        d !== null &&
        typeof d === 'object' &&
        typeof d.Id === 'string' &&
        typeof d.Version === 'string' &&
        (d.ProductName === undefined || /jellyfin/i.test(d.ProductName))
      if (!looksLikeJellyfin) {
        best = pick(best, { kind: 'not_jellyfin', message: 'Not a Jellyfin server.' })
        continue
      }
      const version = d.Version as string
      if (compareVersions(version, MIN_SERVER_VERSION) < 0) {
        return {
          ok: false,
          error: {
            kind: 'version_too_old',
            version,
            message: `Jellyfin ${version} is too old: version ${MIN_SERVER_VERSION} or newer is required.`,
          },
        }
      }
      return {
        ok: true,
        info: {
          url,
          serverId: d.Id as string,
          serverName: d.ServerName ?? 'Jellyfin',
          version,
        },
      }
    } catch (e) {
      if (axios.isCancel(e)) throw e
      best = pick(best, classify(e))
    }
  }
  return {
    ok: false,
    error: best ?? { kind: 'unreachable', message: 'The server could not be reached.' },
  }
}

function pick(current: ProbeError | null, next: ProbeError): ProbeError {
  return !current || SEVERITY[next.kind] > SEVERITY[current.kind] ? next : current
}
