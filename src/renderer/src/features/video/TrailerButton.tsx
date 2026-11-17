import { AnimatePresence, motion } from 'framer-motion'
import { Clapperboard, ExternalLink, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { IconButton, MenuButton, springs, type MenuItem } from '@renderer/design'
import { useLocalTrailers, type BaseItemDto } from '@renderer/jellyfin'
import { trailerPath } from './cards'
import { pickTrailerSources, type TrailerSource } from './trailers'

type YouTubeSource = Extract<TrailerSource, { kind: 'youtube' }>

const watchUrl = (s: YouTubeSource) => `https://www.youtube.com/watch?v=${s.videoId}`

/** Opens a web page in the default browser. */
const openExternal = (url: string) => window.open(url, '_blank', 'noopener,noreferrer')

/** Cinematic in-app player for a YouTube trailer: blurred dark backdrop, 16:9 frame, Esc to close. */
export function TrailerModal({
  source,
  title,
  onClose,
}: {
  source: YouTubeSource | null
  title: string
  onClose(): void
}) {
  const { t } = useTranslation('video')
  const closeRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (!source) return
    const previous = document.activeElement as HTMLElement | null
    closeRef.current?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      e.stopPropagation()
      onClose()
    }
    // The page's CSP may not allow framing YouTube (`frame-src`): then the trailer opens in the
    // default browser instead of showing an empty frame.
    const onBlocked = (e: SecurityPolicyViolationEvent) => {
      if (!e.blockedURI.includes('youtube')) return
      openExternal(watchUrl(source))
      onClose()
    }
    window.addEventListener('keydown', onKey, true)
    document.addEventListener('securitypolicyviolation', onBlocked)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      document.removeEventListener('securitypolicyviolation', onBlocked)
      previous?.focus?.()
    }
  }, [source, onClose])

  return createPortal(
    <AnimatePresence>
      {source && (
        <div className="jr-trailer-layer" data-theme="dark">
          <motion.div
            className="jr-trailer-backdrop"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.25 }}
            onClick={onClose}
            aria-hidden="true"
          />
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label={t('trailer.dialog', { title })}
            className="jr-trailer"
            initial={{ opacity: 0, scale: 0.94, y: 16 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 8, transition: { duration: 0.16 } }}
            transition={springs.smooth}
          >
            <div className="jr-trailer__bar">
              <span className="jr-trailer__title">{source.name || t('item.trailer')}</span>
              <IconButton
                variant="glass"
                size="small"
                icon={<ExternalLink />}
                label={t('trailer.openInBrowser')}
                onClick={() => openExternal(watchUrl(source))}
              />
              <IconButton
                ref={closeRef}
                variant="glass"
                size="small"
                icon={<X strokeWidth={2.5} />}
                label={t('trailer.close')}
                onClick={onClose}
              />
            </div>
            <div className="jr-trailer__frame">
              <iframe
                src={source.embedUrl}
                title={t('trailer.dialog', { title })}
                allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
                allowFullScreen
                referrerPolicy="strict-origin-when-cross-origin"
              />
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>,
    document.body,
  )
}

/**
 * "Trailer" button of the detail page. Nothing when the item has no trailer; one trailer plays
 * straight away; several open a small menu. Server files play in Orphyn's player, YouTube links
 * in the modal, any other link in the default browser.
 */
export function TrailerButton({ item }: { item: BaseItemDto }) {
  const { t } = useTranslation('video')
  const navigate = useNavigate()
  const local = useLocalTrailers(item.Id, item.LocalTrailerCount)
  const [open, setOpen] = useState<YouTubeSource | null>(null)
  const sources = pickTrailerSources(local.data, item.RemoteTrailers)
  if (sources.length === 0) return null

  const play = (s: TrailerSource) => {
    if (s.kind === 'local') navigate(trailerPath(s.itemId))
    else if (s.kind === 'youtube') setOpen(s)
    else openExternal(s.url)
  }
  const label = (s: TrailerSource, i: number) =>
    s.name && sources.filter((x) => x.name === s.name).length === 1
      ? s.name
      : `${t('item.trailer')} ${i + 1}`
  const items: MenuItem[] = sources.map((s, i) => ({
    label: label(s, i),
    icon: <Clapperboard />,
    onSelect: () => play(s),
  }))

  return (
    <>
      {sources.length === 1 ? (
        <IconButton
          variant="glass"
          size="large"
          icon={<Clapperboard />}
          label={t('item.trailer')}
          onClick={() => play(sources[0] as TrailerSource)}
        />
      ) : (
        <MenuButton
          variant="glass"
          size="large"
          icon={<Clapperboard />}
          label={t('item.trailer')}
          items={items}
          placement="bottom-start"
        />
      )}
      <TrailerModal source={open} title={item.Name ?? ''} onClose={() => setOpen(null)} />
    </>
  )
}
