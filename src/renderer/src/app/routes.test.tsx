import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router'
import '@renderer/i18n'
import { routes } from './routes'

describe('app routes', () => {
  it('renders the shell sidebar around a feature route', () => {
    const router = createMemoryRouter(routes, { initialEntries: ['/cast'] })
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    )
    expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument()
  })
})
