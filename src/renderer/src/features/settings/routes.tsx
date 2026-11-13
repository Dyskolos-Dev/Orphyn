import type { RouteObject } from 'react-router'
import { StubPage } from '@renderer/shell/StubPage'

/** Route table for the settings feature. Owned by the settings agent. */
export const settingsRoutes: RouteObject[] = [
  { path: '/settings', element: <StubPage name="Settings" /> },
]
