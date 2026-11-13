import clsx from 'clsx'
import { AnimatePresence, motion } from 'framer-motion'
import { CircleCheck, Eye, EyeOff } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, IconButton, TextField, durations, easings, springs } from '@renderer/design'
import { connectServer, probeServer, useSession, type ServerInfo } from '@renderer/jellyfin'
import type { ServerAccount } from '@shared/jellyfin'
import { errorMessage } from './errors'
import './connect.css'

type Step = 'address' | 'login' | 'success' | 'name'
const STEPS: readonly Step[] = ['address', 'login', 'success', 'name']
/** The success beat is transient: it is not one of the dots. */
const DOT_STEPS: readonly Step[] = ['address', 'login', 'name']
const SUCCESS_MS = 1300

export interface ConnectFlowProps {
  /** `page`: large centred onboarding. `sheet`: inside a Sheet (title shown by the Sheet). */
  variant?: 'page' | 'sheet'
  /** Called once the new server is saved, named and active. */
  onDone(account: ServerAccount): void
}

/**
 * Three-step "connect a server" flow: address (probe) → credentials → name. The password lives in
 * component state only until the login call returns, then it is wiped.
 */
export function ConnectFlow({ variant = 'page', onDone }: ConnectFlowProps) {
  const { t } = useTranslation('servers')
  const [step, setStep] = useState<Step>('address')
  const direction = useRef(1)

  const [address, setAddress] = useState('')
  const [probing, setProbing] = useState(false)
  const [probeError, setProbeError] = useState<string | null>(null)
  const [found, setFound] = useState<{ typed: string; info: ServerInfo } | null>(null)
  const probeRun = useRef(0)

  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [loggingIn, setLoggingIn] = useState(false)
  const [loginError, setLoginError] = useState<string | null>(null)

  const [account, setAccount] = useState<ServerAccount | null>(null)
  const [name, setName] = useState('')
  const [finishing, setFinishing] = useState(false)
  const [finishError, setFinishError] = useState<string | null>(null)

  const go = (next: Step) => {
    direction.current = STEPS.indexOf(next) >= STEPS.indexOf(step) ? 1 : -1
    setStep(next)
  }

  useEffect(() => {
    if (step !== 'success') return
    const timer = window.setTimeout(() => {
      direction.current = 1
      setStep('name')
    }, SUCCESS_MS)
    return () => window.clearTimeout(timer)
  }, [step])

  const foundNow = found && found.typed === address.trim() ? found : null

  const submitAddress = async (e: FormEvent) => {
    e.preventDefault()
    const typed = address.trim()
    if (foundNow) return go('login')
    if (!typed || probing) return
    const run = ++probeRun.current
    setProbing(true)
    setProbeError(null)
    setFound(null)
    try {
      const res = await probeServer(typed)
      if (run !== probeRun.current) return
      if (res.ok) setFound({ typed, info: res.info })
      else setProbeError(errorMessage(t, res.error))
    } catch {
      if (run === probeRun.current) setProbeError(t('errors.generic'))
    } finally {
      if (run === probeRun.current) setProbing(false)
    }
  }

  const submitLogin = async (e: FormEvent) => {
    e.preventDefault()
    if (!found || !username.trim() || !password || loggingIn) return
    setLoggingIn(true)
    setLoginError(null)
    try {
      const res = await connectServer({
        url: found.info.url,
        username: username.trim(),
        password,
      })
      if (res.ok) {
        const session = useSession.getState()
        if (session.account?.id === res.account.id) {
          // Signed in again to the active server: Jellyfin has just revoked this device's
          // previous token, so switch to the new one now, not only when the flow is finished
          // (closing the sheet at the naming step would leave the app on a dead token).
          await session.activate(res.account)
        } else {
          void session.refreshAccounts()
        }
        setAccount(res.account)
        setName(res.account.name)
        go('success')
      } else {
        setLoginError(errorMessage(t, res.error))
      }
    } catch {
      setLoginError(t('errors.generic'))
    } finally {
      // The password never outlives the login call.
      setPassword('')
      setShowPassword(false)
      setLoggingIn(false)
    }
  }

  const submitName = async (e: FormEvent) => {
    e.preventDefault()
    if (!account || finishing) return
    setFinishing(true)
    setFinishError(null)
    try {
      let saved = account
      const trimmed = name.trim()
      if (trimmed && trimmed !== account.name) {
        saved = (await window.lumen.accounts.rename(account.id, trimmed)) ?? account
      }
      await useSession.getState().activate(saved)
      onDone(saved)
    } catch {
      setFinishError(t('errors.generic'))
      setFinishing(false)
    }
  }

  const dotStep: Step = step === 'success' ? 'login' : step
  const x = 28 * direction.current
  return (
    <div className={clsx('jr-connect', `jr-connect--${variant}`)}>
      <div className="jr-connect__stage">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={step}
            className="jr-connect__step"
            initial={{ opacity: 0, x }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -x }}
            transition={{ duration: durations.base, ease: easings.out }}
          >
            {step === 'address' && (
              <form className="jr-connect__step" onSubmit={(e) => void submitAddress(e)} noValidate>
                <h1 className="jr-connect__title">{t('address.title')}</h1>
                <p className="jr-connect__subtitle">{t('address.subtitle')}</p>
                <div className="jr-connect__fields">
                  <TextField
                    label={t('address.label')}
                    placeholder={t('address.placeholder')}
                    hint={t('address.hint')}
                    error={probeError}
                    value={address}
                    onChange={(e) => {
                      setAddress(e.target.value)
                      setProbeError(null)
                    }}
                    autoFocus
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                    inputMode="url"
                    size="large"
                  />
                </div>
                {foundNow && (
                  <div className="jr-connect__found" role="status">
                    <CircleCheck className="jr-connect__found-icon" aria-hidden="true" />
                    <span>
                      <span className="jr-connect__found-title">{t('address.found')}</span>
                      <span className="jr-connect__found-detail">
                        {t('address.foundDetail', {
                          name: foundNow.info.serverName,
                          version: foundNow.info.version,
                        })}
                      </span>
                    </span>
                  </div>
                )}
                <div className="jr-connect__actions">
                  <Button
                    type="submit"
                    size="large"
                    loading={probing}
                    disabled={!address.trim()}
                    fullWidth
                  >
                    {probing
                      ? t('address.checking')
                      : foundNow
                        ? t('address.continue')
                        : t('address.connect')}
                  </Button>
                </div>
              </form>
            )}

            {step === 'login' && found && (
              <form className="jr-connect__step" onSubmit={(e) => void submitLogin(e)}>
                <h1 className="jr-connect__title">{t('login.title')}</h1>
                <p className="jr-connect__subtitle">
                  {t('login.subtitle', { name: found.info.serverName })}
                </p>
                <div className="jr-connect__fields">
                  <TextField
                    label={t('login.username')}
                    value={username}
                    onChange={(e) => {
                      setUsername(e.target.value)
                      setLoginError(null)
                    }}
                    autoComplete="username"
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                    autoFocus
                    size="large"
                  />
                  <TextField
                    label={t('login.password')}
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    onChange={(e) => {
                      setPassword(e.target.value)
                      setLoginError(null)
                    }}
                    autoComplete="current-password"
                    error={loginError}
                    size="large"
                    trailing={
                      <IconButton
                        size="small"
                        icon={showPassword ? <EyeOff /> : <Eye />}
                        label={showPassword ? t('login.hidePassword') : t('login.showPassword')}
                        pressed={showPassword}
                        onClick={() => setShowPassword((s) => !s)}
                      />
                    }
                  />
                </div>
                <div className="jr-connect__actions">
                  <Button
                    type="submit"
                    size="large"
                    loading={loggingIn}
                    disabled={!username.trim() || !password}
                    fullWidth
                  >
                    {t('login.signIn')}
                  </Button>
                  <Button
                    type="button"
                    variant="plain"
                    onClick={() => go('address')}
                    disabled={loggingIn}
                    fullWidth
                  >
                    {t('login.back')}
                  </Button>
                </div>
              </form>
            )}

            {step === 'success' && (
              <div className="jr-connect__step jr-connect__success" role="status">
                <motion.svg
                  className="jr-connect__check"
                  viewBox="0 0 52 52"
                  aria-hidden="true"
                  initial={{ scale: 0.7, opacity: 0 }}
                  animate={{ scale: 1, opacity: 1 }}
                  transition={springs.bouncy}
                >
                  <circle cx="26" cy="26" r="24" className="jr-connect__check-bg" />
                  <motion.path
                    d="M15 27 L23 35 L38 19"
                    className="jr-connect__check-mark"
                    initial={{ pathLength: 0 }}
                    animate={{ pathLength: 1 }}
                    transition={{ duration: durations.slower, ease: easings.out, delay: 0.15 }}
                  />
                </motion.svg>
                <h1 className="jr-connect__title">{t('success.title')}</h1>
                <p className="jr-connect__subtitle">{t('success.subtitle')}</p>
              </div>
            )}

            {step === 'name' && (
              <form className="jr-connect__step" onSubmit={(e) => void submitName(e)}>
                <h1 className="jr-connect__title">{t('name.title')}</h1>
                <p className="jr-connect__subtitle">{t('name.subtitle')}</p>
                <div className="jr-connect__fields">
                  <TextField
                    label={t('name.label')}
                    hint={t('name.hint')}
                    error={finishError}
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    autoFocus
                    onFocus={(e) => e.currentTarget.select()}
                    size="large"
                  />
                </div>
                <div className="jr-connect__actions">
                  <Button type="submit" size="large" loading={finishing} fullWidth>
                    {t('name.finish')}
                  </Button>
                </div>
              </form>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
      <div
        className="jr-connect__dots"
        role="img"
        aria-label={t('steps.label', {
          current: DOT_STEPS.indexOf(dotStep) + 1,
          total: DOT_STEPS.length,
        })}
      >
        {DOT_STEPS.map((s) => (
          <span
            key={s}
            className={clsx('jr-connect__dot', s === dotStep && 'jr-connect__dot--active')}
          />
        ))}
      </div>
    </div>
  )
}
