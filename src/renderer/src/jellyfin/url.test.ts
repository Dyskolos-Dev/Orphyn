import { describe, expect, it } from 'vitest'
import { normalizeServerUrl, urlCandidates } from './url'

describe('urlCandidates', () => {
  it('guesses :8096 over http for a bare host, then https, then plain http', () => {
    expect(urlCandidates('nas').slice(0, 3)).toEqual([
      'http://nas:8096',
      'https://nas',
      'http://nas',
    ])
  })

  it('keeps an explicit port and tries both schemes', () => {
    expect(urlCandidates('192.168.1.5:9000')).toEqual([
      'http://192.168.1.5:9000',
      'https://192.168.1.5:9000',
      'http://192.168.1.5:9000/jellyfin',
      'https://192.168.1.5:9000/jellyfin',
    ])
  })

  it('strips trailing slashes, query, hash and a pasted /web UI path', () => {
    expect(normalizeServerUrl('https://media.example.com/')).toBe('https://media.example.com')
    expect(normalizeServerUrl('https://media.example.com/web/index.html#!/home')).toBe(
      'https://media.example.com',
    )
    expect(normalizeServerUrl('  https://m.example.com?x=1  ')).toBe('https://m.example.com')
  })

  it('keeps an explicit base path and does not add /jellyfin on top', () => {
    const c = urlCandidates('https://example.com/jellyfin/')
    expect(c).toEqual(['https://example.com/jellyfin'])
  })

  it('adds the /jellyfin base path as a last-resort candidate', () => {
    const c = urlCandidates('https://example.com')
    expect(c).toEqual(['https://example.com', 'https://example.com/jellyfin'])
  })

  it('also tries :8096 for explicit http without port', () => {
    expect(urlCandidates('http://nas')).toContain('http://nas:8096')
  })

  it('supports IPv6 literals', () => {
    expect(normalizeServerUrl('http://[::1]:8096')).toBe('http://[::1]:8096')
  })

  it('rejects empty and non-http input', () => {
    expect(urlCandidates('')).toEqual([])
    expect(urlCandidates('   ')).toEqual([])
    expect(urlCandidates('ftp://nas')).toEqual([])
    expect(normalizeServerUrl('http://')).toBeNull()
  })
})
