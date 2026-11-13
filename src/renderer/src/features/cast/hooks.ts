import { useEffect, useState } from 'react'
import type { CastDevice, CastSessionState } from '@shared/cast'
import { getCastApi } from './api'

/** Live device list. Discovery runs only while `active` (the picker is open). */
export function useCastDevices(active: boolean): CastDevice[] {
  const [devices, setDevices] = useState<CastDevice[]>([])
  useEffect(() => {
    const cast = getCastApi()
    if (!cast || !active) return
    let alive = true
    const off = cast.onDevicesChanged((d) => alive && setDevices(d))
    cast
      .startDiscovery()
      .then(() => cast.listDevices())
      .then((d) => alive && setDevices(d))
      .catch(() => undefined)
    return () => {
      alive = false
      off()
      void cast.stopDiscovery().catch(() => undefined)
    }
  }, [active])
  return devices
}

/** Live session state of a device (null while there is none). */
export function useCastSession(deviceId: string | null): CastSessionState | null {
  const [session, setSession] = useState<CastSessionState | null>(null)
  useEffect(() => {
    const cast = getCastApi()
    setSession(null)
    if (!cast || !deviceId) return
    let alive = true
    const off = cast.onSessionChanged((s) => alive && s.deviceId === deviceId && setSession(s))
    cast
      .getSession(deviceId)
      .then((s) => alive && setSession(s))
      .catch(() => undefined)
    return () => {
      alive = false
      off()
    }
  }, [deviceId])
  return session
}
