import { describe, expect, it } from 'vitest'
import { version } from '../../../../../package.json'
import { bundledNotes, notesToShow, type VersionNotes } from './notes'
import { compareVersions, parseVersion } from './version'

const note = { title: 't', items: [{ icon: 'music', title: 'a', text: 'b' }] }
const mk = (v: string): VersionNotes => ({ version: v, note: { fr: note, en: note } })
const notes = [mk('1.0.2'), mk('1.0.3'), mk('1.1.0')]

describe('versions', () => {
  it('parses and compares numerically', () => {
    expect(parseVersion('v1.2.3-beta.1')).toEqual([1, 2, 3])
    expect(parseVersion('nope')).toBeNull()
    expect(compareVersions('1.0.10', '1.0.9')).toBeGreaterThan(0)
    expect(compareVersions('1.0.2', '1.0.3')).toBeLessThan(0)
    expect(compareVersions('2.0.0', '1.99.99')).toBeGreaterThan(0)
    expect(compareVersions('1.0.3', '1.0.3')).toBe(0)
  })
})

describe('notesToShow', () => {
  const run = (lastSeen: string | null, current: string, hasAccount: boolean) =>
    notesToShow({ lastSeen, current, hasAccount, notes }).map((n) => n.version)

  it('shows nothing on a fresh install (no value, no account)', () => {
    expect(run(null, '1.0.3', false)).toEqual([])
  })
  it('treats an unstored version with an account as an upgrade from 1.0.2', () => {
    expect(run(null, '1.0.3', true)).toEqual(['1.0.3'])
  })
  it('shows the new version once, then nothing when already seen', () => {
    expect(run('1.0.2', '1.0.3', true)).toEqual(['1.0.3'])
    expect(run('1.0.3', '1.0.3', true)).toEqual([])
  })
  it('shows every skipped version, newest first, but never a future one', () => {
    expect(run('1.0.1', '1.1.0', true)).toEqual(['1.1.0', '1.0.3', '1.0.2'])
    expect(run('1.0.2', '1.0.3', true)).not.toContain('1.1.0')
  })
  it('shows nothing after a downgrade', () => {
    expect(run('1.1.0', '1.0.3', true)).toEqual([])
  })
})

describe('bundled release notes', () => {
  it('has French and English notes for the running version', () => {
    const current = bundledNotes().find((n) => n.version === version)
    expect(current, `src/renderer/src/features/whatsnew/notes/${version}.json`).toBeDefined()
    for (const n of bundledNotes()) {
      for (const lang of ['fr', 'en'] as const) {
        expect(n.note[lang].title).toBeTruthy()
        expect(n.note[lang].items.length).toBeGreaterThan(0)
      }
    }
  })
  it('lists the newest version first', () => {
    const versions = bundledNotes().map((n) => n.version)
    expect(versions[0]).toBe(version)
  })
})
