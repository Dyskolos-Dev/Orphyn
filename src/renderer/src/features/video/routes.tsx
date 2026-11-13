import type { RouteObject } from 'react-router'
import './video.css'
import { VideoHome } from './VideoHome'
import { VideoItem } from './VideoItem'
import { VideoLibrary } from './VideoLibrary'
import { VideoPlayer } from './player/VideoPlayer'

/** Route table for the video feature. Owned by the video agent. */
export const videoRoutes: RouteObject[] = [
  { path: '/video', element: <VideoHome /> },
  { path: '/video/library/:libraryId', element: <VideoLibrary /> },
  { path: '/video/item/:itemId', element: <VideoItem /> },
  { path: '/video/play/:itemId', element: <VideoPlayer /> },
]
