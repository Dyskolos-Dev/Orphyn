import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router'
import '@renderer/i18n'
import { routes } from './routes'

describe('app routes', () => {
  it('renders the shell sidebar and a stub route', () => {
    const router = createMemoryRouter(routes, { initialEntries: ['/music/album/42'] })
    render(<RouterProvider router={router} />)
    expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Album' })).toBeInTheDocument()
  })
})
