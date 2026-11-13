import { describe, expect, it, vi } from 'vitest'

vi.mock('electron', () => ({
  app: { name: 'Orphyn' },
  BrowserWindow: { getFocusedWindow: () => null, getAllWindows: () => [] },
  Menu: {},
  shell: { openExternal: vi.fn() },
}))

import { buildMenuTemplate } from './menu'

const labels = (platform: NodeJS.Platform) =>
  buildMenuTemplate(platform).map((m) => m.label ?? m.role)

describe('application menu', () => {
  it('has the app menu with Settings (Cmd+,) on macOS', () => {
    const [app] = buildMenuTemplate('darwin')
    expect(app?.label).toBe('Orphyn')
    const items = app?.submenu as { accelerator?: string; role?: string }[]
    expect(items.some((i) => i.accelerator === 'Command+,')).toBe(true)
    expect(items.some((i) => i.role === 'quit')).toBe(true)
    expect(labels('darwin')).toEqual(['Orphyn', 'Edit', 'View', 'Window', 'help'])
  })

  it('keeps Edit roles and a Ctrl+R refresh elsewhere', () => {
    expect(labels('linux')).toEqual(['File', 'Edit', 'View', 'Window', 'help'])
    const view = buildMenuTemplate('win32').find((m) => m.label === 'View')
    expect((view?.submenu as { accelerator?: string }[])[0]?.accelerator).toBe('Ctrl+R')
  })
})
