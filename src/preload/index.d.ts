import type { JellyraApi } from '@shared/ipc'

declare global {
  interface Window {
    jellyra: JellyraApi
  }
}

export {}
