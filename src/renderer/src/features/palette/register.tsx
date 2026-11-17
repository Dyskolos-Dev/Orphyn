import { registerShellExtension } from '@renderer/shell'
import './commands'
import { PaletteHost } from './PaletteHost'

// Plugged into the shell as an always-mounted component; imported for its side effect by the app routes.
registerShellExtension({ id: 'palette', background: PaletteHost })
