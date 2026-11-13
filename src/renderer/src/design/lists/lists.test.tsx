import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Toggle } from '../controls/Toggle'
import { renderWithRouter } from '../test-utils'
import { List, ListRow } from './ListRow'
import { formatDuration, TrackList, TrackRow } from './TrackRow'

describe('formatDuration', () => {
  it('formats m:ss and h:mm:ss', () => {
    expect(formatDuration(0)).toBe('0:00')
    expect(formatDuration(187.9)).toBe('3:07')
    expect(formatDuration(3723)).toBe('1:02:03')
    expect(formatDuration(Number.NaN)).toBe('--:--')
  })
})

describe('TrackRow', () => {
  function Album({ onPlay, onPause }: { onPlay: (n: number) => void; onPause?: () => void }) {
    return (
      <TrackList aria-label="Tracks">
        <TrackRow
          index={1}
          title="One More Time"
          artist="Daft Punk"
          duration={320}
          onPlay={() => onPlay(1)}
        />
        <TrackRow
          index={2}
          title="Aerodynamic"
          artist="Daft Punk"
          duration={212}
          isCurrent
          isPlaying
          explicit
          onPlay={() => onPlay(2)}
          onPause={onPause}
        />
        <TrackRow index={3} title="Digital Love" duration={301} onPlay={() => onPlay(3)} />
      </TrackList>
    )
  }

  it('shows number, title, artist and duration', () => {
    renderWithRouter(<Album onPlay={() => {}} />)
    const row = screen.getByRole('listitem', { name: 'One More Time, Daft Punk, 5:20' })
    expect(row).toHaveTextContent('1')
    expect(row).toHaveTextContent('5:20')
    expect(screen.getAllByRole('listitem')[1]).toHaveAttribute('aria-current', 'true')
    expect(screen.getByTitle('Explicit')).toBeInTheDocument()
  })

  it('plays on double-click, Enter and the hover play button', async () => {
    const user = userEvent.setup()
    const onPlay = vi.fn()
    renderWithRouter(<Album onPlay={onPlay} />)
    const [first, , third] = screen.getAllByRole('listitem')
    await user.dblClick(first!)
    expect(onPlay).toHaveBeenLastCalledWith(1)
    third!.focus()
    await user.keyboard('{Enter}')
    expect(onPlay).toHaveBeenLastCalledWith(3)
    await user.click(screen.getByRole('button', { name: 'Play Digital Love' }))
    expect(onPlay).toHaveBeenLastCalledWith(3)
    expect(onPlay).toHaveBeenCalledTimes(3)
  })

  it('pauses the current playing track instead of restarting it', async () => {
    const user = userEvent.setup()
    const onPlay = vi.fn()
    const onPause = vi.fn()
    renderWithRouter(<Album onPlay={onPlay} onPause={onPause} />)
    screen.getAllByRole('listitem')[1]!.focus()
    await user.keyboard('{Enter}')
    expect(onPause).toHaveBeenCalledOnce()
    expect(onPlay).not.toHaveBeenCalled()
  })

  it('moves between rows with the arrow keys', async () => {
    const user = userEvent.setup()
    renderWithRouter(<Album onPlay={() => {}} />)
    const rows = screen.getAllByRole('listitem')
    rows[0]!.focus()
    await user.keyboard('{ArrowDown}')
    expect(rows[1]).toHaveFocus()
    await user.keyboard('{ArrowDown}{ArrowDown}')
    expect(rows[2]).toHaveFocus()
    await user.keyboard('{ArrowUp}')
    expect(rows[1]).toHaveFocus()
  })
})

describe('ListRow', () => {
  it('renders links with a chevron, buttons, and static rows with controls', async () => {
    const onClick = vi.fn()
    renderWithRouter(
      <List variant="inset" header="Account" aria-label="Account">
        <ListRow title="Servers" subtitle="2 connected" to="/onboarding" />
        <ListRow title="Sign out" destructive onClick={onClick} />
        <ListRow
          title="Autoplay"
          trailing={<Toggle aria-label="Autoplay" checked onChange={() => {}} />}
        />
      </List>,
    )
    expect(screen.getByRole('link', { name: /Servers/ })).toHaveAttribute('href', '/onboarding')
    await userEvent.click(screen.getByRole('button', { name: 'Sign out' }))
    expect(onClick).toHaveBeenCalledOnce()
    expect(screen.getByRole('switch', { name: 'Autoplay' })).toBeInTheDocument()
    expect(screen.getAllByRole('listitem')).toHaveLength(3)
  })
})
