import { Info, Play } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { Button, HeroBanner } from '@renderer/design'
import { imageUrl, type BaseItemDto } from '@renderer/jellyfin'
import { formatRuntime, progressOf, remainingMinutes } from './format'
import { itemPath, playPath } from './cards'

const AUTO_ADVANCE_MS = 8000

/** Hero line: year · genres · runtime / seasons. */
function heroMeta(item: BaseItemDto): string {
  return [
    item.ProductionYear,
    item.Genres?.slice(0, 2).join(', '),
    item.Type === 'Series' ? undefined : formatRuntime(item.RunTimeTicks),
    item.OfficialRating,
  ]
    .filter(Boolean)
    .join(' · ')
}

/**
 * Apple TV "Watch Now" hero: a HeroBanner per featured item that advances by itself and
 * pauses while the pointer or keyboard focus is inside.
 */
export function HeroCarousel({ items }: { items: BaseItemDto[] }) {
  const { t } = useTranslation('video')
  const navigate = useNavigate()
  const [index, setIndex] = useState(0)
  const [paused, setPaused] = useState(false)
  const count = items.length
  const current = items[Math.min(index, Math.max(0, count - 1))]

  useEffect(() => {
    if (paused || count < 2) return
    const id = window.setTimeout(() => setIndex((i) => (i + 1) % count), AUTO_ADVANCE_MS)
    return () => window.clearTimeout(id)
  }, [paused, count, index])

  const go = useCallback((i: number) => setIndex(i), [])
  if (!current?.Id) return null
  const id = current.Id
  const progress = progressOf(current)
  const playable = current.Type === 'Movie' || current.Type === 'Episode'

  return (
    <div
      className="jr-video-carousel"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      <div className="jr-video-carousel__slide" key={id}>
        <HeroBanner
          backdrop={imageUrl(current, 'Backdrop', { maxWidth: 1920, quality: 80 })}
          logo={imageUrl(current, 'Logo', { maxWidth: 600 })}
          title={current.Name ?? ''}
          eyebrow={t('home.featured')}
          meta={heroMeta(current)}
          description={current.Overview}
          progress={progress}
          progressCaption={
            progress ? t('item.minutesLeft', { minutes: remainingMinutes(current) }) : undefined
          }
          actions={
            <>
              {playable && (
                <Button
                  variant="contrast"
                  size="large"
                  icon={<Play fill="currentColor" />}
                  onClick={() =>
                    navigate(playPath(id, current.UserData?.PlaybackPositionTicks ?? 0))
                  }
                >
                  {progress
                    ? t('item.resume', {
                        minutes: Math.max(
                          1,
                          Math.round((current.UserData?.PlaybackPositionTicks ?? 0) / 600_000_000),
                        ),
                      })
                    : t('item.play')}
                </Button>
              )}
              <Button
                variant={playable ? 'gray' : 'contrast'}
                size="large"
                icon={<Info />}
                onClick={() => navigate(itemPath(current))}
              >
                {t('item.more')}
              </Button>
            </>
          }
        />
      </div>
      {count > 1 && (
        <div className="jr-video-carousel__dots" role="group" aria-label={t('home.featured')}>
          {items.map((it, i) => (
            <button
              key={it.Id ?? i}
              type="button"
              className="jr-video-carousel__dot"
              aria-label={t('home.slide', { n: i + 1 })}
              aria-current={i === index}
              onClick={() => go(i)}
            />
          ))}
        </div>
      )}
    </div>
  )
}
