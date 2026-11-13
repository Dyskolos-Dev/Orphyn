/**
 * Picture-in-picture / mini player window (Tauri shell only, filled by the PiP migration task).
 * Optional in `OrphynApi`: the Electron shell does not provide it.
 */
export const PIP_CHANNELS = {
  /** Main -> renderer event, payload: {@link PipState}. */
  stateChanged: 'orphyn:pip:state-changed',
} as const

export type PipKind = 'music' | 'video'

export interface PipState {
  open: boolean
  kind: PipKind | null
}

export interface PipApi {
  getState(): Promise<PipState>
  open(kind: PipKind): Promise<void>
  close(): Promise<void>
  onStateChange(listener: (state: PipState) => void): () => void
}
