/**
 * Low-level Jellyfin HTTP access. Everything else in the data layer goes through `jfRequest`, so
 * auth headers, query serialisation and 401 handling live in exactly one place.
 */
import axios, { AxiosError, type AxiosInstance, type Method } from 'axios'
import { Api } from '@jellyfin/sdk'
import { CLIENT_NAME, DEVICE_NAME } from '@shared/jellyfin'

export const APP_VERSION = '0.1.0'

export type QueryValue = string | number | boolean | null | undefined | readonly (string | number)[]
export type QueryParams = Record<string, QueryValue>

/** Serialises params the way Jellyfin expects: arrays as comma lists, undefined/null dropped. */
export function buildQuery(params: QueryParams | undefined): string {
  if (!params) return ''
  const parts: string[] = []
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null) continue
    const v = Array.isArray(value) ? value.join(',') : String(value)
    if (Array.isArray(value) && value.length === 0) continue
    parts.push(`${encodeURIComponent(key)}=${encodeURIComponent(v)}`)
  }
  return parts.join('&')
}

export interface CreateApiOptions {
  baseUrl: string
  deviceId: string
  accessToken?: string
  /** Inject an axios instance (tests use a custom adapter). */
  http?: AxiosInstance
}

/** Creates an SDK `Api` carrying the Orphyn client name/version and the install's deviceId. */
export function createJellyfinApi(opts: CreateApiOptions): Api {
  return new Api(
    opts.baseUrl,
    { name: CLIENT_NAME, version: APP_VERSION },
    { id: opts.deviceId, name: DEVICE_NAME },
    opts.accessToken,
    opts.http ?? axios.create(),
  )
}

type UnauthorizedHandler = (api: Api) => void
let unauthorizedHandler: UnauthorizedHandler | null = null

/** The session store registers itself here so a 401 flags the server as needing a new login. */
export function setUnauthorizedHandler(handler: UnauthorizedHandler | null): void {
  unauthorizedHandler = handler
}

export interface RequestOptions {
  params?: QueryParams
  data?: unknown
  signal?: AbortSignal
  /** Statuses to resolve with `undefined` instead of throwing (e.g. 404 for "no lyrics"). */
  allowStatus?: number[]
}

export class JellyfinHttpError extends Error {
  constructor(
    message: string,
    readonly status: number | undefined,
  ) {
    super(message)
    this.name = 'JellyfinHttpError'
  }
}

/** Performs an authenticated request against the server of `api` and returns the parsed body. */
export async function jfRequest<T>(
  api: Api,
  method: Method,
  path: string,
  opts: RequestOptions = {},
): Promise<T> {
  const query = buildQuery(opts.params)
  const url = `${api.basePath}${path}${query ? `?${query}` : ''}`
  try {
    const res = await api.axiosInstance.request<T>({
      method,
      url,
      data: opts.data,
      signal: opts.signal,
      headers: { Authorization: api.authorizationHeader },
    })
    return res.data
  } catch (e) {
    if (axios.isCancel(e)) throw e
    const status = e instanceof AxiosError ? e.response?.status : undefined
    if (status !== undefined && opts.allowStatus?.includes(status)) return undefined as T
    if (status === 401 && api.accessToken) unauthorizedHandler?.(api)
    throw new JellyfinHttpError(
      status
        ? `Jellyfin request failed (${status}): ${method} ${path}`
        : `Network error: ${method} ${path}`,
      status,
    )
  }
}

export const jfGet = <T>(api: Api, path: string, params?: QueryParams, signal?: AbortSignal) =>
  jfRequest<T>(api, 'GET', path, { params, signal })

/** Absolute URL with `ApiKey` for media elements / external renderers (they cannot send headers). */
export function authedUrl(api: Api, path: string, params: QueryParams = {}): string {
  const query = buildQuery({ ...params, ApiKey: api.accessToken })
  return `${api.basePath}${path}?${query}`
}
