import { readFileSync, writeFileSync } from 'node:fs'
import { release } from 'node:os'
import { join } from 'node:path'
import {
  app,
  BrowserWindow,
  ipcMain,
  nativeTheme,
  screen,
  shell,
  type BrowserWindowConstructorOptions,
  type IpcMainInvokeEvent,
} from 'electron'
import {
  TITLE_BAR_HEIGHT,
  WINDOW_CHANNELS,
  type TitleBarOverlayColors,
  type WindowAppearance,
  type WindowMaterial,
  type WindowState,
} from '@shared/ipc/window'
import {
  MIN_SIZE,
  parseSavedState,
  resolveBounds,
  supportsMica,
  type SavedWindowState,
} from './bounds'

const STATE_FILE = 'window-state.json'
const materials = new WeakMap<BrowserWindow, WindowMaterial>()

function statePath(): string {
  return join(app.getPath('userData'), STATE_FILE)
}

function loadState(): SavedWindowState | null {
  try {
    return parseSavedState(JSON.parse(readFileSync(statePath(), 'utf8')))
  } catch {
    return null
  }
}

function saveState(win: BrowserWindow): void {
  if (win.isDestroyed()) return
  const state: SavedWindowState = {
    // getNormalBounds: the size to come back to, even when maximized / fullscreen.
    bounds: win.getNormalBounds(),
    isMaximized: win.isMaximized(),
    isFullscreen: win.isFullScreen(),
  }
  try {
    writeFileSync(statePath(), JSON.stringify(state))
  } catch {
    // A read-only profile must not break the app; the next launch uses defaults.
  }
}

function snapshot(win: BrowserWindow): WindowState {
  return {
    isFullscreen: win.isFullScreen(),
    isMaximized: win.isMaximized(),
    isFocused: win.isFocused(),
    material: materials.get(win) ?? 'none',
  }
}

function senderWindow(event: IpcMainInvokeEvent): BrowserWindow | null {
  return BrowserWindow.fromWebContents(event.sender)
}

const CSS_COLOR = /^(#[0-9a-f]{3,8}|rgba?\([\d\s.,%]+\)|transparent)$/i

let ipcRegistered = false

/** Window IPC handlers; registered once, they act on the window that sent the call. */
function registerWindowIpc(): void {
  if (ipcRegistered) return
  ipcRegistered = true
  ipcMain.handle(WINDOW_CHANNELS.minimize, (e) => senderWindow(e)?.minimize())
  ipcMain.handle(WINDOW_CHANNELS.toggleMaximize, (e) => {
    const win = senderWindow(e)
    if (!win) return
    if (win.isMaximized()) win.unmaximize()
    else win.maximize()
  })
  ipcMain.handle(WINDOW_CHANNELS.close, (e) => senderWindow(e)?.close())
  ipcMain.handle(WINDOW_CHANNELS.getState, (e) => {
    const win = senderWindow(e)
    return win
      ? snapshot(win)
      : { isFullscreen: false, isMaximized: false, isFocused: false, material: 'none' }
  })
  ipcMain.handle(WINDOW_CHANNELS.setFullscreen, (e, fullscreen: unknown) => {
    senderWindow(e)?.setFullScreen(fullscreen === true)
  })
  ipcMain.handle(WINDOW_CHANNELS.setTitleBarOverlay, (e, colors: TitleBarOverlayColors) => {
    const win = senderWindow(e)
    if (!win || process.platform !== 'win32') return
    if (!CSS_COLOR.test(colors?.color ?? '') || !CSS_COLOR.test(colors?.symbolColor ?? '')) return
    win.setTitleBarOverlay({
      color: colors.color,
      symbolColor: colors.symbolColor,
      height: TITLE_BAR_HEIGHT,
    })
  })
  ipcMain.handle(WINDOW_CHANNELS.setAppearance, (_e, appearance: WindowAppearance) => {
    if (appearance === 'system' || appearance === 'light' || appearance === 'dark') {
      nativeTheme.themeSource = appearance
    }
  })
}

function platformOptions(): { options: BrowserWindowConstructorOptions; material: WindowMaterial } {
  if (process.platform === 'darwin') {
    return {
      material: 'vibrancy',
      options: {
        titleBarStyle: 'hiddenInset',
        trafficLightPosition: { x: 20, y: 20 },
        vibrancy: 'sidebar',
        visualEffectState: 'followWindow',
        backgroundColor: '#00000000',
      },
    }
  }
  if (process.platform === 'win32') {
    const mica = supportsMica(process.platform, release())
    const dark = nativeTheme.shouldUseDarkColors
    return {
      material: mica ? 'mica' : 'none',
      options: {
        titleBarStyle: 'hidden',
        titleBarOverlay: {
          color: '#00000000',
          symbolColor: dark ? '#ffffff' : '#000000',
          height: TITLE_BAR_HEIGHT,
        },
        ...(mica
          ? { backgroundMaterial: 'mica' as const, backgroundColor: '#00000000' }
          : { backgroundColor: dark ? '#1c1c1e' : '#f2f2f7' }),
      },
    }
  }
  // Linux: keep the native frame (works with every WM / tiling setups), hide the menu bar.
  return {
    material: 'none',
    options: {
      frame: true,
      autoHideMenuBar: true,
      backgroundColor: nativeTheme.shouldUseDarkColors ? '#000000' : '#ffffff',
    },
  }
}

/** Creates the main application window. Owner: window agent. */
export function createMainWindow(): BrowserWindow {
  registerWindowIpc()

  const saved = loadState()
  const display = screen.getPrimaryDisplay()
  const bounds = resolveBounds(
    saved?.bounds,
    screen.getAllDisplays().map((d) => d.workArea),
    display.workArea,
  )
  const { options, material } = platformOptions()

  const win = new BrowserWindow({
    ...bounds,
    minWidth: MIN_SIZE.width,
    minHeight: MIN_SIZE.height,
    show: false,
    title: 'Lumen',
    ...options,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  })
  materials.set(win, material)

  if (saved?.isMaximized) win.maximize()
  if (saved?.isFullscreen) win.setFullScreen(true)

  win.on('ready-to-show', () => win.show())

  const emit = () => {
    if (!win.isDestroyed()) win.webContents.send(WINDOW_CHANNELS.stateChanged, snapshot(win))
  }
  win.on('enter-full-screen', emit)
  win.on('leave-full-screen', emit)
  win.on('maximize', emit)
  win.on('unmaximize', emit)
  win.on('focus', emit)
  win.on('blur', emit)

  let saveTimer: NodeJS.Timeout | undefined
  const scheduleSave = () => {
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => saveState(win), 500)
  }
  win.on('resize', scheduleSave)
  win.on('move', scheduleSave)
  win.on('close', () => {
    clearTimeout(saveTimer)
    saveState(win)
  })

  win.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:/i.test(url)) void shell.openExternal(url)
    return { action: 'deny' }
  })

  const devUrl = process.env['ELECTRON_RENDERER_URL']
  if (devUrl) {
    void win.loadURL(devUrl)
  } else {
    void win.loadFile(join(__dirname, '../renderer/index.html'))
  }
  return win
}
