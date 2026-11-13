import type { RouteObject } from 'react-router'
import { StubPage } from '@renderer/shell/StubPage'

/** Route table for the cast feature. Owned by the cast agent. */
export const castRoutes: RouteObject[] = [{ path: '/cast', element: <StubPage name="Cast" /> }]
