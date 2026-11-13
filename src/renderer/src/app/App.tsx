import { QueryClientProvider } from '@tanstack/react-query'
import { createHashRouter, RouterProvider } from 'react-router'
import '@renderer/i18n'
import { queryClient } from '@renderer/jellyfin'
import { routes } from './routes'

const router = createHashRouter(routes)

/**
 * One QueryClient for the whole app: the session store's. Switching or removing a server clears
 * that client, so the UI must read from the same one (and get its 4xx no-retry policy).
 */
export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}
