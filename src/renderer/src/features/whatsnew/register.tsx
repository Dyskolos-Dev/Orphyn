import { registerShellExtension } from '@renderer/shell'
import { WhatsNewHost } from './WhatsNewHost'

// Plugged into the shell as a provider; imported for its side effect by the app routes.
registerShellExtension({ id: 'whatsnew', provider: WhatsNewHost })
