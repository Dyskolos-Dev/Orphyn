import { RefreshCw, Users } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { useTranslation } from 'react-i18next'
import i18n from 'i18next'
import {
  Button,
  IconButton,
  List,
  ListRow,
  Sheet,
  Spinner,
  Text,
  TextField,
  toast,
  type IconButtonProps,
} from '@renderer/design'
import { JellyfinHttpError, useSession, type SyncPlayGroup } from '@renderer/jellyfin'
import { memberColor, memberInitials, type Member, type MemberState } from './logic'
import { syncPlayManager, useSyncPlay, type MediaKind } from './manager'
import type { Notice } from './types'
import './syncplay.css'

const useInGroup = () => useSyncPlay((s) => s.phase === 'joined' || s.phase === 'reconnecting')

// --------------------------------------------------------------------------------- avatar

export function MemberAvatar({
  name,
  src,
  size = 32,
}: {
  name: string
  src?: string
  size?: number
}) {
  return (
    <span
      className="jr-sp-avatar"
      style={{ width: size, height: size, background: src ? undefined : memberColor(name) }}
      aria-hidden="true"
    >
      {src ? <img src={src} alt="" draggable={false} /> : memberInitials(name)}
    </span>
  )
}

function AvatarStack({ names, max = 4 }: { names: readonly string[]; max?: number }) {
  return (
    <span className="jr-sp-stack" aria-hidden="true">
      {names.slice(0, max).map((n, i) => (
        <MemberAvatar key={`${n}-${i}`} name={n} size={24} />
      ))}
      {names.length > max && <span className="jr-sp-stack__more">+{names.length - max}</span>}
    </span>
  )
}

// --------------------------------------------------------------------------------- button

/** "Watch together" / "Listen together": opens the SyncPlay sheet; pressed while in a group. */
export function SyncPlayButton({
  kind,
  variant = 'plain',
  size,
}: {
  kind: MediaKind
  variant?: IconButtonProps['variant']
  size?: IconButtonProps['size']
}) {
  const { t } = useTranslation('syncplay')
  const inGroup = useInGroup()
  const count = useSyncPlay((s) => s.members.length)
  const hasSession = useSession((s) => Boolean(s.api))
  if (!hasSession) return null
  return (
    <span className="jr-sp-button">
      <IconButton
        icon={<Users />}
        label={kind === 'video' ? t('watchTogether') : t('listenTogether')}
        variant={variant}
        size={size}
        pressed={inGroup}
        aria-haspopup="dialog"
        onClick={() => syncPlayManager.openSheet(kind)}
      />
      {inGroup && count > 1 && (
        <span className="jr-sp-button__count" aria-hidden="true">
          {count}
        </span>
      )}
    </span>
  )
}

// --------------------------------------------------------------------------------- badge

/** Small capsule shown on a player while in a group; opens the sheet. */
export function SyncPlayBadge({ kind }: { kind: MediaKind }) {
  const { t } = useTranslation('syncplay')
  const inGroup = useInGroup()
  const phase = useSyncPlay((s) => s.phase)
  const group = useSyncPlay((s) => s.group)
  const count = useSyncPlay((s) => s.members.length)
  if (!inGroup || !group) return null
  const reconnecting = phase === 'reconnecting'
  return (
    <button
      type="button"
      className="jr-sp-badge"
      data-state={reconnecting ? 'reconnecting' : 'live'}
      aria-label={t('badgeLabel', { name: group.name, count })}
      title={group.name}
      onClick={() => syncPlayManager.openSheet(kind)}
    >
      <span className="jr-sp-badge__dot" aria-hidden="true" />
      <Users aria-hidden="true" />
      <span>{reconnecting ? t('reconnecting') : t('badge', { count })}</span>
    </button>
  )
}

// --------------------------------------------------------------------------------- sheet

function stateLabel(state: MemberState): string {
  return i18n.t(`syncplay:sheet.member.${state}`)
}

function MemberRow({ member, avatar }: { member: Member; avatar?: string }) {
  const { t } = useTranslation('syncplay')
  return (
    <ListRow
      leading={<MemberAvatar name={member.name} src={member.isSelf ? avatar : undefined} />}
      title={member.isSelf ? `${member.name} (${t('sheet.you')})` : member.name}
      trailing={
        <span className="jr-sp-state" data-state={member.state}>
          <span className="jr-sp-state__dot" aria-hidden="true" />
          {stateLabel(member.state)}
        </span>
      }
    />
  )
}

function GroupPanel() {
  const { t } = useTranslation('syncplay')
  const group = useSyncPlay((s) => s.group)
  const members = useSyncPlay((s) => s.members)
  const groupState = useSyncPlay((s) => s.groupState)
  const avatar = useSyncPlay((s) => s.selfAvatar)
  const item = useSyncPlay((s) => s.currentItem)
  const queue = useSyncPlay((s) => s.queue)
  if (!group) return null
  const hasQueue = Boolean(queue && queue.playlist.length > 0)
  return (
    <div className="jr-sp-panel">
      <div className="jr-sp-group-head">
        <span className="jr-sp-group-head__icon" aria-hidden="true">
          <Users />
        </span>
        <span className="jr-sp-group-head__text">
          <Text variant="headline" truncate>
            {group.name}
          </Text>
          <Text variant="footnote" color="secondary">
            {t(`sheet.state.${groupState}`)} · {t('sheet.people', { count: members.length })}
          </Text>
        </span>
      </div>
      <div className="jr-sp-now">
        <Text variant="footnote" color="secondary">
          {t('sheet.nowPlaying')}
        </Text>
        <Text variant="callout" truncate={Boolean(item)}>
          {item
            ? item.SeriesName
              ? `${item.SeriesName} · ${item.Name}`
              : item.Name
            : hasQueue
              ? '—'
              : t('sheet.nothing')}
        </Text>
      </div>
      <List variant="inset" header={t('sheet.members')} aria-label={t('sheet.members')}>
        {members.map((m, i) => (
          <MemberRow key={`${m.name}-${i}`} member={m} avatar={avatar} />
        ))}
      </List>
      <Text variant="footnote" color="secondary">
        {t('sheet.leaveHint')}
      </Text>
    </div>
  )
}

function JoinPanel() {
  const { t } = useTranslation('syncplay')
  const account = useSession((s) => s.account)
  const phase = useSyncPlay((s) => s.phase)
  const [name, setName] = useState(() => t('sheet.defaultName', { user: account?.userName ?? '' }))
  const [groups, setGroups] = useState<SyncPlayGroup[] | null>(null)
  const [error, setError] = useState<'none' | 'failed' | 'noAccess'>('none')
  const [loading, setLoading] = useState(false)
  const connecting = phase === 'connecting'

  const load = useCallback(async () => {
    setLoading(true)
    try {
      setGroups(await syncPlayManager.listGroups())
      setError('none')
    } catch (e) {
      setError(e instanceof JellyfinHttpError && e.status === 403 ? 'noAccess' : 'failed')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
    const timer = setInterval(() => void load(), 5000)
    return () => clearInterval(timer)
  }, [load])

  if (error === 'noAccess') {
    return (
      <div className="jr-sp-panel">
        <Text variant="callout" color="secondary">
          {t('sheet.noAccess')}
        </Text>
      </div>
    )
  }

  return (
    <div className="jr-sp-panel">
      <form
        className="jr-sp-create"
        onSubmit={(e) => {
          e.preventDefault()
          void syncPlayManager.create(
            name.trim() || t('sheet.defaultName', { user: account?.userName ?? '' }),
          )
        }}
      >
        <Text variant="headline">{t('sheet.startSection')}</Text>
        <div className="jr-sp-create__row">
          <TextField
            aria-label={t('sheet.roomName')}
            value={name}
            maxLength={80}
            onChange={(e) => setName(e.target.value)}
            disabled={connecting}
          />
          <Button type="submit" loading={connecting} disabled={connecting}>
            {t('sheet.create')}
          </Button>
        </div>
        <Text variant="footnote" color="secondary">
          {t('sheet.createHint')}
        </Text>
      </form>

      <div className="jr-sp-rooms">
        <div className="jr-sp-rooms__head">
          <Text variant="headline">{t('sheet.roomsSection')}</Text>
          <IconButton
            size="small"
            icon={loading ? <Spinner size="small" /> : <RefreshCw />}
            label={t('sheet.refresh')}
            onClick={() => void load()}
          />
        </div>
        {error === 'failed' && (
          <Text variant="footnote" color="secondary">
            {t('sheet.loadFailed')}
          </Text>
        )}
        {groups && groups.length === 0 && error === 'none' && (
          <Text variant="footnote" color="secondary">
            {t('sheet.empty')}
          </Text>
        )}
        {groups && groups.length > 0 && (
          <List variant="inset" aria-label={t('sheet.roomsSection')}>
            {groups.map((g) => (
              <ListRow
                key={g.id}
                leading={<AvatarStack names={g.participants} />}
                title={g.name}
                subtitle={`${t(`sheet.state.${g.state}`)} · ${t('sheet.people', { count: g.participants.length })}`}
                trailing={
                  <Button
                    size="small"
                    variant="tinted"
                    disabled={connecting}
                    onClick={() => void syncPlayManager.join(g.id)}
                  >
                    {t('sheet.join')}
                  </Button>
                }
              />
            ))}
          </List>
        )}
      </div>
    </div>
  )
}

/** The sheet to create / join / leave a room. One instance, mounted by the host. */
export function SyncPlaySheet() {
  const { t } = useTranslation('syncplay')
  const open = useSyncPlay((s) => s.sheetOpen)
  const kind = useSyncPlay((s) => s.sheetKind)
  const inGroup = useInGroup()
  const account = useSession((s) => s.account)
  const close = () => syncPlayManager.closeSheet()
  return (
    <Sheet
      open={open}
      onClose={close}
      size="medium"
      className="jr-sp-sheet"
      title={kind === 'video' ? t('watchTogether') : t('listenTogether')}
      description={t('sheet.description', { server: account?.serverName ?? '' })}
      footer={
        inGroup ? (
          <>
            <Button variant="gray" destructive onClick={() => void syncPlayManager.leave()}>
              {t('sheet.leave')}
            </Button>
            <Button onClick={close}>{t('sheet.done')}</Button>
          </>
        ) : (
          <Button variant="gray" onClick={close}>
            {t('sheet.done')}
          </Button>
        )
      }
    >
      {inGroup ? <GroupPanel /> : <JoinPanel />}
    </Sheet>
  )
}

// --------------------------------------------------------------------------------- host

export function noticeText(n: Notice): { title: string; error?: boolean } {
  const t = (key: string, opts: Record<string, unknown> = {}) =>
    i18n.t(`syncplay:notice.${key}`, opts)
  switch (n.kind) {
    case 'userJoined':
    case 'userLeft':
      return { title: t(n.kind, { name: n.name }) }
    case 'paused':
    case 'resumed':
    case 'seeked':
      return { title: n.by ? t(`${n.kind}By`, { name: n.by }) : t(n.kind) }
    case 'nowPlaying':
      return {
        title: n.by
          ? t('nowPlayingBy', { name: n.by, title: n.title })
          : t('nowPlaying', { title: n.title }),
      }
    case 'waiting':
      return { title: n.name ? t('waitingFor', { name: n.name }) : t('waiting') }
    case 'joined':
      return { title: t('joined', { group: n.group }) }
    case 'itemUnavailable':
    case 'libraryDenied':
    case 'joinDenied':
    case 'createDenied':
    case 'error':
    case 'groupClosed':
      return { title: t(n.kind), error: n.kind !== 'groupClosed' }
    default:
      return { title: t(n.kind) }
  }
}

/** Always mounted (shell background): router access for the manager, toasts, the sheet. */
export function SyncPlayHost() {
  const navigate = useNavigate()
  useEffect(() => {
    syncPlayManager.setNavigator((path) => void navigate(path))
    return () => syncPlayManager.setNavigator(null)
  }, [navigate])
  useEffect(
    () =>
      syncPlayManager.onNotice((n) => {
        const { title, error } = noticeText(n)
        toast({ title, icon: <Users />, tone: error ? 'error' : 'default' })
      }),
    [],
  )
  return <SyncPlaySheet />
}
