/** Test helpers: an axios instance whose network is a hand-written function (no extra deps). */
import axios, {
  AxiosError,
  type AxiosInstance,
  type AxiosResponse,
  type InternalAxiosRequestConfig,
} from 'axios'

export interface FakeReply {
  status?: number
  data?: unknown
}
export type FakeHandler = (req: {
  method: string
  url: string
  data: unknown
  headers: Record<string, unknown>
}) => FakeReply | AxiosError | Error

export interface FakeHttp {
  http: AxiosInstance
  calls: { method: string; url: string; data: unknown; headers: Record<string, unknown> }[]
}

export function fakeHttp(handler: FakeHandler): FakeHttp {
  const calls: FakeHttp['calls'] = []
  const http = axios.create({
    adapter: (config: InternalAxiosRequestConfig) => {
      let data: unknown = config.data
      if (typeof data === 'string') {
        try {
          data = JSON.parse(data)
        } catch {
          /* keep raw */
        }
      }
      const req = {
        method: (config.method ?? 'get').toUpperCase(),
        url: config.url ?? '',
        data,
        headers: { ...(config.headers as unknown as Record<string, unknown>) },
      }
      calls.push(req)
      const out = handler(req)
      if (out instanceof Error) return Promise.reject(out)
      const status = out.status ?? 200
      const response = {
        data: out.data,
        status,
        statusText: String(status),
        headers: {},
        config,
      } as AxiosResponse
      if (status >= 400) {
        return Promise.reject(
          new AxiosError(`HTTP ${status}`, 'ERR_BAD_REQUEST', config, null, response),
        )
      }
      return Promise.resolve(response)
    },
  })
  return { http, calls }
}

export const networkError = (code = 'ECONNREFUSED') =>
  new AxiosError('connect failed', code, undefined, null, undefined)

/** Minimal in-memory implementation of window.jellyra.accounts. */
export function installFakeAccounts() {
  const accounts: import('@shared/jellyfin').ServerAccount[] = []
  const tokens = new Map<string, string>()
  let activeId: string | null = null
  let n = 0
  const api = {
    list: async () => [...accounts],
    add: async (i: import('@shared/jellyfin').NewServerAccount) => {
      const { accessToken, ...rest } = i
      const acc = {
        ...rest,
        id: `acc-${++n}`,
        addedAt: 'x',
        lastUsedAt: 'x',
        hasToken: true,
      }
      accounts.push(acc)
      tokens.set(acc.id, accessToken)
      activeId ??= acc.id
      return acc
    },
    rename: async () => null,
    remove: async (id: string) => {
      accounts.splice(
        accounts.findIndex((a) => a.id === id),
        1,
      )
      tokens.delete(id)
      if (activeId === id) activeId = null
    },
    setActive: async (id: string | null) => {
      activeId = id
      return accounts.find((a) => a.id === id) ?? null
    },
    getActive: async () => accounts.find((a) => a.id === activeId) ?? null,
    getToken: async (id: string) => tokens.get(id) ?? null,
    getDeviceId: async () => 'device-1',
    getStatus: async () => ({ encrypted: true }),
  }
  ;(window as unknown as { jellyra: unknown }).jellyra = { accounts: api }
  return { api, tokens }
}
