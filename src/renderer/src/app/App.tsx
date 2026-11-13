import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { createHashRouter, RouterProvider } from 'react-router'
import '@renderer/i18n'
import { routes } from './routes'

const router = createHashRouter(routes)

export function App() {
  const [queryClient] = useState(() => new QueryClient())
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}
