import { motion } from 'framer-motion'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { BrandLogo, durations, easings, springs } from '@renderer/design'
import { ShellImmersive } from '@renderer/shell'
import { ConnectFlow } from './ConnectFlow'
import './connect.css'

const WORD_DELAY = 0.12

/** First-launch screen: an aurora backdrop, the logo springing in, then the server setup. */
export function OnboardingPage() {
  const { t } = useTranslation('servers')
  const navigate = useNavigate()
  const words = t('welcome.title').split(' ')
  return (
    <div className="jr-onboarding">
      <ShellImmersive />
      <div className="jr-aurora" aria-hidden="true">
        <span className="jr-aurora__blob jr-aurora__blob--indigo" />
        <span className="jr-aurora__blob jr-aurora__blob--cyan" />
        <span className="jr-aurora__blob jr-aurora__blob--lilac" />
      </div>
      <div className="jr-onboarding__content">
        <motion.div
          className="jr-onboarding__brand"
          initial={{ opacity: 0, scale: 0.6 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ ...springs.bouncy, delay: 0.1 }}
        >
          <span className="jr-onboarding__glow" />
          <BrandLogo size={96} className="jr-onboarding__logo" />
        </motion.div>
        <h1 className="jr-onboarding__welcome" aria-label={t('welcome.title')}>
          {words.map((word, i) => (
            <motion.span
              key={`${word}-${i}`}
              className="jr-onboarding__word"
              aria-hidden="true"
              initial={{ opacity: 0, y: 14 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{
                duration: durations.slower,
                ease: easings.out,
                delay: 0.45 + i * WORD_DELAY,
              }}
            >
              {word}
            </motion.span>
          ))}
        </h1>
        <motion.div
          className="jr-onboarding__flow"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{
            duration: durations.slower,
            ease: easings.out,
            delay: 0.45 + words.length * WORD_DELAY,
          }}
        >
          <ConnectFlow onDone={() => void navigate('/video', { replace: true })} />
        </motion.div>
      </div>
    </div>
  )
}
