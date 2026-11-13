import type { RouteObject } from 'react-router'
import { StubPage } from '@renderer/shell/StubPage'

/** Route table for the music feature. Owned by the music agent. */
export const musicRoutes: RouteObject[] = [
  { path: '/music', element: <StubPage name="Music home" /> },
  { path: '/music/albums', element: <StubPage name="Albums" /> },
  { path: '/music/artists', element: <StubPage name="Artists" /> },
  { path: '/music/playlists', element: <StubPage name="Playlists" /> },
  { path: '/music/songs', element: <StubPage name="Songs" /> },
  { path: '/music/album/:id', element: <StubPage name="Album" /> },
  { path: '/music/artist/:id', element: <StubPage name="Artist" /> },
  { path: '/music/playlist/:id', element: <StubPage name="Playlist" /> },
]
