import { Check, ChevronsUpDown, Plus, Settings2 } from 'lucide-react'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { Popover, toast } from '@renderer/design'
import { useSession } from '@renderer/jellyfin'
import { AddServerSheet } from './AddServerSheet'
import { useActiveAccount } from './useActiveAccount'
import { UserAvatar } from './Avatar'
import './connect.css'

/** Sidebar footer: the active server (name + user avatar) and a popover to switch / add / manage. */
export function ServerSwitcher() {
  const { t } = useTranslation('servers')
  const navigate = useNavigate()
  const accounts = useSession((s) => s.accounts)
  const active = useActiveAccount()
  const needsLogin = useSession((s) => s.needsLogin)
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const [adding, setAdding] = useState(false)

  if (!active && accounts.length === 0) return null

  const switchTo = async (id: string) => {
    setOpen(false)
    if (id === active?.id) return
    try {
      await useSession.getState().switchServer(id)
    } catch {
      toast({ title: t('errors.unreachable'), tone: 'error' })
    }
  }

  return (
    <div className="jr-switcher">
      <button
        ref={anchor}
        type="button"
        className="jr-switcher__button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={active ? t('switcher.active', { name: active.name }) : t('switcher.label')}
        onClick={() => setOpen((o) => !o)}
      >
        {active && <UserAvatar account={active} />}
        <span className="jr-switcher__text">
          <span className="jr-switcher__name">{active?.name ?? t('switcher.none')}</span>
          {active && <span className="jr-switcher__user">{active.userName}</span>}
        </span>
        <ChevronsUpDown className="jr-switcher__chevron" aria-hidden="true" />
      </button>
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        placement="top-start"
        role="menu"
        aria-label={t('switcher.menu')}
      >
        <div className="jr-switcher__menu">
          {accounts.map((a) => (
            <button
              key={a.id}
              type="button"
              role="menuitemradio"
              aria-checked={a.id === active?.id}
              className="jr-switcher__item"
              onClick={() => void switchTo(a.id)}
            >
              {a.id === active?.id ? (
                <Check className="jr-switcher__check" aria-hidden="true" />
              ) : (
                <span className="jr-switcher__spacer" />
              )}
              <span className="jr-switcher__name">{a.name}</span>
              {needsLogin[a.id] && (
                <span className="jr-switcher__flag">{t('switcher.needsLogin')}</span>
              )}
            </button>
          ))}
          <div className="jr-switcher__sep" role="separator" />
          <button
            type="button"
            role="menuitem"
            className="jr-switcher__item"
            onClick={() => {
              setOpen(false)
              setAdding(true)
            }}
          >
            <Plus className="jr-switcher__check" aria-hidden="true" />
            {t('switcher.add')}
          </button>
          <button
            type="button"
            role="menuitem"
            className="jr-switcher__item"
            onClick={() => {
              setOpen(false)
              void navigate('/settings?pane=servers')
            }}
          >
            <Settings2 className="jr-switcher__check" aria-hidden="true" />
            {t('switcher.manage')}
          </button>
        </div>
      </Popover>
      {adding && <AddServerSheet open onClose={() => setAdding(false)} />}
    </div>
  )
}
