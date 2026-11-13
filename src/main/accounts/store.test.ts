// @vitest-environment node
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { AccountStore, type SecureStorage } from './store'

const reverse = (s: string) => [...s].reverse().join('')

const fakeStorage = (
  available = true,
  backend?: string,
  shouldReEncrypt = false,
): SecureStorage => ({
  isAsyncEncryptionAvailable: async () => available,
  encryptStringAsync: async (s) => Buffer.from(`enc:${reverse(s)}`),
  decryptStringAsync: async (b) => ({ result: reverse(b.toString().slice(4)), shouldReEncrypt }),
  getSelectedStorageBackend: backend ? () => backend : undefined,
})

const input = (over = {}) => ({
  name: 'Home',
  url: 'http://nas:8096',
  serverId: 's1',
  serverName: 'NAS',
  version: '10.10.3',
  userId: 'u1',
  userName: 'bob',
  accessToken: 'SECRET-TOKEN',
  ...over,
})

let dir: string
let file: string
const open = (storage: SecureStorage = fakeStorage(), extra = {}) =>
  AccountStore.open({ filePath: file, storage, ...extra })

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), 'orphyn-acc-'))
  file = join(dir, 'accounts.json')
})
afterEach(() => rmSync(dir, { recursive: true, force: true }))

describe('AccountStore', () => {
  it('encrypts the token on disk and restores it', async () => {
    const s = await open()
    const acc = await s.add(input())
    const raw = readFileSync(file, 'utf8')
    expect(raw).not.toContain('SECRET-TOKEN')
    expect(raw).not.toMatch(/password/i)
    expect(JSON.parse(raw).accounts[0].accessToken).toBeTruthy()
    const s2 = await open()
    expect(s2.getToken(acc.id)).toBe('SECRET-TOKEN')
    expect(s2.list()[0]).toMatchObject({ name: 'Home', hasToken: true })
    expect(s2.list()[0]).not.toHaveProperty('accessToken')
  })

  it('keeps the ciphertext across unrelated saves', async () => {
    const s = await open()
    const acc = await s.add(input())
    const s2 = await open()
    s2.rename(acc.id, 'Salon')
    expect((await open()).getToken(acc.id)).toBe('SECRET-TOKEN')
  })

  it('re-encrypts tokens the keychain asks to rotate', async () => {
    const s = await open()
    const acc = await s.add(input())
    const before = readFileSync(file, 'utf8')
    const s2 = await open(fakeStorage(true, undefined, true))
    expect(s2.getToken(acc.id)).toBe('SECRET-TOKEN')
    expect(readFileSync(file, 'utf8')).not.toContain('SECRET-TOKEN')
    expect(JSON.parse(readFileSync(file, 'utf8')).accounts[0].accessToken).toBe(
      JSON.parse(before).accounts[0].accessToken,
    )
  })

  it('keeps the token in memory only when encryption is unavailable', async () => {
    for (const storage of [fakeStorage(false), fakeStorage(true, 'basic_text')]) {
      const s = await open(storage)
      const acc = await s.add(input({ serverId: `s-${Math.random()}` }))
      expect(s.getStatus().encrypted).toBe(false)
      expect(s.getStatus().warning).toBeTruthy()
      expect(s.getToken(acc.id)).toBe('SECRET-TOKEN')
      expect(readFileSync(file, 'utf8')).not.toContain('SECRET-TOKEN')
      const s2 = await open(storage)
      expect(s2.getToken(acc.id)).toBeNull()
      expect(s2.list().find((a) => a.id === acc.id)?.hasToken).toBe(false)
    }
  })

  it('keeps several servers, the first becomes active, setActive switches', async () => {
    const s = await open()
    const a = await s.add(input())
    const b = await s.add(input({ serverId: 's2', name: 'Work', accessToken: 'T2' }))
    expect(s.list()).toHaveLength(2)
    expect(s.getActive()?.id).toBe(a.id)
    expect(s.setActive(b.id)?.id).toBe(b.id)
    expect((await open()).getActive()?.id).toBe(b.id)
    expect(s.setActive('nope')).toBeNull()
  })

  it('updates the same server+user in place', async () => {
    const s = await open()
    const a = await s.add(input())
    const again = await s.add(input({ accessToken: 'NEW', name: 'Second' }))
    expect(again.id).toBe(a.id)
    expect(again.name).toBe('Second')
    expect(s.list()).toHaveLength(1)
    expect(s.getToken(a.id)).toBe('NEW')
    expect((await open()).getToken(a.id)).toBe('NEW')
  })

  it('keeps the chosen name when signing in again without one', async () => {
    const s = await open()
    const a = await s.add(input({ name: 'Salon' }))
    const again = await s.add(input({ name: '', accessToken: 'NEW' }))
    expect(again).toMatchObject({ id: a.id, name: 'Salon' })
    const fresh = await s.add(input({ serverId: 's9', name: '  ' }))
    expect(fresh.name).toBe('NAS')
  })

  it('renames and removes (clearing active)', async () => {
    const s = await open()
    const a = await s.add(input())
    expect(s.rename(a.id, ' Salon ')?.name).toBe('Salon')
    expect(s.rename('x', 'y')).toBeNull()
    s.remove(a.id)
    expect(s.list()).toEqual([])
    expect(s.getActive()).toBeNull()
    expect(s.getToken(a.id)).toBeNull()
  })

  it('removing the active server activates the most recently used remaining one', async () => {
    let clock = 0
    const s = await open(fakeStorage(), { now: () => new Date(1_700_000_000_000 + clock++ * 1000) })
    const a = await s.add(input())
    const b = await s.add(input({ serverId: 's2', accessToken: 'T2' }))
    const c = await s.add(input({ serverId: 's3', accessToken: 'T3' }))
    s.setActive(b.id) // b becomes the most recent
    s.setActive(a.id)
    s.remove(a.id)
    expect(s.getActive()?.id).toBe(b.id)
    s.remove(b.id)
    expect(s.getActive()?.id).toBe(c.id)
  })

  it('has a stable device id and survives a corrupt file', async () => {
    const s = await open()
    const id = s.getDeviceId()
    expect((await open()).getDeviceId()).toBe(id)
    writeFileSync(file, '{not json')
    const s3 = await open()
    expect(s3.list()).toEqual([])
    expect(s3.getDeviceId()).toBeTruthy()
  })
})
