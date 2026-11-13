import { registerShellExtension } from '@renderer/shell'
import { PlaylistHost } from './PlaylistHost'

// Dialogs, sidebar playlists and drop targets: always mounted, no shell file edited.
registerShellExtension({ id: 'music-playlist-host', background: PlaylistHost })
