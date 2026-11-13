import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from 'react'
import { onPip, pipAvailable, sendPip, usePipState } from '@renderer/features/pip/events'
import {
  PIP_EVENTS,
  type VideoPipProgress,
  type VideoPipStart,
} from '@renderer/features/pip/protocol'
import {
  handOverStart,
  returnPosition,
  shouldResumeOnReturn,
} from '@renderer/features/pip/videoHandover'
import type { VideoPlaybackController } from '../controller'

/** Safari / WKWebView's own presentation mode API (macOS), used when `requestPictureInPicture` is missing. */
type WebkitVideo = HTMLVideoElement & {
  webkitSupportsPresentationMode?: (mode: string) => boolean
  webkitSetPresentationMode?: (mode: string) => void
  webkitPresentationMode?: string
}

interface Options {
  controller: VideoPlaybackController
  videoRef: RefObject<HTMLVideoElement | null>
  itemId: string
  title: string
  subtitle?: string
}

type Phase = 'idle' | 'opening' | 'open'

const hasNativePip = (video: WebkitVideo | null): boolean =>
  Boolean(
    video &&
    ((document.pictureInPictureEnabled && 'requestPictureInPicture' in video) ||
      video.webkitSupportsPresentationMode?.('picture-in-picture')),
  )

async function toggleNative(video: WebkitVideo): Promise<boolean> {
  try {
    if (document.pictureInPictureElement) {
      await document.exitPictureInPicture()
    } else if (document.pictureInPictureEnabled && 'requestPictureInPicture' in video) {
      await video.requestPictureInPicture()
    } else if (video.webkitSetPresentationMode) {
      video.webkitSetPresentationMode(
        video.webkitPresentationMode === 'picture-in-picture' ? 'inline' : 'picture-in-picture',
      )
    } else {
      return false
    }
    return true
  } catch {
    return false
  }
}

/**
 * Picture in Picture for the open video.
 *
 * 1. The webview's native PiP when it has one (WebView2, WKWebView): the same `<video>` keeps
 *    playing, so the controller keeps reporting progress to Jellyfin and nothing is duplicated.
 * 2. Otherwise (WebKitGTK) a custom always-on-top window: the main player pauses and hands over
 *    its position, the PiP plays the item with its own controller, and closing it resumes the main
 *    player where the PiP stopped (see `features/pip/videoHandover.ts`).
 */
export function useVideoPip({ controller, videoRef, itemId, title, subtitle }: Options) {
  const pip = usePipState()
  const [nativeOpen, setNativeOpen] = useState(false)
  const phase = useRef<Phase>('idle')
  const last = useRef<VideoPipProgress | null>(null)
  const before = useRef({ wasPlaying: false, position: 0 })
  const start = useRef<VideoPipStart | null>(null)

  const available = useMemo(
    () => hasNativePip(document.createElement('video')) || pipAvailable(),
    [],
  )

  const sendStart = useCallback(() => {
    if (start.current) sendPip(PIP_EVENTS.videoStart, start.current)
  }, [])

  const toggle = useCallback(async () => {
    const video = videoRef.current as WebkitVideo | null
    if (!video) return
    if (phase.current !== 'idle') {
      await window.orphyn?.pip?.close()
      return
    }
    if (hasNativePip(video) && (await toggleNative(video))) return
    const api = window.orphyn?.pip
    if (!api) return
    // Custom window: hand over.
    const snap = controller.getSnapshot()
    const position = handOverStart(video.currentTime, snap.durationSeconds)
    before.current = {
      wasPlaying: snap.status === 'playing' || snap.status === 'loading',
      position,
    }
    last.current = null
    start.current = {
      itemId,
      title,
      subtitle,
      positionSec: position,
      audioIndex: snap.audioIndex,
      subtitleIndex:
        snap.subtitleIndex != null && snap.subtitleIndex >= 0 ? snap.subtitleIndex : undefined,
    }
    phase.current = 'opening'
    controller.pause()
    try {
      await api.open('video')
    } catch {
      phase.current = 'idle'
      if (before.current.wasPlaying) controller.resume()
    }
  }, [controller, itemId, subtitle, title, videoRef])

  // Native PiP state (the viewer can also close it with the window's own button).
  useEffect(() => {
    const video = videoRef.current
    if (!video) return
    const on = () => setNativeOpen(true)
    const off = () => setNativeOpen(false)
    video.addEventListener('enterpictureinpicture', on)
    video.addEventListener('leavepictureinpicture', off)
    return () => {
      video.removeEventListener('enterpictureinpicture', on)
      video.removeEventListener('leavepictureinpicture', off)
    }
  }, [videoRef])

  // Custom window: hand-over on ready, progress back, resume on close.
  useEffect(() => {
    const offReady = onPip<{ kind?: string }>(PIP_EVENTS.ready, (e) => {
      if (e?.kind === 'video' && phase.current !== 'idle') sendStart()
    })
    const offProgress = onPip<VideoPipProgress>(PIP_EVENTS.videoProgress, (p) => {
      if (phase.current !== 'idle') last.current = p
    })
    return () => {
      offReady()
      offProgress()
    }
  }, [sendStart])

  const customOpen = pip.open && pip.kind === 'video'
  useEffect(() => {
    if (customOpen) {
      if (phase.current === 'opening') phase.current = 'open'
      return
    }
    if (phase.current !== 'open') return
    // The PiP window closed: continue in the main window where it stopped.
    phase.current = 'idle'
    const snap = controller.getSnapshot()
    const at = returnPosition(last.current, before.current.position, snap.durationSeconds)
    controller.seek(at)
    if (shouldResumeOnReturn(last.current, before.current.wasPlaying)) controller.resume()
    last.current = null
  }, [customOpen, controller])

  // Leaving the player takes the custom PiP down with it.
  useEffect(
    () => () => {
      if (phase.current !== 'idle') void window.orphyn?.pip?.close()
    },
    [],
  )

  // Ctrl/Cmd+Shift+P.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.shiftKey || e.altKey || !(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'p')
        return
      e.preventDefault()
      void toggle()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [toggle])

  return { available, active: nativeOpen || customOpen, toggle }
}
