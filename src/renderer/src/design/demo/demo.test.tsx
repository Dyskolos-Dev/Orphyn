import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderWithRouter } from '../test-utils'
import { themeStore } from '../theme/theme'
import { DesignGallery } from './DesignGallery'

describe('DesignGallery', () => {
  afterEach(() => {
    localStorage.clear()
    themeStore.reload()
  })

  it('renders every component without errors', () => {
    renderWithRouter(<DesignGallery />)
    for (const name of ['Theme', 'Text', 'Button', 'IconButton', 'Toolbar', 'Grid', 'Feedback']) {
      expect(screen.getByRole('region', { name })).toBeInTheDocument()
    }
    for (const shelf of ['Posters', 'Continue Watching', 'Albums', 'Artists']) {
      expect(screen.getByRole('heading', { level: 2, name: shelf })).toBeInTheDocument()
    }
    expect(screen.getByRole('region', { name: 'Placeholder Title' })).toHaveAttribute(
      'data-theme',
      'dark',
    )
    expect(screen.getByRole('progressbar', { name: 'Downloading' })).toHaveAttribute(
      'aria-valuenow',
      '40',
    )
    expect(screen.getByRole('heading', { name: 'No Music Yet' })).toBeInTheDocument()
  })

  it('switches the theme from the appearance control', async () => {
    const user = userEvent.setup()
    renderWithRouter(<DesignGallery />)
    const appearance = screen.getByRole('radiogroup', { name: 'Appearance' })
    await user.click(within(appearance).getByRole('radio', { name: 'Dark' }))
    expect(document.documentElement.dataset['theme']).toBe('dark')
    await user.click(
      within(screen.getByRole('radiogroup', { name: 'Accent' })).getByRole('radio', {
        name: 'blue',
      }),
    )
    expect(document.documentElement.dataset['accent']).toBe('blue')
  })
})
