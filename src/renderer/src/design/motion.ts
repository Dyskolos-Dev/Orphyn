import type { Transition } from 'framer-motion'

/** framer-motion presets matching the CSS motion tokens (see tokens.css / docs/DESIGN.md). */
export const springs = {
  /** Default UI spring: quick, no overshoot (sheets, popovers, header). */
  smooth: { type: 'spring', stiffness: 300, damping: 30, mass: 1 },
  /** Snappy, for small controls (toggles, segmented control thumb). */
  snappy: { type: 'spring', stiffness: 500, damping: 38, mass: 0.8 },
  /** Slight overshoot, for things that pop in (toasts). */
  bouncy: { type: 'spring', stiffness: 400, damping: 22, mass: 0.9 },
} as const satisfies Record<string, Transition>

/** Durations in seconds, same values as the --duration-* tokens. */
export const durations = {
  instant: 0.1,
  fast: 0.16,
  base: 0.24,
  slow: 0.36,
  slower: 0.52,
} as const

/** Cubic-bezier easings, same values as the --ease-* tokens. */
export const easings = {
  standard: [0.25, 0.1, 0.25, 1],
  out: [0.22, 1, 0.36, 1],
  in: [0.4, 0, 1, 1],
  inOut: [0.65, 0, 0.35, 1],
} as const
