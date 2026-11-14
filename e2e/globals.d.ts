// The renderer's `window.orphyn`, as the scenarios call it inside the page (`page.evaluate`).
import type { OrphynApi } from '../src/shared/ipc'

declare global {
  interface Window {
    orphyn: OrphynApi
  }
}

export {}
