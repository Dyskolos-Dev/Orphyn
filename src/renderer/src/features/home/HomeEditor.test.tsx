import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import '@renderer/i18n'
import { queryClient, useSession } from '@renderer/jellyfin'
import { AddRowSheet, HomeEditList } from './HomeEditor'
import { layoutStorageKey } from './layout'
import { useHomeEditor } from './useHomeLayout'
import { useState } from 'react'

const KEY = layoutStorageKey('music', 'srv', 'user')

function Harness() {
  const editor = useHomeEditor('music')
  const [adding, setAdding] = useState(false)
  return (
    <>
      <button onClick={() => setAdding(true)}>Add a row</button>
      <button onClick={editor.reset}>Reset</button>
      <HomeEditList scope="music" editor={editor} />
      <ol data-testid="saved">
        {editor.layout.rows
          .filter((r) => !r.hidden)
          .map((r) => (
            <li key={r.id}>{r.id}</li>
          ))}
      </ol>
      <AddRowSheet
        open={adding}
        onClose={() => setAdding(false)}
        scope="music"
        layout={editor.layout}
        onAdd={editor.add}
      />
    </>
  )
}

const saved = () =>
  within(screen.getByTestId('saved'))
    .getAllByRole('listitem')
    .map((l) => l.textContent)

beforeEach(() => {
  localStorage.clear()
  useSession.setState({
    account: { serverId: 'srv', userId: 'user' } as never,
  })
})

function renderHarness() {
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <Harness />
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

describe('home editing (Settings > Home)', () => {
  it('hides, reorders with the keyboard, adds a row; every change is saved at once, per server and user', async () => {
    const user = userEvent.setup()
    renderHarness()
    expect(saved()[0]).toBe('recentlyPlayed')
    expect(localStorage.getItem(KEY)).toBeNull()

    await user.click(screen.getByRole('button', { name: /hide recently added/i }))
    expect(saved()).not.toContain('recentlyAdded')
    expect(JSON.parse(localStorage.getItem(KEY) ?? '{}').rows).toBeTruthy()

    const handle = screen.getByRole('button', { name: /^move playlists/i })
    handle.focus()
    for (let i = 0; i < 5; i++) await user.keyboard('{ArrowUp}')

    await user.click(screen.getByRole('button', { name: /add a row/i }))
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('button', { name: /random albums/i }))

    await waitFor(() =>
      expect(saved()).toEqual([
        'recentlyPlayed',
        'mixForYou',
        'mixDaily',
        'playlists',
        'instantMix',
        'mixGenres',
        'favAlbums',
        'favArtists',
        expect.stringMatching(/^randomAlbums-/),
      ]),
    )
    expect(JSON.parse(localStorage.getItem(KEY) ?? '{}').rows).toHaveLength(10)
  })

  it('the "…" menu removes a row and sets its options; Reset restores the defaults', async () => {
    const user = userEvent.setup()
    renderHarness()
    const menuOf = (title: RegExp) =>
      user.click(
        within(screen.getByText(title).closest('li') as HTMLElement).getByRole('button', {
          name: /row options|options/i,
        }),
      )
    await menuOf(/^Playlists$/)
    await user.click(await screen.findByRole('menuitem', { name: /remove playlists/i }))
    expect(saved()).not.toContain('playlists')

    await menuOf(/^Recently Played$/)
    await user.click(await screen.findByRole('menuitemcheckbox', { name: '10' }))
    expect(JSON.parse(localStorage.getItem(KEY) ?? '{}').rows[0].count).toBe(10)

    await user.click(screen.getByRole('button', { name: /^reset$/i }))
    expect(saved()).toContain('playlists')
    expect(JSON.parse(localStorage.getItem(KEY) ?? '{}').rows[0].count).toBeUndefined()
  })
})
