import type { RouteObject } from 'react-router'
import { StubPage } from '@renderer/shell/StubPage'

/** Route table for the search feature. Owned by the search agent. */
export const searchRoutes: RouteObject[] = [
  { path: '/search', element: <StubPage name="Search" /> },
]
