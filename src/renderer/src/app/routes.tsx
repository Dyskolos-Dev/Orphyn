import { Navigate, type RouteObject } from 'react-router'
import { castRoutes } from '@renderer/features/cast/routes'
import '@renderer/features/discord'
import { musicRoutes } from '@renderer/features/music/routes'
// Command palette (Ctrl/Cmd+K): plugs into the shell, imported for its side effect.
import '@renderer/features/palette/register'
import { searchRoutes } from '@renderer/features/search/routes'
import { serversRoutes } from '@renderer/features/servers/routes'
import { settingsRoutes } from '@renderer/features/settings/routes'
import { statsRoutes } from '@renderer/features/stats/routes'
import { videoRoutes } from '@renderer/features/video/routes'
// After the servers feature: its guard must wrap the What's New provider.
import '@renderer/features/whatsnew/register'
import { AppShell } from '@renderer/shell/AppShell'

/** Compose every feature's routes under the shared shell layout. */
export const routes: RouteObject[] = [
  {
    element: <AppShell />,
    children: [
      { index: true, element: <Navigate to="/video" replace /> },
      ...serversRoutes,
      ...settingsRoutes,
      ...searchRoutes,
      ...videoRoutes,
      ...musicRoutes,
      ...castRoutes,
      ...statsRoutes,
    ],
  },
]
