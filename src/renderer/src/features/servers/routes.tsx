import type { RouteObject } from 'react-router'
import { StubPage } from '@renderer/shell/StubPage'

/** Route table for the servers feature. Owned by the servers agent. */
export const serversRoutes: RouteObject[] = [
  { path: '/onboarding', element: <StubPage name="Onboarding" /> },
]
