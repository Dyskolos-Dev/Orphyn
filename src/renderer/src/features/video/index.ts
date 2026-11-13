export { videoRoutes } from './routes'
export {
  getActivePlaybackController,
  subscribeActiveController,
  setActiveController,
} from './active'
export { VideoPlaybackController } from './controller'
export type {
  PlayOptions,
  PlaybackController,
  PlaybackSnapshot,
  PlaybackStatus,
} from './controller'
export { registerVideoCast, VideoCastButtonSlot, VideoCastScreenSlot } from './castSlot'
export type { VideoCastProps } from './castSlot'
