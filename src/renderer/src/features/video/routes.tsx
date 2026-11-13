import type { RouteObject } from 'react-router'
import { StubPage } from '@renderer/shell/StubPage'

/** Route table for the video feature. Owned by the video agent. */
export const videoRoutes: RouteObject[] = [
  { path: '/video', element: <StubPage name="Video home" /> },
  { path: '/video/library/:libraryId', element: <StubPage name="Video library" /> },
  { path: '/video/item/:itemId', element: <StubPage name="Video item" /> },
  { path: '/video/play/:itemId', element: <StubPage name="Video player" /> },
]
