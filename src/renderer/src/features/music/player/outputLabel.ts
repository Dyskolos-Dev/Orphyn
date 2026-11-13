import { create } from 'zustand'

/** Name of the remote device the music plays on (set by the cast feature), null when local. */
export const useMusicOutputName = create<{ name: string | null }>()(() => ({ name: null }))

export const setMusicOutputName = (name: string | null): void =>
  useMusicOutputName.setState({ name })
