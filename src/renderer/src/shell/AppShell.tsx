import { useTranslation } from 'react-i18next'
import { NavLink, Outlet } from 'react-router'

const links = [
  { to: '/video', key: 'nav.video' },
  { to: '/music', key: 'nav.music' },
  { to: '/search', key: 'nav.search' },
  { to: '/settings', key: 'nav.settings' },
  { to: '/onboarding', key: 'nav.servers' },
  { to: '/cast', key: 'nav.cast' },
] as const

/** Application layout: sidebar + routed content. Owner: shell agent. */
export function AppShell() {
  const { t } = useTranslation()
  return (
    <div style={{ display: 'flex', height: '100%' }}>
      <nav aria-label="Main" style={{ width: 200, padding: 16, borderRight: '1px solid #8884' }}>
        <strong>{t('app.name')}</strong>
        <ul style={{ listStyle: 'none', padding: 0 }}>
          {links.map((l) => (
            <li key={l.to}>
              <NavLink to={l.to}>{t(l.key)}</NavLink>
            </li>
          ))}
        </ul>
      </nav>
      <main style={{ flex: 1, padding: 16, overflow: 'auto' }}>
        <Outlet />
      </main>
    </div>
  )
}
