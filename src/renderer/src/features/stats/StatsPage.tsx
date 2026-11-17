import { animate, motion, useInView, useReducedMotion } from 'framer-motion'
import { Clapperboard, Disc3, Music, Sparkles, Tv } from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState, SegmentedControl, Skeleton } from '@renderer/design'
import { imageUrl } from '@renderer/jellyfin'
import type { BaseItemDto } from '@renderer/jellyfin/types'
import { ShellPage } from '@renderer/shell'
import type { GenreEntry, RankedEntry, YearStats } from './aggregate'
import { useYearInReview } from './useStats'
import './stats.css'

/** Fades and lifts its children in when scrolled into view; static with reduced motion. */
function Reveal({
  children,
  delay = 0,
  className,
}: {
  children: ReactNode
  delay?: number
  className?: string
}) {
  const reduced = useReducedMotion()
  if (reduced) return <div className={className}>{children}</div>
  return (
    <motion.div
      className={className}
      initial={{ opacity: 0, y: 24 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true, margin: '0px 0px -10% 0px' }}
      transition={{ duration: 0.6, delay, ease: [0.22, 1, 0.36, 1] }}
    >
      {children}
    </motion.div>
  )
}

/** Number that counts up the first time it is seen. */
function CountUp({ value }: { value: number }) {
  const ref = useRef<HTMLSpanElement>(null)
  const inView = useInView(ref, { once: true })
  const reduced = useReducedMotion()
  const { i18n } = useTranslation()
  const [shown, setShown] = useState(reduced ? value : 0)
  useEffect(() => {
    if (reduced) {
      setShown(value)
      return
    }
    if (!inView) return
    const controls = animate(0, value, {
      duration: 1.4,
      ease: [0.22, 1, 0.36, 1],
      onUpdate: (v) => setShown(Math.round(v)),
    })
    return () => controls.stop()
  }, [inView, value, reduced])
  return (
    <span ref={ref} className="jrs-num">
      {new Intl.NumberFormat(i18n.language).format(shown)}
    </span>
  )
}

function Art({ item, size, round }: { item: BaseItemDto; size: number; round?: boolean }) {
  const src = imageUrl(item, 'Primary', { maxWidth: size * 2, maxHeight: size * 2, quality: 92 })
  return (
    <div className={round ? 'jrs-art jrs-art--round' : 'jrs-art'}>
      {src ? <img src={src} alt="" loading="lazy" decoding="async" /> : <Music aria-hidden />}
    </div>
  )
}

function Section({
  icon,
  title,
  children,
}: {
  icon: ReactNode
  title: string
  children: ReactNode
}) {
  return (
    <section className="jrs-section">
      <Reveal>
        <h2 className="jrs-section__title">
          {icon}
          <span>{title}</span>
        </h2>
      </Reveal>
      {children}
    </section>
  )
}

function Ranking({ entries }: { entries: RankedEntry[] }) {
  const { t } = useTranslation('stats')
  return (
    <ol className="jrs-rank">
      {entries.map((e, i) => (
        <Reveal key={e.key} delay={i * 0.06}>
          <li className="jrs-rank__row">
            <span className="jrs-rank__n">{i + 1}</span>
            <Art item={e.item} size={56} />
            <span className="jrs-rank__text">
              <span className="jrs-rank__name">{e.name}</span>
              {e.subtitle && <span className="jrs-rank__sub">{e.subtitle}</span>}
            </span>
            <span className="jrs-rank__plays">{t('plays', { count: e.plays })}</span>
          </li>
        </Reveal>
      ))}
    </ol>
  )
}

function Posters({ entries }: { entries: RankedEntry[] }) {
  return (
    <div className="jrs-posters">
      {entries.map((e, i) => (
        <Reveal key={e.key} delay={i * 0.07}>
          <figure className="jrs-poster">
            <Art item={e.item} size={240} />
            <figcaption>
              <span className="jrs-rank__name">{e.name}</span>
              {e.subtitle && <span className="jrs-rank__sub">{e.subtitle}</span>}
            </figcaption>
          </figure>
        </Reveal>
      ))}
    </div>
  )
}

function Genres({ genres }: { genres: GenreEntry[] }) {
  const reduced = useReducedMotion()
  const max = genres[0]?.share || 1
  return (
    <ul className="jrs-genres">
      {genres.map((g, i) => (
        <li key={g.name}>
          <span className="jrs-genres__name">{g.name}</span>
          <span className="jrs-genres__track">
            <motion.span
              className="jrs-genres__bar"
              initial={reduced ? false : { scaleX: 0 }}
              whileInView={{ scaleX: g.share / max }}
              viewport={{ once: true }}
              transition={{ duration: 0.9, delay: i * 0.08, ease: [0.22, 1, 0.36, 1] }}
              style={reduced ? { transform: `scaleX(${g.share / max})` } : undefined}
            />
          </span>
          <span className="jrs-genres__pct">{Math.round(g.share * 100)}%</span>
        </li>
      ))}
    </ul>
  )
}

function Loading() {
  return (
    <div className="jrs" aria-busy="true">
      <Skeleton height={220} radius="var(--radius-2xl)" />
      <Skeleton height={28} width="40%" />
      {Array.from({ length: 4 }, (_, i) => (
        <Skeleton key={i} height={64} radius="var(--radius-lg)" />
      ))}
    </div>
  )
}

function Story({ stats, exact }: { stats: YearStats; exact: boolean }) {
  const { t, i18n } = useTranslation('stats')
  const { music, video, facts } = stats
  const topArtist = music.artists[0]
  const hasVideo = video.moviePlays + video.episodePlays > 0
  const month =
    facts.busiestMonth === null
      ? null
      : new Intl.DateTimeFormat(i18n.language, { month: 'long' }).format(
          new Date(stats.year, facts.busiestMonth, 1),
        )
  const hours = Math.round(facts.totalMinutes / 60)

  return (
    <div className="jrs">
      <Reveal className="jrs-hero">
        <p className="jrs-hero__eyebrow">{t('hero.eyebrow', { year: stats.year })}</p>
        <p className="jrs-hero__big">
          <CountUp value={facts.totalMinutes} />
          <span className="jrs-hero__unit">{t('hero.minutes')}</span>
        </p>
        <p className="jrs-hero__line">
          {t('hero.line', { hours: hours.toLocaleString(i18n.language) })}
        </p>
      </Reveal>

      {topArtist && (
        <Section icon={<Disc3 aria-hidden />} title={t('music.title')}>
          <Reveal className="jrs-spotlight">
            <Art item={topArtist.item} size={360} round />
            <div>
              <p className="jrs-hero__eyebrow">{t('music.topArtist')}</p>
              <p className="jrs-spotlight__name">{topArtist.name}</p>
              <p className="jrs-rank__sub">
                {t('music.topArtistLine', { count: topArtist.plays })}
              </p>
            </div>
          </Reveal>
          <div className="jrs-grid">
            <div>
              <h3 className="jrs-sub">{t('music.artists')}</h3>
              <Ranking entries={music.artists} />
            </div>
            <div>
              <h3 className="jrs-sub">{t('music.songs')}</h3>
              <Ranking entries={music.songs} />
            </div>
          </div>
          <h3 className="jrs-sub">{t('music.albums')}</h3>
          <Posters entries={music.albums} />
          {music.genres.length > 0 && (
            <>
              <h3 className="jrs-sub">{t('music.genres')}</h3>
              <Genres genres={music.genres} />
            </>
          )}
        </Section>
      )}

      {hasVideo && (
        <Section icon={<Clapperboard aria-hidden />} title={t('video.title')}>
          <Reveal className="jrs-stats">
            <div>
              <CountUp value={video.moviePlays} />
              <span>{t('video.movies', { count: video.moviePlays })}</span>
            </div>
            <div>
              <CountUp value={video.episodePlays} />
              <span>{t('video.episodes', { count: video.episodePlays })}</span>
            </div>
          </Reveal>
          {video.movies.length > 0 && (
            <>
              <h3 className="jrs-sub">{t('video.topMovies')}</h3>
              <Posters entries={video.movies} />
            </>
          )}
          {video.series.length > 0 && (
            <>
              <h3 className="jrs-sub">
                <Tv aria-hidden /> {t('video.topSeries')}
              </h3>
              <Posters entries={video.series} />
            </>
          )}
        </Section>
      )}

      <Section icon={<Sparkles aria-hidden />} title={t('facts.title')}>
        <div className="jrs-facts">
          {month && (
            <Reveal className="jrs-fact">
              <strong>{month}</strong>
              <span>{t('facts.busiestMonth')}</span>
            </Reveal>
          )}
          {music.plays > 0 && (
            <Reveal className="jrs-fact" delay={0.06}>
              <strong>{Math.round(facts.topArtistShare * 100)}%</strong>
              <span>{t('facts.artistShare', { artist: topArtist?.name ?? '' })}</span>
            </Reveal>
          )}
          <Reveal className="jrs-fact" delay={0.12}>
            <strong>{Math.max(1, Math.round(facts.minutesPerDay))}</strong>
            <span>{t('facts.perDay')}</span>
          </Reveal>
          {music.distinctSongs > 0 && (
            <Reveal className="jrs-fact" delay={0.18}>
              <strong>{music.distinctSongs.toLocaleString(i18n.language)}</strong>
              <span>{t('facts.distinctSongs')}</span>
            </Reveal>
          )}
        </div>
        {!exact && <p className="jrs-note">{t('approx')}</p>}
      </Section>
    </div>
  )
}

export function StatsPage() {
  const { t } = useTranslation('stats')
  const [picked, setPicked] = useState<number | null>(null)
  const year = picked ?? new Date().getFullYear()
  const { isLoading, isError, years, stats, exact } = useYearInReview(year)

  const picker =
    years.length > 1 ? (
      <SegmentedControl
        aria-label={t('year')}
        size="small"
        value={String(year)}
        onChange={(v) => setPicked(Number(v))}
        options={years.slice(0, 5).map((y) => ({ value: String(y), label: String(y) }))}
      />
    ) : undefined

  return (
    <ShellPage title={t('title')} subtitle={t('subtitle')} headerAccessory={picker}>
      {isLoading ? (
        <Loading />
      ) : isError || !stats ? (
        <EmptyState icon={<Music />} title={t('error.title')} message={t('error.message')} />
      ) : stats.isEmpty ? (
        <EmptyState
          icon={<Sparkles />}
          title={t('empty.title', { year })}
          message={t('empty.message')}
        />
      ) : (
        <Story stats={stats} exact={exact} />
      )}
    </ShellPage>
  )
}
