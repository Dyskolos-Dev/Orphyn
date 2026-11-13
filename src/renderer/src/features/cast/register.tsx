import { registerCastButton } from '@renderer/features/music'
import { registerVideoCast } from '@renderer/features/video'
import { MusicCastButton, VideoCastButton, VideoCastScreen } from './CastButtons'

// Plugs the picker into the music player bar / Now Playing and into the video player.
registerCastButton(MusicCastButton)
registerVideoCast({ Button: VideoCastButton, Screen: VideoCastScreen })
