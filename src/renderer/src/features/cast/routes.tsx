import type { RouteObject } from 'react-router'
import { StubPage } from '@renderer/shell/StubPage'
import './register' // installs the picker in the music and video players

/** Route table for the cast feature. */
export const castRoutes: RouteObject[] = [{ path: '/cast', element: <StubPage name="Cast" /> }]
