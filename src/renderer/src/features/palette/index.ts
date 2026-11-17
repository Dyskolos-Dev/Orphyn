// Command palette public API (Ctrl/Cmd+K). Other features can add commands to it.
export { paletteRegistry } from './commands'
export { closePalette, openPalette, usePaletteUi } from './store'
export type { Command, CommandGroup, PaletteContext } from './registry'
