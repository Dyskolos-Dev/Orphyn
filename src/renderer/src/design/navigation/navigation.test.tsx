import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Disc3, House } from 'lucide-react'
import { renderWithRouter } from '../test-utils'
import { Sidebar, SidebarItem, SidebarSection } from './Sidebar'
import { Toolbar } from './Toolbar'

describe('Sidebar', () => {
  function Nav() {
    return (
      <Sidebar
        aria-label="Main"
        header={<input aria-label="Search" />}
        footer={<span>footer</span>}
      >
        <SidebarSection title="Music" collapsible>
          <SidebarItem icon={<House />} label="Home" to="/music" />
          <SidebarItem icon={<Disc3 />} label="Albums" to="/music/albums" />
        </SidebarSection>
      </Sidebar>
    )
  }

  it('marks the current route and only matches Home exactly', () => {
    renderWithRouter(<Nav />, { route: '/music/albums' })
    expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument()
    const albums = screen.getByRole('link', { name: 'Albums' })
    expect(albums).toHaveAttribute('aria-current', 'page')
    expect(albums).toHaveClass('jr-sidebar-item__link--active')
    expect(screen.getByRole('link', { name: 'Home' })).not.toHaveAttribute('aria-current')
  })

  it('collapses a section from its heading button', async () => {
    const user = userEvent.setup()
    renderWithRouter(<Nav />)
    const toggle = screen.getByRole('button', { name: 'Music' })
    expect(toggle).toHaveAttribute('aria-expanded', 'true')
    await user.click(toggle)
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('link', { name: 'Albums' })).toBeNull()
    await user.keyboard('{Enter}')
    expect(screen.getByRole('link', { name: 'Albums' })).toBeInTheDocument()
  })

  it('supports button items with a manual active state', async () => {
    const onClick = vi.fn()
    renderWithRouter(
      <Sidebar aria-label="Main">
        <SidebarSection>
          <SidebarItem label="Queue" active onClick={onClick} />
        </SidebarSection>
      </Sidebar>,
    )
    const btn = screen.getByRole('button', { name: 'Queue' })
    expect(btn).toHaveAttribute('aria-current', 'page')
    await userEvent.click(btn)
    expect(onClick).toHaveBeenCalledOnce()
  })
})

describe('Toolbar', () => {
  it('is a drag region whose title is hidden until shown', () => {
    const { container, rerender } = renderWithRouter(
      <Toolbar title="Albums" showTitle={false} trailing={<button>Sort</button>} />,
    )
    const bar = container.firstChild as HTMLElement
    expect(bar).toHaveClass('jr-drag')
    expect(screen.getByText('Albums')).toHaveAttribute('aria-hidden', 'true')
    rerender(<Toolbar title="Albums" showTitle trailing={<button>Sort</button>} />)
    expect(screen.getByText('Albums')).toHaveClass('jr-toolbar__title--visible')
    expect(screen.getByRole('button', { name: 'Sort' })).toBeInTheDocument()
  })
})
