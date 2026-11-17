import { AnimatePresence, motion } from 'framer-motion'
import { SkipForward } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { springs } from '@renderer/design'
import type { SkipKind } from '../skip'

const LABEL: Record<SkipKind, string> = {
  intro: 'skip.intro',
  recap: 'skip.recap',
  credits: 'skip.credits',
  nextEpisode: 'skip.nextEpisode',
}

/** Translucent pill at the bottom right: Skip Intro / Recap / Credits, or Next Episode. */
export function SkipPillButton({ kind, onSkip }: { kind: SkipKind | undefined; onSkip(): void }) {
  const { t } = useTranslation('video')
  return (
    <AnimatePresence>
      {kind && (
        <motion.button
          // A new kind (intro → credits) is a new pill: it springs in again.
          key={kind}
          type="button"
          className="jr-video-skip"
          data-theme="dark"
          initial={{ opacity: 0, y: 16, scale: 0.92 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 8, scale: 0.97, transition: { duration: 0.16 } }}
          transition={springs.bouncy}
          onClick={onSkip}
        >
          <span>{t(LABEL[kind])}</span>
          <SkipForward aria-hidden="true" fill="currentColor" />
          <kbd aria-hidden="true">S</kbd>
        </motion.button>
      )}
    </AnimatePresence>
  )
}
