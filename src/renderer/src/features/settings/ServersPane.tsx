import { Plus } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import {
  Button,
  EmptyState,
  List,
  ListRow,
  MenuButton,
  Sheet,
  TextField,
  toast,
} from '@renderer/design'
import { useSession } from '@renderer/jellyfin'
import { AddServerSheet, ReloginSheet, UserAvatar } from '@renderer/features/servers'
import type { ServerAccount } from '@shared/jellyfin'
import { useAccountsStatus } from './useAccountsStatus'

export function ServersPane() {
  const { t } = useTranslation('settings')
  const accounts = useSession((s) => s.accounts)
  const activeId = useSession((s) => s.account?.id)
  const needsLogin = useSession((s) => s.needsLogin)
  const status = useAccountsStatus()
  const [adding, setAdding] = useState(false)
  const [renaming, setRenaming] = useState<ServerAccount | null>(null)
  const [removing, setRemoving] = useState<ServerAccount | null>(null)
  const [relogin, setRelogin] = useState<ServerAccount | null>(null)

  return (
    <>
      {accounts.length === 0 ? (
        <EmptyState
          size="inline"
          title={t('servers.empty')}
          message={t('servers.emptyMessage')}
          action={
            <Button icon={<Plus />} onClick={() => setAdding(true)}>
              {t('servers.add')}
            </Button>
          }
        />
      ) : (
        <List
          variant="inset"
          header={t('servers.header')}
          footer={status && !status.encrypted ? t('servers.storageWarning') : undefined}
          aria-label={t('servers.header')}
        >
          {accounts.map((a) => {
            const expired = Boolean(needsLogin[a.id]) || !a.hasToken
            return (
              <ListRow
                key={a.id}
                leading={<UserAvatar account={a} size={32} />}
                title={
                  <>
                    {a.name}
                    {a.id === activeId && (
                      <span className="jr-settings__badge">{t('servers.active')}</span>
                    )}
                  </>
                }
                subtitle={
                  expired
                    ? t('servers.needsLogin')
                    : t('servers.details', { user: a.userName, url: a.url })
                }
                trailing={
                  <span className="jr-settings__actions">
                    {expired && (
                      <Button variant="tinted" size="small" onClick={() => setRelogin(a)}>
                        {t('servers.signInAgain')}
                      </Button>
                    )}
                    <MenuButton
                      label={t('servers.more', { name: a.name })}
                      variant="plain"
                      items={[
                        {
                          label: t('servers.switch'),
                          disabled: a.id === activeId || expired,
                          onSelect: () => {
                            void useSession.getState().switchServer(a.id)
                          },
                        },
                        { label: t('servers.rename'), onSelect: () => setRenaming(a) },
                        { type: 'separator' },
                        {
                          label: t('servers.signOut'),
                          destructive: true,
                          onSelect: () => setRemoving(a),
                        },
                      ]}
                    />
                  </span>
                }
              />
            )
          })}
        </List>
      )}
      {accounts.length > 0 && (
        <div>
          <Button variant="gray" size="small" icon={<Plus />} onClick={() => setAdding(true)}>
            {t('servers.add')}
          </Button>
        </div>
      )}

      {adding && <AddServerSheet open onClose={() => setAdding(false)} />}
      {relogin && <ReloginSheet open account={relogin} onClose={() => setRelogin(null)} />}
      <RenameSheet account={renaming} onClose={() => setRenaming(null)} />
      <RemoveSheet account={removing} onClose={() => setRemoving(null)} />
    </>
  )
}

function RenameSheet({ account, onClose }: { account: ServerAccount | null; onClose(): void }) {
  const { t } = useTranslation('settings')
  const [draft, setDraft] = useState<{ id: string; name: string } | null>(null)
  const name = draft && account && draft.id === account.id ? draft.name : (account?.name ?? '')
  const [busy, setBusy] = useState(false)

  const save = async (e: FormEvent) => {
    e.preventDefault()
    const trimmed = name.trim()
    if (!account || !trimmed || busy) return
    setBusy(true)
    try {
      if (trimmed !== account.name) {
        await window.jellyra.accounts.rename(account.id, trimmed)
        await useSession.getState().refreshAccounts()
      }
      onClose()
    } catch {
      toast({ title: t('servers.removeFailed'), tone: 'error' })
    } finally {
      setBusy(false)
      setDraft(null)
    }
  }

  return (
    <Sheet
      open={account !== null}
      onClose={() => {
        setDraft(null)
        onClose()
      }}
      size="small"
      title={t('servers.renameTitle')}
    >
      <form onSubmit={(e) => void save(e)} className="jr-settings__form">
        <TextField
          label={t('servers.renameLabel')}
          value={name}
          onChange={(e) => account && setDraft({ id: account.id, name: e.target.value })}
          autoFocus
          onFocus={(e) => e.currentTarget.select()}
        />
        <div className="jr-settings__sheet-actions">
          <Button type="button" variant="gray" onClick={onClose}>
            {t('servers.cancel')}
          </Button>
          <Button type="submit" loading={busy} disabled={!name.trim()}>
            {t('servers.save')}
          </Button>
        </div>
      </form>
    </Sheet>
  )
}

function RemoveSheet({ account, onClose }: { account: ServerAccount | null; onClose(): void }) {
  const { t } = useTranslation('settings')
  const [busy, setBusy] = useState(false)
  // Keep the last account while the sheet animates out.
  const [last, setLast] = useState<ServerAccount | null>(null)
  if (account && account !== last) setLast(account)
  const shown = account ?? last

  const confirm = async () => {
    if (!account || busy) return
    setBusy(true)
    try {
      await useSession.getState().signOut(account.id)
      onClose()
    } catch {
      toast({ title: t('servers.removeFailed'), tone: 'error' })
    } finally {
      setBusy(false)
    }
  }

  return (
    <Sheet
      open={account !== null}
      onClose={onClose}
      size="small"
      title={t('servers.removeTitle', { name: shown?.name ?? '' })}
      description={t('servers.removeMessage')}
      footer={
        <>
          <Button variant="gray" onClick={onClose}>
            {t('servers.cancel')}
          </Button>
          <Button destructive loading={busy} onClick={() => void confirm()}>
            {t('servers.removeConfirm')}
          </Button>
        </>
      }
    />
  )
}
