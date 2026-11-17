/**
 * Audio output device choice (music AND video). The saved choice lives in the playback
 * preferences (`audioOutputId`); this module lists the output devices, works out which one really
 * applies (a saved device that is unplugged falls back to the system default, and is picked up
 * again when it comes back) and pushes it to every registered media element / AudioContext.
 * Needs `setSinkId` (Chromium / WebView2); elsewhere `isSinkSupported()` is false and the setting
 * is hidden.
 */
import { useSyncExternalStore } from 'react'
import {
  getPlaybackPreferences,
  subscribePlaybackPreferences,
} from '@renderer/features/settings/preferences'

export interface AudioDevice {
  deviceId: string
  label: string
}

/** Anything that can be pointed at an output: an HTMLMediaElement or an AudioContext. */
export interface SinkTarget {
  setSinkId?: (id: string) => Promise<void>
}

export function isSinkSupported(): boolean {
  return typeof HTMLMediaElement !== 'undefined' && 'setSinkId' in HTMLMediaElement.prototype
}

/**
 * The sink id to apply: '' (system default) unless `saved` is a device currently present.
 * `devices === null` means the list is unknown (not enumerated yet / unsupported): trust `saved`.
 */
export function resolveSinkId(
  saved: string | null,
  devices: readonly { deviceId: string }[] | null,
): string {
  if (!saved || saved === 'default' || saved === 'communications') return ''
  if (devices === null) return saved
  return devices.some((d) => d.deviceId === saved) ? saved : ''
}

/** Real outputs only: drops the virtual "default" / "communications" entries, names the blanks. */
export function normalizeDevices(
  raw: readonly { kind: string; deviceId: string; label: string }[],
  fallbackLabel: (n: number) => string,
): { devices: AudioDevice[]; labelsHidden: boolean } {
  const outs = raw.filter(
    (d) =>
      d.kind === 'audiooutput' &&
      d.deviceId &&
      d.deviceId !== 'default' &&
      d.deviceId !== 'communications',
  )
  return {
    devices: outs.map((d, i) => ({ deviceId: d.deviceId, label: d.label || fallbackLabel(i + 1) })),
    labelsHidden: outs.length > 0 && outs.every((d) => !d.label),
  }
}

interface State {
  /** null until the first enumeration succeeded. */
  devices: AudioDevice[] | null
  labelsHidden: boolean
}

let state: State = { devices: null, labelsHidden: false }
let started = false
let applied = ''
let fallbackLabel: (n: number) => string = (n) => `Output ${n}`
const targets = new Set<SinkTarget>()
const listeners = new Set<() => void>()

export function effectiveSinkId(): string {
  return resolveSinkId(getPlaybackPreferences().audioOutputId, state.devices)
}

function push(target: SinkTarget, id: string): void {
  try {
    void target.setSinkId?.(id)?.catch(() => {})
  } catch {
    // A target that refuses the device keeps playing on its current one.
  }
}

function sync(): void {
  const id = effectiveSinkId()
  if (id === applied) return
  applied = id
  targets.forEach((t) => push(t, id))
}

export async function refreshDevices(): Promise<void> {
  if (!isSinkSupported() || !navigator.mediaDevices?.enumerateDevices) return
  try {
    const { devices, labelsHidden } = normalizeDevices(
      await navigator.mediaDevices.enumerateDevices(),
      fallbackLabel,
    )
    state = { devices, labelsHidden }
    sync()
    listeners.forEach((l) => l())
  } catch {
    // Keep the previous list.
  }
}

function start(): void {
  if (started || !isSinkSupported()) return
  started = true
  subscribePlaybackPreferences(sync)
  navigator.mediaDevices?.addEventListener?.('devicechange', () => void refreshDevices())
  void refreshDevices()
  sync()
}

/** Names used when the browser hides device labels (no media permission). */
export function setFallbackLabel(fn: (n: number) => string): void {
  fallbackLabel = fn
}

/** Routes `target` to the chosen output now and whenever it changes. Returns the unregister. */
export function registerSinkTarget(target: SinkTarget): () => void {
  targets.add(target)
  start()
  if (applied !== '') push(target, applied)
  return () => {
    targets.delete(target)
  }
}

/**
 * Device names are only given once the page holds a media permission. One-time request from a
 * click in Settings: the microphone stream is stopped at once, only the permission is kept.
 */
export async function revealDeviceLabels(): Promise<void> {
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    stream.getTracks().forEach((t) => t.stop())
  } catch {
    // Denied: the generic names stay.
  }
  await refreshDevices()
}

function subscribe(l: () => void): () => void {
  start()
  listeners.add(l)
  return () => listeners.delete(l)
}

const getState = () => state

export function useAudioDevices(): State & { supported: boolean } {
  const s = useSyncExternalStore(subscribe, getState, getState)
  return { ...s, supported: isSinkSupported() }
}

/** Test hook. */
export function resetAudioDeviceState(): void {
  state = { devices: null, labelsHidden: false }
  applied = ''
  targets.clear()
}
