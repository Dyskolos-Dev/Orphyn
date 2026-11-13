import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useRef, useState } from 'react'
import '@renderer/i18n'
import { Button } from '../controls/Button'
import { TextField } from '../controls/TextField'
import { ContextMenu, MenuButton, type MenuItem } from './Menu'
import { computePosition, Popover } from './Popover'
import { Sheet } from './Sheet'
import { dismissToast, toast, ToastViewport } from './Toast'

describe('Sheet', () => {
  function Demo() {
    const [open, setOpen] = useState(false)
    return (
      <>
        <Button onClick={() => setOpen(true)}>Add server</Button>
        <Sheet
          open={open}
          onClose={() => setOpen(false)}
          title="Add a server"
          description="Enter its address"
          footer={<Button onClick={() => setOpen(false)}>Connect</Button>}
        >
          <TextField label="Address" />
        </Sheet>
      </>
    )
  }

  it('opens as a labelled modal dialog, focusing the first field', async () => {
    const user = userEvent.setup()
    render(<Demo />)
    await user.click(screen.getByRole('button', { name: 'Add server' }))
    const dialog = screen.getByRole('dialog', { name: 'Add a server' })
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    expect(dialog).toHaveAccessibleDescription('Enter its address')
    expect(screen.getByLabelText('Address')).toHaveFocus()
  })

  it('traps Tab inside and restores focus on Escape', async () => {
    const user = userEvent.setup()
    render(<Demo />)
    const opener = screen.getByRole('button', { name: 'Add server' })
    await user.click(opener)
    // Focus order inside: Close, Address, Connect; Tab on Connect wraps to Close.
    await user.tab()
    expect(screen.getByRole('button', { name: 'Connect' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus()
    await user.tab()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Connect' })).toHaveFocus()
    await user.tab({ shift: true })
    expect(screen.getByLabelText('Address')).toHaveFocus()
    await user.tab({ shift: true })
    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus()
    // Shift+Tab on the first element wraps to the last.
    await user.tab({ shift: true })
    expect(screen.getByRole('button', { name: 'Connect' })).toHaveFocus()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(opener).toHaveFocus()
  })

  it('closes with the close button', async () => {
    const user = userEvent.setup()
    render(<Demo />)
    await user.click(screen.getByRole('button', { name: 'Add server' }))
    await user.click(screen.getByRole('button', { name: 'Close' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  })
})

describe('Popover', () => {
  function Demo() {
    const ref = useRef<HTMLButtonElement>(null)
    const [open, setOpen] = useState(false)
    return (
      <>
        <button ref={ref} onClick={() => setOpen((o) => !o)}>
          Queue
        </button>
        <button>Outside</button>
        <Popover open={open} onClose={() => setOpen(false)} anchor={ref} aria-label="Up Next">
          <button>Clear</button>
        </Popover>
      </>
    )
  }

  it('toggles from its anchor, closes on outside click and Escape', async () => {
    const user = userEvent.setup()
    render(<Demo />)
    await user.click(screen.getByRole('button', { name: 'Queue' }))
    expect(screen.getByRole('dialog', { name: 'Up Next' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Clear' })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Outside' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())

    await user.click(screen.getByRole('button', { name: 'Queue' }))
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(screen.getByRole('button', { name: 'Queue' })).toHaveFocus()
  })

  it('keeps the Escape that dismisses it from the page (e.g. leaving the video player)', async () => {
    const pageEscape = vi.fn()
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && pageEscape()
    // Registered before the popover opens, like a page-level shortcut.
    window.addEventListener('keydown', onKey)
    try {
      const user = userEvent.setup()
      render(<Demo />)
      await user.click(screen.getByRole('button', { name: 'Queue' }))
      await user.keyboard('{Escape}')
      await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
      expect(pageEscape).not.toHaveBeenCalled()
      await user.keyboard('{Escape}')
      expect(pageEscape).toHaveBeenCalledTimes(1)
    } finally {
      window.removeEventListener('keydown', onKey)
    }
  })

  it('flips above the anchor and clamps inside the viewport', () => {
    const anchor = { top: 700, bottom: 730, left: 950, right: 990, width: 40, height: 30 }
    const pos = computePosition(anchor, { width: 200, height: 150 }, 'bottom-start', 6, {
      width: 1000,
      height: 800,
    })
    expect(pos.side).toBe('top')
    expect(pos.top).toBe(700 - 6 - 150)
    expect(pos.left).toBe(1000 - 200 - 8)
  })
})

describe('Menu', () => {
  const onSelect = { add: vi.fn(), love: vi.fn(), del: vi.fn() }
  const items: MenuItem[] = [
    { label: 'Add to Library', onSelect: onSelect.add },
    { label: 'Love', checked: false, onSelect: onSelect.love },
    { type: 'separator' },
    { label: 'Delete', destructive: true, disabled: true, onSelect: onSelect.del },
  ]

  beforeEach(() => Object.values(onSelect).forEach((f) => f.mockClear()))

  it('MenuButton opens a keyboard-navigable menu', async () => {
    const user = userEvent.setup()
    render(<MenuButton label="More" items={items} />)
    const trigger = screen.getByRole('button', { name: 'More' })
    expect(trigger).toHaveAttribute('aria-haspopup', 'menu')
    await user.click(trigger)
    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    const menu = screen.getByRole('menu', { name: 'More' })
    expect(menu).toBeInTheDocument()
    expect(screen.getByRole('menuitem', { name: 'Add to Library' })).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(screen.getByRole('menuitemcheckbox', { name: 'Love' })).toHaveFocus()
    // Disabled items are skipped.
    await user.keyboard('{ArrowDown}')
    expect(screen.getByRole('menuitem', { name: 'Add to Library' })).toHaveFocus()
    await user.keyboard('l')
    expect(screen.getByRole('menuitemcheckbox', { name: 'Love' })).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(onSelect.love).toHaveBeenCalledOnce()
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(trigger).toHaveFocus()
  })

  it('ContextMenu opens on right click at the pointer and ignores disabled items', async () => {
    const user = userEvent.setup()
    render(
      <ContextMenu items={items} aria-label="Track actions">
        <div>Track</div>
      </ContextMenu>,
    )
    fireEvent.contextMenu(screen.getByText('Track'), { clientX: 120, clientY: 80 })
    expect(screen.getByRole('menu', { name: 'Track actions' })).toBeInTheDocument()
    await user.click(screen.getByRole('menuitem', { name: 'Delete' }))
    expect(onSelect.del).not.toHaveBeenCalled()
    await user.click(screen.getByRole('menuitem', { name: 'Add to Library' }))
    expect(onSelect.add).toHaveBeenCalledOnce()
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
  })

  it('ContextMenu opens from the keyboard with Shift+F10', async () => {
    const user = userEvent.setup()
    render(
      <ContextMenu items={items}>
        <button>Row</button>
      </ContextMenu>,
    )
    screen.getByRole('button', { name: 'Row' }).focus()
    await user.keyboard('{Shift>}{F10}{/Shift}')
    expect(screen.getByRole('menu')).toBeInTheDocument()
  })
})

describe('Toast', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('announces, runs its action, and can be dismissed', async () => {
    const user = userEvent.setup()
    const undo = vi.fn()
    render(<ToastViewport />)
    act(() => {
      toast({ title: 'Added to Library', action: { label: 'Undo', onClick: undo } })
    })
    expect(screen.getByRole('status')).toHaveTextContent('Added to Library')
    await user.click(screen.getByRole('button', { name: 'Undo' }))
    expect(undo).toHaveBeenCalledOnce()
    await waitFor(() => expect(screen.queryByText('Added to Library')).toBeNull())

    let id = 0
    act(() => {
      id = toast({ title: 'Cannot reach server', tone: 'error', duration: Infinity })
    })
    expect(screen.getByRole('alert')).toHaveTextContent('Cannot reach server')
    act(() => dismissToast(id))
    await waitFor(() => expect(screen.queryByText('Cannot reach server')).toBeNull())
  })

  it('hides itself after its duration', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    render(<ToastViewport />)
    act(() => {
      toast({ title: 'Downloaded', duration: 1000 })
    })
    expect(screen.getByText('Downloaded')).toBeInTheDocument()
    act(() => {
      vi.advanceTimersByTime(1200)
    })
    await waitFor(() => expect(screen.queryByText('Downloaded')).toBeNull())
  })
})
