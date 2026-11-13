import { QueryClient } from '@tanstack/react-query'
import { describe, expect, it, vi } from 'vitest'
import { refreshLibrary, shouldRefreshOnFocus } from './refresh'

describe('refreshLibrary', () => {
  it('invalidates only the queries of the given server', async () => {
    const client = new QueryClient()
    const a = vi.fn(() => Promise.resolve('a'))
    const b = vi.fn(() => Promise.resolve('b'))
    await client.fetchQuery({ queryKey: ['jf', 'acc1', 'views'], queryFn: a, staleTime: 60_000 })
    await client.fetchQuery({ queryKey: ['jf', 'acc2', 'views'], queryFn: b, staleTime: 60_000 })
    client.setQueryData(['other'], 1)
    await refreshLibrary(client, 'acc1')
    expect(client.getQueryState(['jf', 'acc1', 'views'])?.isInvalidated).toBe(true)
    expect(client.getQueryState(['jf', 'acc2', 'views'])?.isInvalidated).toBe(false)
    expect(client.getQueryState(['other'])?.isInvalidated).toBe(false)
  })

  it('does nothing without an active server', async () => {
    const client = new QueryClient()
    const spy = vi.spyOn(client, 'invalidateQueries')
    await refreshLibrary(client, null)
    expect(spy).not.toHaveBeenCalled()
  })
})

describe('shouldRefreshOnFocus', () => {
  it('needs more than 5 minutes in the background', () => {
    expect(shouldRefreshOnFocus(null, 1_000_000)).toBe(false)
    expect(shouldRefreshOnFocus(0, 4 * 60_000)).toBe(false)
    expect(shouldRefreshOnFocus(0, 5 * 60_000 + 1)).toBe(true)
  })
})
