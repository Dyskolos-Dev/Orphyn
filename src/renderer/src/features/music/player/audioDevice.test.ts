import { describe, expect, it } from 'vitest'
import { normalizeDevices, resolveSinkId } from './audioDevice'

const devs = [{ deviceId: 'a' }, { deviceId: 'b' }]

describe('resolveSinkId', () => {
  it('uses the system default when nothing is saved', () => {
    expect(resolveSinkId(null, devs)).toBe('')
    expect(resolveSinkId('default', devs)).toBe('')
  })
  it('uses the saved device while it is present', () => {
    expect(resolveSinkId('b', devs)).toBe('b')
  })
  it('falls back to default when the saved device is gone, and returns when it is back', () => {
    expect(resolveSinkId('c', devs)).toBe('')
    expect(resolveSinkId('c', [...devs, { deviceId: 'c' }])).toBe('c')
  })
  it('trusts the saved device while the list is unknown', () => {
    expect(resolveSinkId('c', null)).toBe('c')
  })
})

describe('normalizeDevices', () => {
  const label = (n: number) => `Output ${n}`
  it('keeps real outputs only and names blank ones', () => {
    const r = normalizeDevices(
      [
        { kind: 'audioinput', deviceId: 'm', label: 'Mic' },
        { kind: 'audiooutput', deviceId: 'default', label: 'Default' },
        { kind: 'audiooutput', deviceId: 'a', label: 'Speakers' },
        { kind: 'audiooutput', deviceId: 'b', label: '' },
      ],
      label,
    )
    expect(r.devices).toEqual([
      { deviceId: 'a', label: 'Speakers' },
      { deviceId: 'b', label: 'Output 2' },
    ])
    expect(r.labelsHidden).toBe(false)
  })
  it('flags hidden labels', () => {
    const r = normalizeDevices([{ kind: 'audiooutput', deviceId: 'a', label: '' }], label)
    expect(r.labelsHidden).toBe(true)
  })
})
