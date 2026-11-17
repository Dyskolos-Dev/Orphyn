import { Radio, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { stopRadio } from './radioPlayer'
import { useRadio } from './radio'
import './mixes.css'

/** "Radio · <seed>" pill with a stop button, shown by the queue while a radio is running. */
export function RadioIndicator() {
  const { t } = useTranslation('mixes')
  const seed = useRadio((s) => s.seed)
  if (!seed) return null
  return (
    <span className="jrx-radio" role="status">
      <Radio size={12} aria-hidden="true" />
      {t('radio.indicator', { name: seed.name })}
      <button type="button" className="jrm-heart" aria-label={t('radio.stop')} onClick={stopRadio}>
        <X size={12} aria-hidden="true" />
      </button>
    </span>
  )
}
