// @vitest-environment node
import { describe, expect, it, vi } from 'vitest'

vi.mock('electron', () => ({ app: {} }))

const { mergeFeatureList, userDataOverride, MEDIA_FEATURES } = await import('./startup')

describe('mergeFeatureList', () => {
  it('adds the media features to an empty list', () => {
    expect(mergeFeatureList('', MEDIA_FEATURES)).toBe(
      'HardwareMediaKeyHandling,MediaSessionService',
    )
  })

  it('keeps existing features first and does not duplicate', () => {
    expect(mergeFeatureList('Foo, HardwareMediaKeyHandling', MEDIA_FEATURES)).toBe(
      'Foo,HardwareMediaKeyHandling,MediaSessionService',
    )
  })
})

describe('userDataOverride', () => {
  it('is null when unset or blank', () => {
    expect(userDataOverride({})).toBeNull()
    expect(userDataOverride({ JELLYRA_USER_DATA_DIR: '  ' })).toBeNull()
  })

  it('ignores relative paths', () => {
    expect(userDataOverride({ JELLYRA_USER_DATA_DIR: 'profile' })).toBeNull()
  })

  it('returns an absolute path', () => {
    const dir = process.platform === 'win32' ? 'C:\\tmp\\jellyra' : '/tmp/jellyra'
    expect(userDataOverride({ JELLYRA_USER_DATA_DIR: dir })).toBe(dir)
  })
})
