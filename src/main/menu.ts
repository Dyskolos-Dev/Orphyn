import { app, BrowserWindow, Menu, shell, type MenuItemConstructorOptions } from 'electron'
import { APP_CHANNELS } from '@shared/ipc/app'

export const WEBSITE_URL = 'https://dyskolos.fr'

function send(channel: string): void {
  const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0]
  if (win && !win.isDestroyed()) win.webContents.send(channel)
}

/** Application menu template. On Windows / Linux it is hidden (Alt shows it) but keeps Edit roles. */
export function buildMenuTemplate(
  platform: NodeJS.Platform,
  name = 'Lumen',
): MenuItemConstructorOptions[] {
  const mac = platform === 'darwin'
  const settings: MenuItemConstructorOptions = {
    label: mac ? 'Settings…' : 'Settings',
    accelerator: mac ? 'Command+,' : 'Ctrl+,',
    click: () => send(APP_CHANNELS.openSettings),
  }
  return [
    ...(mac
      ? ([
          {
            label: name,
            submenu: [
              { role: 'about' },
              { type: 'separator' },
              settings,
              { type: 'separator' },
              { role: 'services' },
              { type: 'separator' },
              { role: 'hide' },
              { role: 'hideOthers' },
              { role: 'unhide' },
              { type: 'separator' },
              { role: 'quit' },
            ],
          },
        ] as MenuItemConstructorOptions[])
      : ([
          {
            label: 'File',
            submenu: [settings, { type: 'separator' }, { role: 'quit' }],
          },
        ] as MenuItemConstructorOptions[])),
    {
      label: 'Edit',
      submenu: [
        { role: 'undo' },
        { role: 'redo' },
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { role: 'selectAll' },
      ],
    },
    {
      label: 'View',
      submenu: [
        {
          label: 'Refresh Library',
          accelerator: mac ? 'Command+R' : 'Ctrl+R',
          click: () => send(APP_CHANNELS.refresh),
        },
        { type: 'separator' },
        { role: 'togglefullscreen' },
      ],
    },
    {
      label: 'Window',
      submenu: mac
        ? [{ role: 'minimize' }, { role: 'zoom' }, { type: 'separator' }, { role: 'front' }]
        : [{ role: 'minimize' }, { role: 'close' }],
    },
    {
      role: 'help',
      submenu: [
        { label: 'Lumen Website', click: () => void shell.openExternal(WEBSITE_URL) },
        {
          label: 'Releases',
          click: () => void shell.openExternal('https://github.com/Dyskolos-Dev/Lumen/releases'),
        },
      ],
    },
  ]
}

export function installApplicationMenu(): void {
  Menu.setApplicationMenu(Menu.buildFromTemplate(buildMenuTemplate(process.platform, app.name)))
}
