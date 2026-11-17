import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import '@renderer/i18n'
import { queryClient, useSession } from '@renderer/jellyfin'
import { AddRowSheet, HomeEditActions, HomeEditList } from './HomeEditor'
import { layoutStorageKey } from './layout'
import { useHomeEditor } from './useHomeLayout'
import { useState } from 'react'

const KEY = layoutStorageKey('music', 'srv', 'user')

function Harness() {
  const editor = useHomeEditor('music')
  const [adding, setAdding] = useState(false)
  return (
    <>
      <HomeEditActions editor={editor} onAdd={() => setAdding(true)} />
      {editor.editing ? (
        <HomeEditList scope="music" editor={editor} />
      ) : (
        <ol data-testid="saved">
          {editor.layout.rows
            .filter((r) => !r.hidden)
            .map((r) => (
              <li key={r.id}>{r.id}</li>
            ))}
        </ol>
      )}
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

describe('home edit mode', () => {
  it('hides, reorders with the keyboard, adds a row, and Done saves per server and user', async () => {
    const user = userEvent.setup()
    renderHarness()
    expect(saved()[0]).toBe('recentlyPlayed')

    await user.click(screen.getByRole('button', { name: /edit|modifier/i }))
    // Hide "Recently Added".
    await user.click(screen.getByRole('button', { name: /hide recently added/i }))
    // Move "Playlists" to the top with the arrow keys on its handle.
    const handle = screen.getByRole('button', { name: /^move playlists/i })
    handle.focus()
    for (let i = 0; i < 5; i++) await user.keyboard('{ArrowUp}')
    // Add a random albums row.
    await user.click(screen.getByRole('button', { name: /add a row/i }))
    const dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('button', { name: /random albums/i }))

    // Nothing is saved before Done.
    expect(localStorage.getItem(KEY)).toBeNull()
    await user.click(screen.getByRole('button', { name: /^done$/i }))

    await waitFor(() =>
      expect(saved()).toEqual([
        'playlists',
        'recentlyPlayed',
        'instantMix',
        'favAlbums',
        'favArtists',
        expect.stringMatching(/^randomAlbums-/),
      ]),
    )
    expect(JSON.parse(localStorage.getItem(KEY) ?? '{}').rows).toHaveLength(7)
  })

  it('Cancel drops the changes and Reset restores the defaults', async () => {
    const user = userEvent.setup()
    renderHarness()
    await user.click(screen.getByRole('button', { name: /edit|modifier/i }))
    await user.click(screen.getByRole('button', { name: /remove playlists/i }))
    expect(screen.queryByRole('button', { name: /remove playlists/i })).toBeNull()
    await user.click(screen.getByRole('button', { name: /^reset$/i }))
    expect(screen.getByRole('button', { name: /remove playlists/i })).toBeTruthy()
    await user.click(
      screen.getByRole('button', { name: /remove favourite songs|remove recently played/i }),
    )
    await user.click(screen.getByRole('button', { name: /^cancel$/i }))
    expect(saved()).toContain('recentlyPlayed')
    expect(localStorage.getItem(KEY)).toBeNull()
  })
})
