import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createContext, useContext, type ReactNode } from 'react'
import { createMemoryRouter, RouterProvider, useSearchParams } from 'react-router'
import '@renderer/i18n'
import {
  AppShell,
  registerShellExtension,
  ShellNowPlayingSlot,
  ShellPage,
  ShellServerSwitcherSlot,
  useShellImmersive,
} from '.'

function SearchProbe() {
  const [params] = useSearchParams()
  return <ShellPage title="Search">{`q=${params.get('q')}`}</ShellPage>
}

function ImmersivePage() {
  useShellImmersive()
  return <p>Player</p>
}

function SlotPage() {
  return (
    <ShellPage title="Slots">
      <ShellNowPlayingSlot>
        <span>Slot player</span>
      </ShellNowPlayingSlot>
      <ShellServerSwitcherSlot>
        <span>Slot switcher</span>
      </ShellServerSwitcherSlot>
    </ShellPage>
  )
}

function renderShell(path = '/video') {
  const router = createMemoryRouter(
    [
      {
        element: <AppShell />,
        children: [
          { path: '/video', element: <ShellPage title="Watch Now">video</ShellPage> },
          { path: '/music/albums', element: <ShellPage title="Albums">albums</ShellPage> },
          { path: '/search', element: <SearchProbe /> },
          { path: '/play', element: <ImmersivePage /> },
          { path: '/slots', element: <SlotPage /> },
        ],
      },
    ],
    { initialEntries: [path] },
  )
  return { router, ...render(<RouterProvider router={router} />) }
}

describe('AppShell', () => {
  it('renders the Apple Music-style sidebar with Watch and Music sections', () => {
    renderShell('/music/albums')
    const nav = screen.getByRole('navigation', { name: 'Main' })
    const watch = within(nav).getByRole('region', { name: 'Watch' })
    const music = within(nav).getByRole('region', { name: 'Music' })
    expect(
      within(watch)
        .getAllByRole('link')
        .map((a) => a.textContent),
    ).toEqual(['Home', 'Movies', 'TV Shows'])
    expect(within(watch).getByRole('link', { name: 'Home' })).toHaveAttribute('href', '/video')
    expect(within(watch).getByRole('link', { name: 'Movies' })).toHaveAttribute(
      'href',
      '/video/library/movies',
    )
    expect(
      within(music)
        .getAllByRole('link')
        .map((a) => a.getAttribute('href')),
    ).toEqual(['/music', '/music/albums', '/music/artists', '/music/playlists', '/music/songs'])
    expect(within(music).getByRole('link', { name: 'Albums' })).toHaveAttribute(
      'aria-current',
      'page',
    )
    expect(within(nav).getByRole('link', { name: 'Settings' })).toHaveAttribute('href', '/settings')
    expect(within(nav).getByRole('searchbox', { name: 'Search Jellyra' })).toBeInTheDocument()
  })

  it('shows the large title, and collapses it into the toolbar on scroll', async () => {
    const { container } = renderShell('/music/albums')
    expect(screen.getByRole('heading', { level: 1, name: 'Albums' })).toBeInTheDocument()
    const toolbarTitle = container.querySelector('.jr-toolbar__title')!
    expect(container.querySelector('.jr-page__toolbar .jr-toolbar')).toHaveClass('jr-drag')
    expect(document.title).toBe('Albums – Jellyra')

    // jsdom has no layout: place the end of the large title by hand.
    const scroller = screen.getByRole('main')
    const sentinel = container.querySelector('.jr-page__sentinel') as HTMLElement
    scroller.getBoundingClientRect = () => ({ top: 0 }) as DOMRect
    sentinel.getBoundingClientRect = () => ({ top: 120 }) as DOMRect
    fireEvent.scroll(scroller)
    await waitFor(() => expect(toolbarTitle).not.toHaveClass('jr-toolbar__title--visible'))
    expect(toolbarTitle).toHaveAttribute('aria-hidden', 'true')

    sentinel.getBoundingClientRect = () => ({ top: 20 }) as DOMRect
    fireEvent.scroll(scroller)
    await waitFor(() => expect(toolbarTitle).toHaveClass('jr-toolbar__title--visible'))

    sentinel.getBoundingClientRect = () => ({ top: 200 }) as DOMRect
    fireEvent.scroll(scroller)
    await waitFor(() => expect(toolbarTitle).not.toHaveClass('jr-toolbar__title--visible'))
  })

  it('navigates to /search?q= while typing in the sidebar search', async () => {
    const user = userEvent.setup()
    const { router } = renderShell('/video')
    await user.type(screen.getByRole('searchbox', { name: 'Search Jellyra' }), 'dune')
    expect(router.state.location.pathname).toBe('/search')
    expect(screen.getByText('q=dune')).toBeInTheDocument()
    // Typing replaced the history entry instead of pushing one per letter.
    act(() => void router.navigate(-1))
    await waitFor(() => expect(router.state.location.pathname).toBe('/video'))
    expect(screen.getByRole('searchbox', { name: 'Search Jellyra' })).toHaveValue('')
  })

  it('focuses the search with Ctrl+F', async () => {
    const user = userEvent.setup()
    renderShell('/video')
    await user.keyboard('{Control>}f{/Control}')
    expect(screen.getByRole('searchbox', { name: 'Search Jellyra' })).toHaveFocus()
  })

  it('lets features plug in through the registry without editing the shell', async () => {
    const Ctx = createContext('none')
    const Provider = ({ children }: { children: ReactNode }) => (
      <Ctx.Provider value="from provider">{children}</Ctx.Provider>
    )
    const MiniPlayer = () => <div>Mini player {useContext(Ctx)}</div>
    const unregister = registerShellExtension({
      id: 'test-music',
      provider: Provider,
      nowPlaying: MiniPlayer,
      serverSwitcher: () => <button>Home server</button>,
      toolbarItems: () => <button>Cast</button>,
      sidebarItems: [{ id: 'cast', section: 'more', label: 'Cast devices', to: '/cast' }],
    })
    try {
      renderShell('/video')
      const nowPlaying = screen.getByRole('region', { name: 'Now Playing' })
      expect(nowPlaying).toHaveTextContent('Mini player from provider')
      expect(screen.getByRole('button', { name: 'Home server' })).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Cast' })).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Cast devices' })).toHaveAttribute('href', '/cast')

      act(() => unregister())
      await waitFor(() => expect(screen.queryByText(/Mini player/)).toBeNull())
      expect(screen.queryByRole('link', { name: 'Cast devices' })).toBeNull()
    } finally {
      unregister()
    }
  })

  it('renders slot children from a page into the now-playing bar and the sidebar', () => {
    renderShell('/slots')
    expect(screen.getByRole('region', { name: 'Now Playing' })).toHaveTextContent('Slot player')
    expect(
      within(screen.getByRole('navigation', { name: 'Main' })).getByText('Slot switcher'),
    ).toBeInTheDocument()
  })

  it('hides the chrome while a page is immersive', async () => {
    const { router, container } = renderShell('/play')
    expect(container.querySelector('.jr-shell')).toHaveClass('jr-shell--immersive')
    expect(screen.queryByRole('navigation', { name: 'Main' })).toBeNull()
    act(() => void router.navigate('/video'))
    await waitFor(() =>
      expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument(),
    )
  })
})
