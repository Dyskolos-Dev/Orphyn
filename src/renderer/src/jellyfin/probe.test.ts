import { AxiosError } from 'axios'
import { describe, expect, it } from 'vitest'
import { probeServer } from './probe'
import { fakeHttp, networkError } from './test-utils'

const info = (over: object = {}) => ({
  Id: 'abc',
  ServerName: 'My NAS',
  Version: '10.10.3',
  ProductName: 'Jellyfin Server',
  ...over,
})

describe('probeServer', () => {
  it('returns server name/version and the URL that answered', async () => {
    const { http, calls } = fakeHttp((r) =>
      r.url === 'http://nas:8096/System/Info/Public' ? { data: info() } : networkError(),
    )
    const res = await probeServer('nas', { http })
    expect(res).toEqual({
      ok: true,
      info: { url: 'http://nas:8096', serverId: 'abc', serverName: 'My NAS', version: '10.10.3' },
    })
    expect(calls).toHaveLength(1)
  })

  it('falls through candidates, finding a /jellyfin base path', async () => {
    const { http } = fakeHttp((r) =>
      r.url === 'https://example.com/jellyfin/System/Info/Public'
        ? { data: info() }
        : { status: 404 },
    )
    const res = await probeServer('https://example.com', { http })
    expect(res).toMatchObject({ ok: true, info: { url: 'https://example.com/jellyfin' } })
  })

  it('unreachable when nothing answers', async () => {
    const { http } = fakeHttp(() => networkError('ENOTFOUND'))
    expect(await probeServer('nope.invalid', { http })).toMatchObject({
      ok: false,
      error: { kind: 'unreachable' },
    })
  })

  it('not_jellyfin for other web servers', async () => {
    const html = fakeHttp(() => ({ data: '<html></html>' }))
    expect(await probeServer('http://x:80', { http: html.http })).toMatchObject({
      error: { kind: 'not_jellyfin' },
    })
    const other = fakeHttp(() => ({ data: info({ ProductName: 'Plex' }) }))
    expect(await probeServer('http://x:80', { http: other.http })).toMatchObject({
      error: { kind: 'not_jellyfin' },
    })
    const http404 = fakeHttp(() => ({ status: 404 }))
    expect(await probeServer('http://x:80', { http: http404.http })).toMatchObject({
      error: { kind: 'not_jellyfin' },
    })
  })

  it('tls beats unreachable', async () => {
    const { http } = fakeHttp((r) =>
      r.url.startsWith('https://')
        ? new AxiosError('x', 'DEPTH_ZERO_SELF_SIGNED_CERT')
        : networkError(),
    )
    expect(await probeServer('nas.example', { http })).toMatchObject({
      ok: false,
      error: { kind: 'tls' },
    })
  })

  it('version_too_old below 10.9, ok at 10.9.0', async () => {
    const old = fakeHttp(() => ({ data: info({ Version: '10.8.13' }) }))
    expect(await probeServer('nas', { http: old.http })).toMatchObject({
      ok: false,
      error: { kind: 'version_too_old', version: '10.8.13' },
    })
    const edge = fakeHttp(() => ({ data: info({ Version: '10.9.0' }) }))
    expect(await probeServer('nas', { http: edge.http })).toMatchObject({ ok: true })
  })

  it('invalid_url for garbage input without any request', async () => {
    const { http, calls } = fakeHttp(() => ({ data: info() }))
    expect(await probeServer('ftp://x', { http })).toMatchObject({ error: { kind: 'invalid_url' } })
    expect(calls).toHaveLength(0)
  })
})
