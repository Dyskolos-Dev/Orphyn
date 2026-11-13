import type { RouteObject } from 'react-router'
import { SearchPage } from './SearchPage'

/** Route table for the search feature. Owned by the search agent. */
export const searchRoutes: RouteObject[] = [{ path: '/search', element: <SearchPage /> }]
