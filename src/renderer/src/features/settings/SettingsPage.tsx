import {
  Info,
  MessageCircle,
  Music,
  Palette,
  Play,
  Server,
  Settings as SettingsIcon,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchParams } from 'react-router'
import { SidebarItem } from '@renderer/design'
import { ShellPage } from '@renderer/shell'
import { AboutPane } from './AboutPane'
import { AppearancePane } from './AppearancePane'
import { DiscordPane } from './DiscordPane'
import { GeneralPane } from './GeneralPane'
import { MusicPane } from './MusicPane'
import { PlaybackPane } from './PlaybackPane'
import { ServersPane } from './ServersPane'
import './settings.css'

const PANES = [
  { id: 'general', icon: <SettingsIcon />, view: <GeneralPane /> },
  { id: 'appearance', icon: <Palette />, view: <AppearancePane /> },
  { id: 'servers', icon: <Server />, view: <ServersPane /> },
  { id: 'playback', icon: <Play />, view: <PlaybackPane /> },
  { id: 'music', icon: <Music />, view: <MusicPane /> },
  { id: 'discord', icon: <MessageCircle />, view: <DiscordPane /> },
  { id: 'about', icon: <Info />, view: <AboutPane /> },
] as const satisfies readonly { id: string; icon: ReactNode; view: ReactNode }[]

/** Settings: a pane list on the left, like macOS System Settings; the pane is `?pane=`. */
export function SettingsPage() {
  const { t } = useTranslation('settings')
  const [params] = useSearchParams()
  const requested = params.get('pane')
  const pane = PANES.find((p) => p.id === requested) ?? PANES[0]

  return (
    <ShellPage title={t('title')} gutter>
      <div className="jr-settings">
        <nav className="jr-settings__nav" aria-label={t('nav')}>
          <ul className="jr-sidebar-section__list">
            {PANES.map((p) => (
              <SidebarItem
                key={p.id}
                icon={p.icon}
                label={t(`panes.${p.id}`)}
                to={`/settings?pane=${p.id}`}
                active={p.id === pane.id}
              />
            ))}
          </ul>
        </nav>
        <section className="jr-settings__pane" aria-label={t(`panes.${pane.id}`)}>
          <h2 className="jr-settings__pane-title">{t(`panes.${pane.id}`)}</h2>
          {pane.view}
        </section>
      </div>
    </ShellPage>
  )
}
