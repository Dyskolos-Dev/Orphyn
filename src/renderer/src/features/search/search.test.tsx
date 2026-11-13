import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router'
import type { SearchResults } from '@renderer/jellyfin'
import '@renderer/i18n'

const mocks = vi.hoisted(() => ({
  data: undefined as unknown,
  isPending: false,
  isError: false,
  term: '',
  played: [] as string[],
}))

vi.mock('@renderer/features/music', () => ({
  musicPlayer: { playItem: (id: string) => mocks.played.push(id) },
}))

vi.mock('@renderer/jellyfin', () => ({
  useSearch: (term: string) => {
    mocks.term = term
    return {
      data: mocks.data,
      isPending: mocks.isPending,
      isError: mocks.isError,
      refetch: vi.fn(),
    }
  },
  imageUrl: () => undefined,
}))

import { groupResults, matchScore, routeFor } from './grouping'
import {
  RECENT_SEARCHES_KEY,
  addRecentSearch,
  getRecentSearches,
  reloadRecentSearches,
} from './recents'
import { SearchPage } from './SearchPage'

const results: SearchResults = {
  movies: [
    { Id: 'm1', Name: 'Dune', Type: 'Movie', ProductionYear: 2021 },
    { Id: 'm2', Name: 'Dune: Part Two', Type: 'Movie', ProductionYear: 2024 },
  ],
  series: [{ Id: 's1', Name: 'Dune: Prophecy', Type: 'Series' }],
  episodes: [
    {
      Id: 'e1',
      Name: 'Pilot',
      Type: 'Episode',
      SeriesName: 'Dune: Prophecy',
      ParentIndexNumber: 1,
      IndexNumber: 1,
    },
  ],
  artists: [{ Id: 'ar1', Name: 'Dune', Type: 'MusicArtist' }],
  albums: [{ Id: 'al1', Name: 'Dune Sessions', Type: 'MusicAlbum', AlbumArtist: 'Hans' }],
  tracks: [{ Id: 't1', Name: 'Dune Theme', Type: 'Audio', AlbumId: 'al1', Album: 'Dune Sessions' }],
}

describe('groupResults', () => {
  it('orders the shelves Movies, Series, Episodes, Artists, Albums, Songs and skips empty ones', () => {
    const g = groupResults({ ...results, series: [], artists: [] }, 'dune')
    expect(g.sections.map((s) => s.id)).toEqual(['movies', 'episodes', 'albums', 'songs'])
    expect(g.total).toBe(5)
  })

  it('picks the exact name match as the top result, movies winning a tie', () => {
    const g = groupResults(results, 'dune')
    expect(g.top?.item.Id).toBe('m1')
    expect(g.top?.kind).toBe('movie')
  })

  it('prefers a better match over a higher-priority kind and ignores accents', () => {
    const g = groupResults(
      {
        ...results,
        movies: [{ Id: 'x', Name: 'Dune: Part Two' }],
        artists: [{ Id: 'a', Name: 'Édune' }],
      },
      'edune',
    )
    expect(g.top?.item.Id).toBe('a')
    expect(matchScore('Édune', 'EDUNE')).toBe(3)
    expect(matchScore('Part Two', 'two')).toBe(1)
    expect(matchScore('Nothing', 'zzz')).toBe(-1)
  })

  it('is empty without results', () => {
    expect(groupResults(undefined, 'x')).toEqual({ top: null, sections: [], total: 0 })
  })
})

describe('routeFor', () => {
  it('maps every kind to a documented route', () => {
    expect(routeFor('movie', { Id: '1' })).toBe('/video/item/1')
    expect(routeFor('series', { Id: '2' })).toBe('/video/item/2')
    expect(routeFor('episode', { Id: '3' })).toBe('/video/item/3')
    expect(routeFor('album', { Id: '4' })).toBe('/music/album/4')
    expect(routeFor('artist', { Id: '5' })).toBe('/music/artist/5')
    expect(routeFor('song', { Id: '6', AlbumId: '7' })).toBe('/music/album/7')
  })
})

describe('recent searches', () => {
  beforeEach(() => {
    localStorage.clear()
    reloadRecentSearches()
  })

  it('keeps the newest first, de-duplicates and persists', () => {
    addRecentSearch('dune')
    addRecentSearch('alien')
    addRecentSearch('DUNE')
    addRecentSearch('x')
    expect(getRecentSearches()).toEqual(['DUNE', 'alien'])
    expect(JSON.parse(localStorage.getItem(RECENT_SEARCHES_KEY) ?? '[]')).toEqual(['DUNE', 'alien'])
  })
})

function Where() {
  const { pathname } = useLocation()
  return <p data-testid="where">{pathname}</p>
}

function renderPage(url: string) {
  return render(
    <MemoryRouter initialEntries={[url]}>
      <Routes>
        <Route path="/search" element={<SearchPage />} />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  )
}

describe('SearchPage', () => {
  beforeEach(() => {
    localStorage.clear()
    reloadRecentSearches()
    mocks.data = results
    mocks.isPending = false
    mocks.isError = false
  })

  it('shows the top result and grouped shelves, and navigates to the right route', async () => {
    const user = userEvent.setup()
    renderPage('/search?q=dune')
    expect(await screen.findByRole('region', { name: 'Top Result' })).toBeInTheDocument()
    for (const name of ['Movies', 'Series', 'Episodes', 'Artists', 'Albums', 'Songs']) {
      expect(screen.getByRole('heading', { name })).toBeInTheDocument()
    }
    expect(mocks.term).toBe('dune')

    await user.click(screen.getByRole('link', { name: /Dune Sessions/ }))
    expect(screen.getByTestId('where')).toHaveTextContent('/music/album/al1')
    expect(getRecentSearches()).toEqual(['dune'])
  })

  it('opens an artist, an episode and a movie on their documented routes', async () => {
    const user = userEvent.setup()
    renderPage('/search?q=dune')
    const topLink = (await screen.findAllByRole('link')).find((l) =>
      l.className.includes('jr-search__top-card'),
    )
    expect(topLink).toHaveAttribute('href', '/video/item/m1')
    await user.click(screen.getByRole('link', { name: /Pilot/ }))
    expect(screen.getByTestId('where')).toHaveTextContent('/video/item/e1')
  })

  it('plays a song with the music player', async () => {
    const user = userEvent.setup()
    renderPage('/search?q=dune')
    const row = await screen.findByText('Dune Theme')
    await user.dblClick(row)
    expect(mocks.played).toContain('t1')
  })

  it('shows the no-results state', async () => {
    mocks.data = { movies: [], series: [], episodes: [], artists: [], albums: [], tracks: [] }
    renderPage('/search?q=zzzz')
    expect(await screen.findByText('No Results for “zzzz”')).toBeInTheDocument()
  })

  it('shows the landing state, then recent searches', async () => {
    const { unmount } = renderPage('/search')
    expect(await screen.findByText('Search Your Library')).toBeInTheDocument()
    unmount()

    addRecentSearch('alien')
    renderPage('/search')
    expect(await screen.findByRole('button', { name: /alien/ })).toBeInTheDocument()
    expect(screen.getByText('Recent Searches')).toBeInTheDocument()
  })

  it('shows an error state with a retry', async () => {
    mocks.isError = true
    renderPage('/search?q=dune')
    expect(await screen.findByText('Search Failed')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Try Again' })).toBeInTheDocument()
  })
})
