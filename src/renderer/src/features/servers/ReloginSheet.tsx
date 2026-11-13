import { Eye, EyeOff } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, IconButton, Sheet, TextField } from '@renderer/design'
import { connectServer, useSession } from '@renderer/jellyfin'
import type { ServerAccount } from '@shared/jellyfin'
import { errorMessage } from './errors'

/** Sign in again to a saved server whose token expired (same user, same name). */
export function ReloginSheet({
  account,
  open,
  onClose,
}: {
  account: ServerAccount
  open: boolean
  onClose(): void
}) {
  const { t } = useTranslation('servers')
  const [password, setPassword] = useState('')
  const [show, setShow] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const close = () => {
    setPassword('')
    setError(null)
    onClose()
  }

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    if (!password || busy) return
    setBusy(true)
    setError(null)
    try {
      const res = await connectServer({
        url: account.url,
        username: account.userName,
        password,
        name: account.name,
      })
      if (res.ok) {
        const session = useSession.getState()
        if (!session.account || session.account.id === account.id) {
          await session.activate(res.account)
        } else {
          await session.refreshAccounts()
        }
        close()
      } else {
        setError(errorMessage(t, res.error))
      }
    } catch {
      setError(t('errors.generic'))
    } finally {
      // The password never outlives the login call.
      setPassword('')
      setShow(false)
      setBusy(false)
    }
  }

  return (
    <Sheet
      open={open}
      onClose={close}
      size="small"
      title={t('login.reloginTitle')}
      description={t('login.reloginSubtitle', { name: account.name, user: account.userName })}
    >
      <form onSubmit={(e) => void submit(e)} className="jr-connect__fields">
        <TextField
          label={t('login.password')}
          type={show ? 'text' : 'password'}
          value={password}
          onChange={(e) => {
            setPassword(e.target.value)
            setError(null)
          }}
          autoComplete="current-password"
          error={error}
          autoFocus
          trailing={
            <IconButton
              size="small"
              icon={show ? <EyeOff /> : <Eye />}
              label={show ? t('login.hidePassword') : t('login.showPassword')}
              pressed={show}
              onClick={() => setShow((s) => !s)}
            />
          }
        />
        <Button type="submit" loading={busy} disabled={!password} fullWidth>
          {t('login.signIn')}
        </Button>
      </form>
    </Sheet>
  )
}
