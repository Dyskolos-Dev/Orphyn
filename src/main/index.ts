import { app, BrowserWindow } from 'electron'
import { registerAccountsIpc } from './accounts'
import { registerCastIpc } from './cast'
import { applyStartupSwitches } from './startup'
import { installApplicationMenu } from './menu'
import { registerUpdater } from './updater'
import { createMainWindow } from './window'

// Must run before `ready`: user data location and Chromium features are fixed at startup.
applyStartupSwitches()

void app.whenReady().then(async () => {
  // The renderer asks for the accounts as soon as it loads: the store must be open first.
  await registerAccountsIpc()
  registerCastIpc()
  installApplicationMenu()
  registerUpdater()
  createMainWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createMainWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
