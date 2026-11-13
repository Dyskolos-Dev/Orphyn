/**
 * Window dragging from the areas the renderer marks with the `jr-drag` class (the same areas
 * Electron reads through `app-region: drag` in `styles/base.css`). Interactive children and
 * `.jr-no-drag` stay clickable, with the same selector as the CSS. A double click maximizes or
 * restores, like a native title bar. Tauri's own `data-tauri-drag-region` only reacts on the
 * marked element itself, not on its children, hence this listener.
 */
import { getCurrentWindow } from '@tauri-apps/api/window'
import { call } from './tauri'

export const DRAG_SELECTOR = '.jr-drag'
export const NO_DRAG_SELECTOR =
  "button, a, input, select, textarea, [role='button'], [tabindex], .jr-no-drag"

/** True when a press on `target` should move the window. */
export function isDragTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  const region = target.closest(DRAG_SELECTOR)
  if (!region) return false
  const blocker = target.closest(NO_DRAG_SELECTOR)
  return !blocker || !region.contains(blocker)
}

export function installDragRegions(): void {
  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0 || !isDragTarget(e.target)) return
    e.preventDefault()
    if (e.detail === 2) void call('window_toggle_maximize')
    else void getCurrentWindow().startDragging()
  })
}
