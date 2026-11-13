import { app, BrowserWindow } from 'electron'
import { registerAccountsIpc } from './accounts'
import { registerCastIpc } from './cast'
import { createMainWindow } from './window'

void app.whenReady().then(() => {
  registerAccountsIpc()
  registerCastIpc()
  createMainWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createMainWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
