import { registerShellExtension } from '@renderer/shell'
import { ServerGuard } from './ServerGuard'
import { ServerSwitcher } from './ServerSwitcher'

// Plugged into the shell without editing shell files: the guard wraps every route, the switcher
// lives in the sidebar footer. Imported for its side effect by routes.tsx.
registerShellExtension({ id: 'servers', provider: ServerGuard, serverSwitcher: ServerSwitcher })
