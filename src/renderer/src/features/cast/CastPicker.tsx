import { Check, Laptop, Speaker, Tv } from 'lucide-react'
import { useState, type RefObject } from 'react'
import { useTranslation } from 'react-i18next'
import { Popover, Slider, Spinner, Text, toast } from '@renderer/design'
import type { CastDevice } from '@shared/cast'
import { getCastApi } from './api'
import { devicesForScope, groupDevices, type CastScope } from './devices'
import { useCastDevices, useCastSession } from './hooks'
import { useCastState } from './store'
import './cast.css'

export interface CastPickerProps {
  scope: CastScope
  open: boolean
  onClose(): void
  anchor: RefObject<HTMLElement | null>
  placement?: 'top-end' | 'bottom-end'
  /** Device the player currently plays on (null = this computer). */
  currentId: string | null
  /** Moves playback to the device, or back to this computer with null. Rejects with a message. */
  onSelect(device: CastDevice | null): Promise<void>
}

function DeviceVolume({ deviceId }: { deviceId: string }) {
  const { t } = useTranslation('cast')
  const session = useCastSession(deviceId)
  const [drag, setDrag] = useState<number | null>(null)
  const volume = drag ?? session?.volume ?? 1
  return (
    <div className="jr-cast__volume">
      <Slider
        aria-label={t('volume')}
        variant="regular"
        value={volume}
        onChange={(v) => {
          setDrag(v)
          void getCastApi()
            ?.setVolume(deviceId, v)
            .catch(() => undefined)
        }}
        onChangeEnd={() => setDrag(null)}
      />
    </div>
  )
}

/** AirPlay-style popover: "This computer" then the devices, grouped by speakers and TVs. */
export function CastPicker({
  scope,
  open,
  onClose,
  anchor,
  placement = 'top-end',
  currentId,
  onSelect,
}: CastPickerProps) {
  const { t } = useTranslation('cast')
  const devices = devicesForScope(useCastDevices(open), scope)
  const connectingId = useCastState((s) => s.connectingId)

  const choose = async (device: CastDevice | null) => {
    if ((device?.id ?? null) === currentId || connectingId) return
    useCastState.setState({ connectingId: device?.id ?? 'local' })
    try {
      await onSelect(device)
      if (!device) onClose()
    } catch (e) {
      toast({
        title: t('connectFailed', { device: device?.name ?? t('thisComputer') }),
        description: e instanceof Error ? e.message : String(e),
        tone: 'error',
      })
    } finally {
      useCastState.setState({ connectingId: null })
    }
  }

  const row = (device: CastDevice | null, icon: React.ReactNode) => {
    const id = device?.id ?? null
    const selected = id === currentId
    const connecting = connectingId === (id ?? 'local')
    return (
      <li key={id ?? 'local'}>
        <button
          type="button"
          role="menuitemradio"
          aria-checked={selected}
          className="jr-cast__row"
          disabled={Boolean(connectingId) && !connecting}
          onClick={() => void choose(device)}
        >
          <span className="jr-cast__icon" aria-hidden="true">
            {icon}
          </span>
          <Text variant="body" truncate className="jr-cast__name">
            {device?.name ?? t('thisComputer')}
          </Text>
          {connecting ? (
            <Spinner size="small" aria-label={t('connecting')} />
          ) : selected ? (
            <Check className="jr-cast__check" aria-hidden="true" />
          ) : null}
        </button>
        {selected && device && device.kind !== 'airplay' && <DeviceVolume deviceId={device.id} />}
      </li>
    )
  }

  return (
    <Popover
      open={open}
      onClose={onClose}
      anchor={anchor}
      placement={placement}
      aria-label={t('title')}
      width={290}
    >
      <div className="jr-cast">
        <Text variant="headline" className="jr-cast__title">
          {t('title')}
        </Text>
        <ul className="jr-cast__list" role="menu" aria-label={t('title')}>
          {row(null, <Laptop />)}
        </ul>
        {groupDevices(devices).map(({ group, devices: list }) => (
          <section key={group} aria-label={t(`group.${group}`)}>
            <Text variant="caption" color="secondary" className="jr-cast__group">
              {t(`group.${group}`)}
            </Text>
            <ul className="jr-cast__list" role="menu">
              {list.map((d) => row(d, group === 'tv' ? <Tv /> : <Speaker />))}
            </ul>
          </section>
        ))}
        {devices.length === 0 && (
          <Text variant="footnote" color="secondary" className="jr-cast__hint">
            {t(scope === 'video' ? 'noneVideo' : 'none')}
          </Text>
        )}
      </div>
    </Popover>
  )
}
