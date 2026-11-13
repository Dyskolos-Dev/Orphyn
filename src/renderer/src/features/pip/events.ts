import { emit } from '@tauri-apps/api/event'
import { useEffect, useState } from 'react'
import type { PipKind, PipState } from '@shared/ipc/pip'
import { subscribe } from '@renderer/bridge/tauri'

/** Broadcasts a PiP message to every window (the other side listens with `onPip`). */
export function sendPip(event: string, payload?: unknown): void {
  void emit(event, payload).catch(() => undefined)
}

/** Listens to a PiP message; returns the unsubscribe. */
export const onPip = subscribe

const CLOSED: PipState = { open: false, kind: null }

/** Whether a PiP window is open (and which), kept in sync by the Rust side. */
export function usePipState(): PipState {
  const [state, setState] = useState<PipState>(CLOSED)
  useEffect(() => {
    const api = window.orphyn?.pip
    if (!api) return
    void api.getState().then(setState, () => undefined)
    return api.onStateChange(setState)
  }, [])
  return state
}

/** Opens the PiP of `kind`, or closes it when it is already the one showing. */
export async function togglePip(kind: PipKind, state: PipState): Promise<void> {
  const api = window.orphyn?.pip
  if (!api) return
  try {
    if (state.open && state.kind === kind) await api.close()
    else await api.open(kind)
  } catch {
    /* the window could not be created: nothing to show */
  }
}

/** True in the Tauri shell, where the PiP windows exist. */
export const pipAvailable = (): boolean => Boolean(window.orphyn?.pip)
