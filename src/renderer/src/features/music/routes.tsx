import type { RouteObject } from 'react-router'
import './music.css'
import './register'
import './playlists/register'
import { FavoritesPage } from './favorites/FavoritesPage'
import { AlbumPage } from './pages/AlbumPage'
import { AlbumsPage } from './pages/AlbumsPage'
import { ArtistPage } from './pages/ArtistPage'
import { ArtistsPage } from './pages/ArtistsPage'
import { HomePage } from './pages/HomePage'
import { PlaylistPage } from './pages/PlaylistPage'
import { PlaylistsPage } from './pages/PlaylistsPage'
import { SongsPage } from './pages/SongsPage'

/** Route table for the music feature. Owned by the music agent. */
export const musicRoutes: RouteObject[] = [
  { path: '/music', element: <HomePage /> },
  { path: '/music/albums', element: <AlbumsPage /> },
  { path: '/music/artists', element: <ArtistsPage /> },
  { path: '/music/playlists', element: <PlaylistsPage /> },
  { path: '/music/favorites', element: <FavoritesPage /> },
  { path: '/music/songs', element: <SongsPage /> },
  { path: '/music/album/:id', element: <AlbumPage /> },
  { path: '/music/artist/:id', element: <ArtistPage /> },
  { path: '/music/playlist/:id', element: <PlaylistPage /> },
]
