import { fireEvent, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Button } from '../controls/Button'
import { renderWithRouter } from '../test-utils'
import { Artwork, initials } from './Artwork'
import { LandscapeCard, PosterCard, RoundCard, SquareCard } from './cards'
import { Grid } from './Grid'
import { HeroBanner } from './HeroBanner'
import { Shelf } from './Shelf'

describe('Artwork', () => {
  it('starts as a placeholder and fades in once loaded', () => {
    const { container } = renderWithRouter(<Artwork src="https://x/a.jpg" alt="Cover" />)
    const box = container.querySelector('.jr-artwork')!
    const img = screen.getByRole('img', { name: 'Cover' })
    expect(img).toHaveAttribute('loading', 'lazy')
    expect(box).toHaveAttribute('data-status', 'loading')
    fireEvent.load(img)
    expect(box).toHaveAttribute('data-status', 'loaded')
  })

  it('falls back gracefully on error and without a URL', () => {
    const { container, rerender } = renderWithRouter(
      <Artwork src="https://x/broken.jpg" alt="Cover" fallback={<span>DP</span>} />,
    )
    fireEvent.error(screen.getByRole('img', { name: 'Cover' }))
    expect(container.querySelector('.jr-artwork')).toHaveAttribute('data-status', 'error')
    expect(screen.getByRole('img', { name: 'Cover' })).toHaveTextContent('DP')

    // A new URL retries.
    rerender(<Artwork src="https://x/b.jpg" alt="Cover" />)
    expect(container.querySelector('img')).toHaveAttribute('src', 'https://x/b.jpg')

    rerender(<Artwork src={null} />)
    expect(container.querySelector('img')).toBeNull()
    expect(container.querySelector('.jr-artwork__fallback')).toBeInTheDocument()
  })

  it('computes initials', () => {
    expect(initials('Daft Punk')).toBe('DP')
    expect(initials('  björk ')).toBe('B')
    expect(initials('The Rolling Stones')).toBe('TS')
  })
})

describe('cards', () => {
  it('PosterCard links to its route and shows progress', () => {
    renderWithRouter(
      <PosterCard
        title="Dune"
        subtitle="2021"
        image="https://x/p.jpg"
        to="/video/item/1"
        progress={0.42}
      />,
    )
    const link = screen.getByRole('link', { name: /Dune/ })
    expect(link).toHaveAttribute('href', '/video/item/1')
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '42')
    expect(link.querySelector('.jr-artwork')).toHaveStyle({ aspectRatio: '2 / 3' })
  })

  it('PosterCard without text keeps an accessible name', () => {
    renderWithRouter(<PosterCard title="Dune" hideText to="/video/item/1" />)
    expect(screen.getByRole('link', { name: 'Dune' })).toBeInTheDocument()
    expect(screen.queryByText('Dune')).toBeNull()
  })

  it('LandscapeCard is 16:9 with a caption and an inline progress bar', () => {
    renderWithRouter(
      <LandscapeCard
        title="S1 E3"
        subtitle="The Office"
        caption="24 min left"
        progress={0.5}
        onClick={() => {}}
      />,
    )
    const btn = screen.getByRole('button', { name: /S1 E3/ })
    expect(btn.querySelector('.jr-artwork')).toHaveStyle({ aspectRatio: '16 / 9' })
    expect(screen.getByText('24 min left')).toBeInTheDocument()
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '50')
  })

  it('SquareCard has a separate hover play button that does not navigate', async () => {
    const onPlay = vi.fn()
    const onMore = vi.fn()
    renderWithRouter(
      <SquareCard
        title="Discovery"
        subtitle="Daft Punk"
        to="/music/album/7"
        onPlay={onPlay}
        onMore={onMore}
      />,
    )
    await userEvent.click(screen.getByRole('button', { name: 'Play Discovery' }))
    expect(onPlay).toHaveBeenCalledOnce()
    await userEvent.click(screen.getByRole('button', { name: 'More for Discovery' }))
    expect(onMore).toHaveBeenCalledOnce()
    expect(screen.getByRole('link', { name: /Discovery/ })).toHaveAttribute(
      'href',
      '/music/album/7',
    )
  })

  it('RoundCard is circular and falls back to initials', () => {
    const { container } = renderWithRouter(<RoundCard title="Daft Punk" to="/music/artist/3" />)
    expect(container.querySelector('.jr-artwork--circle')).toBeInTheDocument()
    expect(screen.getByText('DP')).toBeInTheDocument()
  })
})

describe('Shelf', () => {
  const posters = ['A', 'B', 'C'].map((t) => (
    <PosterCard key={t} title={t} to={`/video/item/${t}`} />
  ))

  it('renders a titled list with a See All link', () => {
    renderWithRouter(
      <Shelf title="Up Next" seeAllTo="/video/library/movies">
        {posters}
      </Shelf>,
    )
    const region = screen.getByRole('region', { name: 'Up Next' })
    expect(within(region).getByRole('heading', { level: 2, name: 'Up Next' })).toBeInTheDocument()
    expect(within(region).getAllByRole('listitem')).toHaveLength(3)
    expect(within(region).getByRole('link', { name: 'See All' })).toHaveAttribute(
      'href',
      '/video/library/movies',
    )
  })

  it('moves focus between items with the arrow keys', async () => {
    const user = userEvent.setup()
    renderWithRouter(<Shelf title="Row">{posters}</Shelf>)
    const [a, b, c] = screen.getAllByRole('link')
    a!.focus()
    await user.keyboard('{ArrowRight}')
    expect(b).toHaveFocus()
    await user.keyboard('{ArrowRight}{ArrowRight}')
    expect(c).toHaveFocus()
    await user.keyboard('{ArrowLeft}')
    expect(b).toHaveFocus()
  })

  it('shows the chevrons only when there is more to scroll, and pages on click', () => {
    const { container } = renderWithRouter(<Shelf title="Row">{posters}</Shelf>)
    const list = container.querySelector('.jr-shelf__list') as HTMLElement
    const back = container.querySelector('[aria-label="Scroll back"]') as HTMLElement
    const forward = container.querySelector('[aria-label="Scroll forward"]') as HTMLElement
    expect(back).not.toBeVisible()
    expect(forward).not.toBeVisible()

    Object.defineProperty(list, 'clientWidth', { value: 500, configurable: true })
    Object.defineProperty(list, 'scrollWidth', { value: 1500, configurable: true })
    const scrollBy = vi.fn()
    list.scrollBy = scrollBy as never
    fireEvent.scroll(list)
    expect(forward).toBeVisible()
    expect(back).not.toBeVisible()
    fireEvent.click(forward)
    expect(scrollBy).toHaveBeenCalledWith(expect.objectContaining({ behavior: 'smooth' }))
    expect(scrollBy.mock.calls[0]![0].left).toBeGreaterThan(0)

    list.scrollLeft = 1000
    fireEvent.scroll(list)
    expect(back).toBeVisible()
    expect(forward).not.toBeVisible()
  })
})

describe('Grid', () => {
  it('wraps each child in a list item with the preset minimum width', () => {
    const { container } = renderWithRouter(
      <Grid kind="square" aria-label="Albums">
        <SquareCard title="A" />
        <SquareCard title="B" />
      </Grid>,
    )
    expect(screen.getByRole('list', { name: 'Albums' })).toBeInTheDocument()
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
    expect((container.firstChild as HTMLElement).style.getPropertyValue('--jr-grid-min')).toBe(
      '160px',
    )
  })
})

describe('HeroBanner', () => {
  it('renders in the dark theme with the logo, and falls back to the text title', () => {
    renderWithRouter(
      <HeroBanner
        title="Severance"
        eyebrow="Up Next"
        logo="https://x/logo.png"
        backdrop="https://x/bd.jpg"
        meta="2025 · Drama"
        actions={<Button variant="contrast">Play</Button>}
        progress={0.3}
        progressCaption="31 min left"
      />,
    )
    const region = screen.getByRole('region', { name: 'Severance' })
    expect(region).toHaveAttribute('data-theme', 'dark')
    const logo = screen.getByRole('img', { name: 'Severance' })
    expect(screen.getByRole('heading', { level: 1 })).toContainElement(logo)
    fireEvent.error(logo)
    expect(screen.getByRole('heading', { level: 1, name: 'Severance' })).toHaveTextContent(
      'Severance',
    )
    expect(screen.getByRole('button', { name: 'Play' })).toBeInTheDocument()
    expect(screen.getByText('31 min left')).toBeInTheDocument()
  })
})
