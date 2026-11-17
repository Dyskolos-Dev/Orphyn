/**
 * Crossfade between songs: the pure rules (should we fade, for how long), the equal-power curves
 * and the Web Audio graph the local output uses to apply them.
 */
import type { RepeatMode, Track } from './types'
import { trackDuration } from './track'

export interface CrossfadeSettings {
  enabled: boolean
  /** Wanted fade length, seconds. */
  seconds: number
  /** Gapless albums: do not blend two consecutive tracks of the same album. */
  skipSameAlbum: boolean
}

export const CROSSFADE_OFF: CrossfadeSettings = { enabled: false, seconds: 0, skipSameAlbum: true }
export const CROSSFADE_MIN = 1
export const CROSSFADE_MAX = 12
/** A fade shorter than this is inaudible: play the plain (gapless) switch instead. */
export const MIN_FADE_SECONDS = 0.5

export interface FadePlanInput {
  settings: CrossfadeSettings
  current: Track
  next: Track
  repeat: RepeatMode
  /** Remote (cast) output: it cannot mix two streams, so it switches plainly. */
  casting: boolean
  /** Real duration of the current track when the output knows it (seconds). */
  currentDuration?: number
}

/** True when `next` follows `current` on the same album, as in a gapless album playthrough. */
export function isAlbumContinuation(current: Track, next: Track): boolean {
  if (!current.AlbumId || current.AlbumId !== next.AlbumId) return false
  const a = current.IndexNumber
  const b = next.IndexNumber
  if (a == null || b == null) return true
  return (current.ParentIndexNumber ?? 1) === (next.ParentIndexNumber ?? 1) && b === a + 1
}

/**
 * Seconds of crossfade to use from `current` into `next`; 0 means no crossfade (plain switch).
 * Tracks shorter than twice the wanted length get a proportionally shorter fade.
 */
export function planFade({
  settings,
  current,
  next,
  repeat,
  casting,
  currentDuration,
}: FadePlanInput): number {
  if (!settings.enabled || casting || repeat === 'one') return 0
  if (settings.skipSameAlbum && isAlbumContinuation(current, next)) return 0
  let seconds = Math.min(CROSSFADE_MAX, Math.max(CROSSFADE_MIN, settings.seconds))
  for (const d of [currentDuration || trackDuration(current), trackDuration(next)]) {
    if (d > 0) seconds = Math.min(seconds, d / 2)
  }
  return seconds >= MIN_FADE_SECONDS ? seconds : 0
}

// ---------------------------------------------------------------------------------- curves

/** Equal-power gains at progress `p` (0..1): `in² + out² = 1`, so the loudness stays constant. */
export function fadeGains(p: number): { in: number; out: number } {
  const x = Math.min(1, Math.max(0, p)) * (Math.PI / 2)
  return { in: Math.sin(x), out: Math.cos(x) }
}

/** Samples of the incoming (`in`) or outgoing (`out`) curve from progress `from` to 1. */
export function fadeCurve(kind: 'in' | 'out', from = 0, steps = 64): Float32Array {
  const curve = new Float32Array(steps)
  for (let i = 0; i < steps; i++) {
    curve[i] = fadeGains(from + ((1 - from) * i) / (steps - 1))[kind]
  }
  return curve
}

// ---------------------------------------------------------------------------------- graph

/** What the output needs from Web Audio; faked in tests. */
export interface FadeGraph {
  /** Routes the element through its own gain node. False when that is not possible. */
  attach(el: HTMLAudioElement): boolean
  isAttached(el: HTMLAudioElement): boolean
  /** Seconds on the audio clock. */
  now(): number
  resume(): Promise<void>
  /** Fixes the gain of an element (cancels any ramp in progress). */
  setGain(el: HTMLAudioElement, gain: number): void
  /** Ramps both gains with equal-power curves from progress `from` to 1 over `seconds`. */
  ramp(outEl: HTMLAudioElement, inEl: HTMLAudioElement, from: number, seconds: number): void
}

/** Web Audio graph: each element → its GainNode → one shared destination. Null if unsupported. */
export function createWebAudioGraph(): FadeGraph | null {
  if (typeof AudioContext === 'undefined') return null
  let ctx: AudioContext | null = null
  const gains = new Map<HTMLAudioElement, GainNode>()
  const gainOf = (el: HTMLAudioElement) => gains.get(el)
  return {
    attach(el) {
      if (gains.has(el)) return true
      try {
        ctx ??= new AudioContext()
        // Cross-origin streams only produce sound through a media element source with CORS.
        el.crossOrigin = 'anonymous'
        const gain = ctx.createGain()
        ctx.createMediaElementSource(el).connect(gain)
        gain.connect(ctx.destination)
        gains.set(el, gain)
        return true
      } catch {
        return false
      }
    },
    isAttached: (el) => gains.has(el),
    now: () => ctx?.currentTime ?? 0,
    resume: async () => {
      if (ctx && ctx.state === 'suspended') await ctx.resume().catch(() => {})
    },
    setGain(el, gain) {
      const node = gainOf(el)
      if (!node) return
      node.gain.cancelScheduledValues(0)
      node.gain.value = gain
    },
    ramp(outEl, inEl, from, seconds) {
      const out = gainOf(outEl)
      const inn = gainOf(inEl)
      if (!ctx || !out || !inn) return
      const at = ctx.currentTime
      out.gain.cancelScheduledValues(0)
      inn.gain.cancelScheduledValues(0)
      out.gain.setValueCurveAtTime(fadeCurve('out', from), at, seconds)
      inn.gain.setValueCurveAtTime(fadeCurve('in', from), at, seconds)
    },
  }
}
