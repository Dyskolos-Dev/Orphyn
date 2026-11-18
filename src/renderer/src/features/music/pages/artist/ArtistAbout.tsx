import clsx from 'clsx'
import { useLayoutEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, Skeleton, Text } from '@renderer/design'
import type { Bio } from '../../library/artistBio'

interface Props {
  bio: Bio | undefined
  loading: boolean
}

/** "À propos": the biography clamped to 4 lines with a "More" toggle, its source shown discreetly. */
export function ArtistAbout({ bio, loading }: Props) {
  const { t } = useTranslation('music')
  const [open, setOpen] = useState(false)
  const [clamped, setClamped] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const text = bio?.text ?? ''
  useLayoutEffect(() => {
    const el = ref.current
    if (el && !open) setClamped(el.scrollHeight > el.clientHeight + 1)
  }, [text, open])

  if (loading) {
    return (
      <section className="jrm-section jrm-artist__section" aria-busy="true">
        <Text variant="title-2" className="jrm-section__title">
          {t('artist.about')}
        </Text>
        <Skeleton shape="text" lines={4} />
      </section>
    )
  }
  if (!bio || !text) return null

  return (
    <section className="jrm-section jrm-artist__section">
      <Text variant="title-2" className="jrm-section__title">
        {t('artist.about')}
      </Text>
      <div className="jrm-artist__bio">
        <div
          ref={ref}
          className={clsx('jrm-artist__bio-text', !open && 'jrm-artist__bio-text--clamped')}
        >
          <Text variant="callout" color="secondary">
            {text}
          </Text>
        </div>
        {(clamped || open) && (
          <Button
            variant="plain"
            size="small"
            onClick={() => setOpen((v) => !v)}
            aria-expanded={open}
          >
            {open ? t('artist.less') : t('artist.more')}
          </Button>
        )}
        {bio.source !== 'jellyfin' && (
          <Text variant="caption" color="tertiary" className="jrm-artist__source">
            {t(`artist.source.${bio.source}`)}
          </Text>
        )}
      </div>
    </section>
  )
}
