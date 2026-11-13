// @vitest-environment node
import { mkdirSync, mkdtempSync, readFileSync, existsSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { migrateLegacyUserData } from './migrate'

let root: string
let oldDir: string
let newDir: string

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), 'orphyn-migrate-'))
  oldDir = join(root, 'Lumen')
  newDir = join(root, 'Orphyn')
  mkdirSync(join(oldDir, 'Local Storage', 'leveldb'), { recursive: true })
  mkdirSync(join(oldDir, 'IndexedDB'), { recursive: true })
  mkdirSync(join(oldDir, 'Cache'), { recursive: true })
  writeFileSync(join(oldDir, 'accounts.json'), '{"accounts":["a"]}')
  writeFileSync(join(oldDir, 'window-state.json'), '{"width":900}')
  writeFileSync(join(oldDir, 'Local Storage', 'leveldb', '000003.log'), 'prefs')
  writeFileSync(join(oldDir, 'Cache', 'junk'), 'x')
  mkdirSync(newDir)
})

afterEach(() => rmSync(root, { recursive: true, force: true }))

describe('migrateLegacyUserData', () => {
  it('copies accounts, state files and profile storage, and logs it', () => {
    const logs: string[] = []
    expect(migrateLegacyUserData(newDir, (m) => logs.push(m))).toBe(oldDir)
    expect(readFileSync(join(newDir, 'accounts.json'), 'utf8')).toBe('{"accounts":["a"]}')
    expect(readFileSync(join(newDir, 'window-state.json'), 'utf8')).toBe('{"width":900}')
    expect(readFileSync(join(newDir, 'Local Storage', 'leveldb', '000003.log'), 'utf8')).toBe(
      'prefs',
    )
    expect(existsSync(join(newDir, 'IndexedDB'))).toBe(true)
    expect(existsSync(join(newDir, 'Cache'))).toBe(false)
    expect(existsSync(join(oldDir, 'accounts.json'))).toBe(true)
    expect(logs).toHaveLength(1)
  })

  it('does nothing when the new folder already has accounts', () => {
    writeFileSync(join(newDir, 'accounts.json'), '{"accounts":["new"]}')
    expect(migrateLegacyUserData(newDir)).toBeNull()
    expect(readFileSync(join(newDir, 'accounts.json'), 'utf8')).toBe('{"accounts":["new"]}')
    expect(existsSync(join(newDir, 'window-state.json'))).toBe(false)
  })

  it('does nothing without a legacy folder holding accounts', () => {
    rmSync(join(oldDir, 'accounts.json'))
    expect(migrateLegacyUserData(newDir)).toBeNull()
    expect(existsSync(join(newDir, 'window-state.json'))).toBe(false)
  })

  it('works when the new folder does not exist yet and never overwrites', () => {
    rmSync(newDir, { recursive: true })
    expect(migrateLegacyUserData(newDir)).toBe(oldDir)
    expect(existsSync(join(newDir, 'accounts.json'))).toBe(true)
    expect(migrateLegacyUserData(newDir)).toBeNull()
  })
})
