import type { OrphynApi } from '@shared/ipc'

declare global {
  interface Window {
    orphyn: OrphynApi
  }
}

export {}
