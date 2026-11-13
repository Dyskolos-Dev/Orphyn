import type { RouteObject } from 'react-router'
import './language'
import { SettingsPage } from './SettingsPage'

/** Route table for the settings feature. Owned by the settings agent. */
export const settingsRoutes: RouteObject[] = [{ path: '/settings', element: <SettingsPage /> }]
