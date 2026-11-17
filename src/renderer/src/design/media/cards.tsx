import clsx from 'clsx'
import { Ellipsis, Play } from 'lucide-react'
import type { CSSProperties, MouseEvent, ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { IconButton } from '../controls/IconButton'
import { Artwork, initials, type ArtworkRatio } from './Artwork'
import { useShelfLayout } from './shelfContext'
import './media.css'

/** Props shared by every card. */
export interface CardBaseProps {
  /** Main line; also the card's accessible name. */
  title: string
  /** Second line (year, artist, episode...). */
  subtitle?: ReactNode
  /** Artwork URL. */
  image?: string | null
  srcSet?: string
  /** Router path to open (hash router). Renders a link. */
  to?: string
  /** Click handler when there is no `to`. Renders a button. */
  onClick?(): void
  /** Right click / context-menu key. Wrap the card in <ContextMenu> for a full menu instead. */
  onContextMenu?(e: MouseEvent): void
  /** Overlay play button (shown on hover / focus). */
  onPlay?(): void
  /** Overlay "more" button, e.g. to open a <Menu>. Receives the click to anchor a menu. */
  onMore?(e: MouseEvent<HTMLButtonElement>): void
  /** Small overlay in the top-right corner (e.g. "4K", unplayed count). */
  badge?: ReactNode
  /** Icon or text when the artwork is missing. */
  fallback?: ReactNode
  /** Highlight as current selection / now playing. */
  selected?: boolean
  className?: string
  style?: CSSProperties
}

interface MediaCardProps extends CardBaseProps {
  kind: 'poster' | 'landscape' | 'square' | 'round'
  ratio: ArtworkRatio
  shape: 'rounded' | 'poster' | 'circle'
  hideText?: boolean
  artOverlay?: ReactNode
  textAlign?: 'start' | 'center'
}

function MediaCard({
  kind,
  ratio,
  shape,
  title,
  subtitle,
  image,
  srcSet,
  to,
  onClick,
  onContextMenu,
  onPlay,
  onMore,
  badge,
  fallback,
  selected,
  hideText,
  artOverlay,
  textAlign = 'start',
  className,
  style,
}: MediaCardProps) {
  const { t } = useTranslation('shell')
  const body = (
    <>
      <div className="jr-card__art">
        <Artwork
          src={image}
          srcSet={srcSet}
          ratio={ratio}
          shape={shape}
          shadow
          fallback={fallback}
          sizes="(max-width: 1200px) 25vw, 320px"
        />
        {artOverlay}
        {badge != null && <span className="jr-card__badge">{badge}</span>}
      </div>
      {!hideText && (
        <div className={clsx('jr-card__text', textAlign === 'center' && 'jr-card__text--center')}>
          <span className="jr-card__title">{title}</span>
          {subtitle != null && <span className="jr-card__subtitle">{subtitle}</span>}
        </div>
      )}
    </>
  )
  const hitProps = {
    className: 'jr-card__hit',
    onContextMenu,
    'aria-label': hideText ? title : undefined,
  }
  let hit: ReactNode
  if (to) {
    hit = (
      <Link to={to} {...hitProps} draggable={false}>
        {body}
      </Link>
    )
  } else if (onClick) {
    hit = (
      <button type="button" onClick={onClick} {...hitProps}>
        {body}
      </button>
    )
  } else {
    hit = (
      <div {...hitProps} aria-label={undefined}>
        {body}
      </div>
    )
  }
  return (
    <div
      className={clsx(
        'jr-card',
        `jr-card--${kind}`,
        selected && 'jr-card--selected',
        (onPlay || onMore) && 'jr-card--has-actions',
        className,
      )}
      style={{ ...style, '--jr-card-ratio': ratio.replace('/', ' / ') } as CSSProperties}
    >
      {hit}
      {(onPlay || onMore) && (
        <div className="jr-card__actions">
          {onPlay && (
            <IconButton
              className="jr-card__play"
              variant="glass"
              size="medium"
              icon={<Play fill="currentColor" />}
              label={t('design.playItem', { title })}
              onClick={onPlay}
            />
          )}
          {onMore && (
            <IconButton
              className="jr-card__more"
              variant="glass"
              size="small"
              icon={<Ellipsis />}
              label={t('design.moreFor', { title })}
              onClick={onMore}
            />
          )}
        </div>
      )}
    </div>
  )
}

export interface PosterCardProps extends CardBaseProps {
  /** 0..1 watched progress, drawn as a thin bar at the bottom of the poster. */
  progress?: number
  /** Hide the text under the poster (rows of posters). */
  hideText?: boolean
  /**
   * 16:9 artwork (thumb / backdrop) shown instead of the poster when the user prefers landscape
   * cards on video shelves (Settings > Appearance). Without it the poster is cropped to 16:9.
   */
  landscapeImage?: string | null
}

/** 2:3 movie / series poster (16:9 on shelves when the user prefers landscape video cards). */
export function PosterCard({ progress, hideText, landscapeImage, ...props }: PosterCardProps) {
  const { postersAsLandscape } = useShelfLayout()
  if (postersAsLandscape) {
    return (
      <MediaCard
        {...props}
        image={landscapeImage ?? props.image}
        srcSet={landscapeImage ? undefined : props.srcSet}
        kind="landscape"
        ratio="16/9"
        shape="poster"
        hideText={hideText}
        className={clsx(props.className, !landscapeImage && 'jr-card--cropped-poster')}
        artOverlay={progress != null && <CardProgress value={progress} />}
      />
    )
  }
  return (
    <MediaCard
      {...props}
      kind="poster"
      ratio="2/3"
      shape="poster"
      hideText={hideText}
      artOverlay={progress != null && <CardProgress value={progress} />}
    />
  )
}

export interface LandscapeCardProps extends CardBaseProps {
  /** 0..1 watched progress bar inside the artwork (Continue Watching). */
  progress?: number
  /** Text in the artwork's bottom-left corner, e.g. "24 min left". */
  caption?: ReactNode
  /** Third line, e.g. an episode synopsis. */
  description?: ReactNode
}

/** 16:9 episode / continue-watching card with optional progress bar. */
export function LandscapeCard({ progress, caption, description, ...props }: LandscapeCardProps) {
  return (
    <MediaCard
      {...props}
      kind="landscape"
      ratio="16/9"
      shape="poster"
      subtitle={
        description != null ? (
          <>
            {props.subtitle != null && <span className="jr-card__line">{props.subtitle}</span>}
            <span className="jr-card__description">{description}</span>
          </>
        ) : (
          props.subtitle
        )
      }
      artOverlay={
        (progress != null || caption != null) && (
          <div className="jr-card__landscape-footer">
            {caption != null && <span className="jr-card__caption">{caption}</span>}
            {progress != null && <CardProgress value={progress} inline />}
          </div>
        )
      }
    />
  )
}

export type SquareCardProps = CardBaseProps

/** 1:1 album / playlist artwork with hover play. */
export function SquareCard(props: SquareCardProps) {
  return <MediaCard {...props} kind="square" ratio="1/1" shape="rounded" />
}

export type RoundCardProps = Omit<CardBaseProps, 'onPlay' | 'badge'>

/** Circular artist / person card with centered name. Falls back to initials. */
export function RoundCard({ fallback, ...props }: RoundCardProps) {
  return (
    <MediaCard
      {...props}
      kind="round"
      ratio="1/1"
      shape="circle"
      textAlign="center"
      fallback={fallback ?? <span className="jr-card__initials">{initials(props.title)}</span>}
    />
  )
}

function CardProgress({ value, inline = false }: { value: number; inline?: boolean }) {
  const { t } = useTranslation('shell')
  const pct = Math.round(Math.min(1, Math.max(0, value)) * 100)
  return (
    <span
      className={clsx('jr-card__progress', inline && 'jr-card__progress--inline')}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={pct}
      aria-label={t('design.progress', { percent: pct })}
    >
      <span className="jr-card__progress-fill" style={{ width: `${pct}%` }} />
    </span>
  )
}
