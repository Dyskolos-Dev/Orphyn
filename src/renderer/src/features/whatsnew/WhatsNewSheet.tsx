import {
  Cast,
  Download,
  Film,
  LayoutPanelTop,
  Music,
  RefreshCw,
  Sparkles,
  Users,
  type LucideIcon,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { BrandLogo, Button, Sheet, Text } from '@renderer/design'
import type { VersionNotes } from './notes'

const NOTE_ICONS: Record<string, LucideIcon> = {
  layout: LayoutPanelTop,
  music: Music,
  download: Download,
  sparkles: Sparkles,
  film: Film,
  users: Users,
  cast: Cast,
  refresh: RefreshCw,
}

/** "What's New" sheet: logo, title, one icon row per item, Continue. */
export function WhatsNewSheet({
  open,
  notes,
  onClose,
}: {
  open: boolean
  /** Newest first; older versions get their own heading under the first one. */
  notes: readonly VersionNotes[]
  onClose(): void
}) {
  const { t, i18n } = useTranslation('common')
  const lang = i18n.resolvedLanguage?.startsWith('fr') ? 'fr' : 'en'
  const newest = notes[0]
  return (
    <Sheet
      open={open && Boolean(newest)}
      onClose={onClose}
      size="medium"
      hideCloseButton
      className="jr-whatsnew"
      title={t('whatsNew.title', { version: newest?.version ?? '' })}
      footer={<Button onClick={onClose}>{t('whatsNew.continue')}</Button>}
    >
      <div className="jr-whatsnew__body">
        <BrandLogo size={64} className="jr-whatsnew__logo" />
        {notes.map((n, i) => {
          const text = n.note[lang] ?? n.note.en
          return (
            <section key={n.version} className="jr-whatsnew__version">
              {i > 0 && (
                <Text variant="headline" className="jr-whatsnew__heading">
                  {t('whatsNew.versionHeading', { version: n.version, title: text.title })}
                </Text>
              )}
              <ul className="jr-whatsnew__items">
                {text.items.map((item) => {
                  const Icon = NOTE_ICONS[item.icon] ?? Sparkles
                  return (
                    <li key={item.title} className="jr-whatsnew__item">
                      <Icon className="jr-whatsnew__icon" aria-hidden="true" />
                      <div>
                        <Text variant="headline">{item.title}</Text>
                        <Text variant="subheadline" color="secondary">
                          {item.text}
                        </Text>
                      </div>
                    </li>
                  )
                })}
              </ul>
            </section>
          )
        })}
      </div>
    </Sheet>
  )
}
