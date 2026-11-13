import { describe, expect, it } from 'vitest'
import { migrateLegacyStorage } from './storageMigration'

describe('migrateLegacyStorage', () => {
  it('copies lumen.* values to orphyn.* and deletes the old keys', () => {
    localStorage.clear()
    localStorage.setItem('lumen.theme.mode', 'dark')
    localStorage.setItem('lumen.language', 'fr')
    localStorage.setItem('orphyn.language', 'en')
    localStorage.setItem('other', 'x')
    migrateLegacyStorage(localStorage)
    expect(localStorage.getItem('orphyn.theme.mode')).toBe('dark')
    expect(localStorage.getItem('orphyn.language')).toBe('en')
    expect(localStorage.getItem('lumen.theme.mode')).toBeNull()
    expect(localStorage.getItem('lumen.language')).toBeNull()
    expect(localStorage.getItem('other')).toBe('x')
  })
})
