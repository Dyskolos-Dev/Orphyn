import { BarChart3 } from 'lucide-react'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import type { RouteObject } from 'react-router'
import { registerShellExtension } from '@renderer/shell'
import { StatsPage } from './StatsPage'

/** Sidebar link, last of the "more" group: the least intrusive spot, no shell file touched. */
function useStatsSidebarEntry() {
  const { t } = useTranslation('stats')
  const label = t('nav')
  useEffect(
    () =>
      registerShellExtension({
        id: 'stats',
        sidebarItems: [
          { id: 'stats', section: 'more', label, icon: <BarChart3 />, to: '/stats', order: 200 },
        ],
      }),
    [label],
  )
}

function StatsHost() {
  useStatsSidebarEntry()
  return null
}

registerShellExtension({ id: 'stats-host', background: StatsHost })

export const statsRoutes: RouteObject[] = [{ path: '/stats', element: <StatsPage /> }]
