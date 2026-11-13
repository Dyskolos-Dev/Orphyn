import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { app, BrowserWindow, ipcMain, shell } from 'electron'
import { autoUpdater } from 'electron-updater'
import { UPDATER_CHANNELS as C } from '@shared/ipc/updater'
import { canApplyInApp, UpdaterController } from './state'

const SETTINGS_FILE = 'updater.json'

function settingsPath(): string {
  return join(app.getPath('userData'), SETTINGS_FILE)
}

function loadAutoUpdate(): boolean {
  try {
    const raw = JSON.parse(readFileSync(settingsPath(), 'utf8')) as { autoUpdate?: unknown }
    return raw.autoUpdate !== false
  } catch {
    return true
  }
}

function saveAutoUpdate(enabled: boolean): void {
  try {
    writeFileSync(settingsPath(), JSON.stringify({ autoUpdate: enabled }))
  } catch {
    // Read-only profile: the choice applies for this session.
  }
}

/** Sets up update checks and their IPC. Call once, after `app` is ready. */
export function registerUpdater(): UpdaterController {
  const controller = new UpdaterController(
    autoUpdater,
    {
      canApplyInApp: canApplyInApp(process.platform),
      isPackaged: app.isPackaged,
      openExternal: (url) => void shell.openExternal(url),
      loadAutoUpdate,
      saveAutoUpdate,
    },
    app.getVersion(),
  )
  ipcMain.handle(C.getState, () => controller.getState())
  ipcMain.handle(C.check, () => controller.check())
  ipcMain.handle(C.install, () => controller.install())
  ipcMain.handle(C.openRelease, () => controller.openRelease())
  ipcMain.handle(C.setAutoUpdate, (_e, enabled: unknown) =>
    controller.setAutoUpdate(enabled === true),
  )
  controller.onChange((state) => {
    for (const win of BrowserWindow.getAllWindows()) {
      if (!win.isDestroyed()) win.webContents.send(C.stateChanged, state)
    }
  })
  controller.start()
  return controller
}
