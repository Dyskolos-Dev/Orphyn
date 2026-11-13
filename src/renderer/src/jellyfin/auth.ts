/**
 * Login / logout. The password only ever travels in the AuthenticateByName request body: it is never
 * stored, logged, or included in thrown errors.
 */
import axios, { AxiosError, type AxiosInstance } from 'axios'
import type { ServerAccount } from '@shared/jellyfin'
import { createJellyfinApi, jfRequest } from './client'
import { probeServer, type ProbeError, type ServerInfo } from './probe'

export interface LoginSuccess {
  accessToken: string
  userId: string
  userName: string
  serverId: string
}

export type LoginErrorKind = 'invalid_credentials' | 'unreachable' | 'server_error'
export interface LoginError {
  kind: LoginErrorKind
  message: string
}
export type LoginResult = { ok: true; session: LoginSuccess } | { ok: false; error: LoginError }

interface AuthenticationResult {
  AccessToken?: string
  ServerId?: string
  User?: { Id?: string; Name?: string }
}

/** POST /Users/AuthenticateByName as client "Jellyra" with this install's deviceId. */
export async function login(
  url: string,
  username: string,
  password: string,
  deviceId: string,
  http?: AxiosInstance,
): Promise<LoginResult> {
  const api = createJellyfinApi({ baseUrl: url, deviceId, http })
  try {
    const res = await jfRequest<AuthenticationResult>(api, 'POST', '/Users/AuthenticateByName', {
      data: { Username: username, Pw: password },
    })
    if (!res?.AccessToken || !res.User?.Id) {
      return { ok: false, error: { kind: 'server_error', message: 'Unexpected server response.' } }
    }
    return {
      ok: true,
      session: {
        accessToken: res.AccessToken,
        userId: res.User.Id,
        userName: res.User.Name ?? username,
        serverId: res.ServerId ?? '',
      },
    }
  } catch (e) {
    const status = (e as { status?: number }).status
    if (status === 401 || status === 400) {
      return {
        ok: false,
        error: { kind: 'invalid_credentials', message: 'Wrong username or password.' },
      }
    }
    if (status === undefined) {
      return {
        ok: false,
        error: { kind: 'unreachable', message: 'The server could not be reached.' },
      }
    }
    return { ok: false, error: { kind: 'server_error', message: `Server error (${status}).` } }
  }
}

export type ConnectResult =
  { ok: true; account: ServerAccount } | { ok: false; error: ProbeError | LoginError }

export interface ConnectParams {
  /** What the user typed; normalised and probed. */
  url: string
  username: string
  password: string
  /** User-chosen display name; defaults to the existing entry's name, else the server's. */
  name?: string
  http?: AxiosInstance
}

/** Probe → login → persist through `accounts.add` (main encrypts the token). Does not activate. */
export async function connectServer(p: ConnectParams): Promise<ConnectResult> {
  const probe = await probeServer(p.url, { http: p.http })
  if (!probe.ok) return probe
  const info: ServerInfo = probe.info
  const deviceId = await window.jellyra.accounts.getDeviceId()
  const res = await login(info.url, p.username, p.password, deviceId, p.http)
  if (!res.ok) return res
  const account = await window.jellyra.accounts.add({
    // Empty: the store keeps an existing account's name, or uses the server's name.
    name: p.name?.trim() ?? '',
    url: info.url,
    serverId: info.serverId,
    serverName: info.serverName,
    version: info.version,
    userId: res.session.userId,
    userName: res.session.userName,
    accessToken: res.session.accessToken,
  })
  return { ok: true, account }
}

/** Revokes the session on the server (best effort: offline is fine) then forgets the account. */
export async function logoutAccount(account: ServerAccount, http?: AxiosInstance): Promise<void> {
  const token = await window.jellyra.accounts.getToken(account.id)
  if (token) {
    const deviceId = await window.jellyra.accounts.getDeviceId()
    const api = createJellyfinApi({ baseUrl: account.url, deviceId, accessToken: token, http })
    try {
      await jfRequest<void>(api, 'POST', '/Sessions/Logout')
    } catch (e) {
      if (axios.isCancel(e) || e instanceof AxiosError) {
        // ignore: the token will simply expire server side
      }
    }
  }
  await window.jellyra.accounts.remove(account.id)
}
