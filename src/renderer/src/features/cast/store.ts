import { create } from 'zustand'
import type { CastKind } from '@shared/cast'
import type { CastScope } from './devices'

export interface CastTarget {
  deviceId: string
  name: string
  kind: CastKind
  scope: CastScope
}

interface CastState {
  /** Device the music player or the video player is currently playing on. */
  target: CastTarget | null
  /** Device a switch is in progress for (spinner in the picker). */
  connectingId: string | null
}

export const useCastState = create<CastState>()(() => ({ target: null, connectingId: null }))

export const getCastTarget = (scope: CastScope): CastTarget | null => {
  const t = useCastState.getState().target
  return t && t.scope === scope ? t : null
}
