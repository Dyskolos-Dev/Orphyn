import { describe, expect, it, vi, beforeEach } from 'vitest'
import { connectServer, login, logoutAccount } from './auth'
import { jfGet } from './client'
import { queryClient, useSession } from './session'
import { fakeHttp, installFakeAccounts, networkError } from './test-utils'

const authOk = {
  AccessToken: 'TOKEN-1',
  ServerId: 'srv',
  User: { Id: 'u1', Name: 'bob' },
}

describe('login', () => {
  it('sends the Orphyn client identity and never leaks the password', async () => {
    const { http, calls } = fakeHttp(() => ({ data: authOk }))
    const spy = vi.spyOn(console, 'log')
    const res = await login('http://s:8096', 'bob', 'hunter2', 'dev-9', http)
    expect(res).toMatchObject({ ok: true, session: { accessToken: 'TOKEN-1', userId: 'u1' } })
    const call = calls[0]!
    expect(call.url).toBe('http://s:8096/Users/AuthenticateByName')
    expect(call.data).toEqual({ Username: 'bob', Pw: 'hunter2' })
    const auth = String(call.headers['Authorization'])
    expect(auth).toContain('Client="Orphyn"')
    expect(auth).toContain('DeviceId="dev-9"')
    expect(auth).not.toContain('hunter2')
    expect(JSON.stringify(spy.mock.calls)).not.toContain('hunter2')
  })

  it('maps 401 to invalid_credentials and offline to unreachable, without the password', async () => {
    const bad = await login(
      'http://s',
      'bob',
      'hunter2',
      'd',
      fakeHttp(() => ({ status: 401 })).http,
    )
    expect(bad).toMatchObject({ ok: false, error: { kind: 'invalid_credentials' } })
    expect(JSON.stringify(bad)).not.toContain('hunter2')
    const off = await login('http://s', 'bob', 'x', 'd', fakeHttp(() => networkError()).http)
    expect(off).toMatchObject({ error: { kind: 'unreachable' } })
  })
})

describe('connectServer / logout', () => {
  beforeEach(() => installFakeAccounts())

  const serverHttp = () =>
    fakeHttp((r) => {
      if (r.url.endsWith('/System/Info/Public')) {
        return { data: { Id: 'srv', ServerName: 'NAS', Version: '10.10.3' } }
      }
      if (r.url.endsWith('/Users/AuthenticateByName')) return { data: authOk }
      return { status: 204 }
    })

  it('probes, logs in and stores the account with the chosen name (token only, no password)', async () => {
    const { http } = serverHttp()
    const res = await connectServer({
      url: 'http://s:8096',
      username: 'bob',
      password: 'hunter2',
      name: 'Salon',
      http,
    })
    expect(res.ok).toBe(true)
    const accounts = await window.orphyn.accounts.list()
    expect(accounts).toHaveLength(1)
    expect(accounts[0]).toMatchObject({ name: 'Salon', serverId: 'srv', userId: 'u1' })
    expect(JSON.stringify(accounts)).not.toContain('hunter2')
  })

  it('logout revokes the session on the server then removes the account', async () => {
    const { http, calls } = serverHttp()
    const res = await connectServer({ url: 'http://s:8096', username: 'bob', password: 'p', http })
    if (!res.ok) throw new Error('connect failed')
    await logoutAccount(res.account, http)
    const logout = calls.find((c) => c.url.endsWith('/Sessions/Logout'))
    expect(logout?.method).toBe('POST')
    expect(String(logout?.headers['Authorization'])).toContain('Token="TOKEN-1"')
    expect(await window.orphyn.accounts.list()).toEqual([])
  })

  it('logout still removes the account when the server is offline', async () => {
    const { http } = serverHttp()
    const res = await connectServer({ url: 'http://s:8096', username: 'bob', password: 'p', http })
    if (!res.ok) throw new Error('connect failed')
    await logoutAccount(res.account, fakeHttp(() => networkError()).http)
    expect(await window.orphyn.accounts.list()).toEqual([])
  })
})

describe('session store', () => {
  beforeEach(() => {
    useSession.setState({
      status: 'idle',
      accounts: [],
      account: null,
      api: null,
      deviceId: null,
      needsLogin: {},
    })
    queryClient.clear()
  })

  async function twoServers() {
    const { api } = installFakeAccounts()
    const base = { serverName: 'S', version: '10.10.0', userName: 'bob' }
    const a = await api.add({
      ...base,
      name: 'A',
      url: 'http://a',
      serverId: 'sa',
      userId: 'ua',
      accessToken: 'ta',
    })
    const b = await api.add({
      ...base,
      name: 'B',
      url: 'http://b',
      serverId: 'sb',
      userId: 'ub',
      accessToken: 'tb',
    })
    return { a, b }
  }

  it('init activates the stored active server with its token', async () => {
    const { a } = await twoServers()
    await useSession.getState().init()
    const s = useSession.getState()
    expect(s.status).toBe('ready')
    expect(s.account?.id).toBe(a.id)
    expect(s.api?.basePath).toBe('http://a')
    expect(s.api?.accessToken).toBe('ta')
    expect(s.accounts).toHaveLength(2)
  })

  it('no server → status no-server', async () => {
    installFakeAccounts()
    await useSession.getState().init()
    expect(useSession.getState()).toMatchObject({ status: 'no-server', api: null })
  })

  it('switching servers swaps the Api and drops cached data of the old server', async () => {
    const { b } = await twoServers()
    await useSession.getState().init()
    queryClient.setQueryData(['jf', 'old', 'views'], ['stale'])
    expect(queryClient.getQueryCache().getAll()).toHaveLength(1)
    await useSession.getState().switchServer(b.id)
    const s = useSession.getState()
    expect(s.api?.basePath).toBe('http://b')
    expect(s.api?.accessToken).toBe('tb')
    expect(queryClient.getQueryCache().getAll()).toHaveLength(0)
    expect((await window.orphyn.accounts.getActive())?.id).toBe(b.id)
  })

  it('a missing token requires login', async () => {
    const { tokens } = installFakeAccounts()
    await window.orphyn.accounts.add({
      name: 'A',
      url: 'http://a',
      serverId: 'sa',
      serverName: 'S',
      version: '10.10.0',
      userId: 'ua',
      userName: 'bob',
      accessToken: 't',
    })
    tokens.clear()
    await useSession.getState().init()
    const s = useSession.getState()
    expect(s.status).toBe('needs-login')
    expect(s.api).toBeNull()
    expect(Object.keys(s.needsLogin)).toHaveLength(1)
  })

  it('a 401 from the active server marks it as needing re-login', async () => {
    const { a } = await twoServers()
    await useSession.getState().init()
    const api = useSession.getState().api!
    const { http } = fakeHttp(() => ({ status: 401 }))
    Object.defineProperty(api, 'axiosInstance', { value: http })
    await expect(jfGet(api, '/UserViews')).rejects.toMatchObject({ status: 401 })
    expect(useSession.getState().needsLogin[a.id]).toBe(true)
    expect(useSession.getState().status).toBe('needs-login')
  })

  it('signOut revokes, removes and falls back to no active server', async () => {
    const { a } = await twoServers()
    await useSession.getState().init()
    await useSession.getState().signOut(a.id)
    expect(useSession.getState().accounts).toHaveLength(1)
    expect(useSession.getState().account).toBeNull()
  })
})
