import {
  ChevronLeft,
  FastForward,
  Languages,
  Maximize,
  Minimize,
  Pause,
  Play,
  RotateCcw,
  RotateCw,
  Subtitles,
  Volume1,
  Volume2,
  VolumeX,
} from 'lucide-react'
import { useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { IconButton, MenuButton, Popover, Slider, Text, type MenuItem } from '@renderer/design'
import type { AudioTrack, SubtitleTrack } from '@renderer/jellyfin'
import type { PlaybackSnapshot } from '../controller'
import { formatClock } from '../format'

export const PLAYBACK_RATES = [0.5, 0.75, 1, 1.25, 1.5, 2]

interface TrackListProps {
  children: ReactNode
}

function TrackPopover({
  label,
  icon,
  active,
  children,
  onOpenChange,
}: {
  label: string
  icon: ReactNode
  active?: boolean
  children: (close: () => void) => ReactNode
  onOpenChange(open: boolean): void
}) {
  const anchor = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const set = (v: boolean) => {
    setOpen(v)
    onOpenChange(v)
  }
  return (
    <>
      <IconButton
        ref={anchor}
        variant="glass"
        icon={icon}
        label={label}
        pressed={active}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => set(!open)}
      />
      <Popover
        open={open}
        onClose={() => set(false)}
        anchor={anchor}
        placement="top-end"
        aria-label={label}
        width={260}
      >
        <div className="jr-video-tracklist" role="radiogroup" aria-label={label}>
          {children(() => set(false))}
        </div>
      </Popover>
    </>
  )
}

function TrackOption({
  label,
  detail,
  selected,
  onSelect,
}: {
  label: string
  detail?: string
  selected: boolean
  onSelect(): void
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      className="jr-video-tracklist__option"
      onClick={onSelect}
    >
      <span className="jr-video-tracklist__check" aria-hidden="true">
        {selected ? '✓' : ''}
      </span>
      <span className="jr-video-tracklist__label">
        {label}
        {detail && <small>{detail}</small>}
      </span>
    </button>
  )
}

export type { TrackListProps }

export interface PlayerControlsProps {
  snapshot: PlaybackSnapshot
  title: string
  subtitle?: string
  isFullscreen: boolean
  onBack(): void
  onTogglePlay(): void
  onSeekBy(delta: number): void
  onSeek(seconds: number): void
  onVolume(volume: number): void
  onToggleMute(): void
  onRate(rate: number): void
  onToggleFullscreen(): void
  onSelectAudio(track: AudioTrack): void
  onSelectSubtitle(track: SubtitleTrack | null): void
  /** Called when a popover opens/closes so the controls do not auto-hide under it. */
  onHoldChange(held: boolean): void
  /** Playing a trailer: shows a "Trailer" badge. */
  trailer?: boolean
  /** "Skip intros automatically" state; undefined hides the toggle (nothing to skip here). */
  skipIntroAuto?: boolean
  onToggleSkipIntroAuto?(): void
  /** Cast button (from the cast feature), shown beside the track menus. */
  castButton?: ReactNode
  /** Picture in Picture toggle (hidden when the webview cannot do it). */
  pipButton?: ReactNode
}

/** Overlay: top bar, centre transport, scrubber and bottom row. */
export function PlayerControls({
  snapshot,
  title,
  subtitle,
  isFullscreen,
  onBack,
  onTogglePlay,
  onSeekBy,
  onSeek,
  onVolume,
  onToggleMute,
  onRate,
  onToggleFullscreen,
  onSelectAudio,
  onSelectSubtitle,
  onHoldChange,
  trailer,
  skipIntroAuto,
  onToggleSkipIntroAuto,
  castButton,
  pipButton,
}: PlayerControlsProps) {
  const { t } = useTranslation('video')
  const [scrub, setScrub] = useState<number | null>(null)
  const playing = snapshot.status === 'playing' || snapshot.status === 'loading'
  const duration = snapshot.durationSeconds
  const position = scrub ?? snapshot.positionSeconds
  const tracks = snapshot.tracks
  const VolumeIcon =
    snapshot.muted || snapshot.volume === 0 ? VolumeX : snapshot.volume < 0.5 ? Volume1 : Volume2

  const rateItems: MenuItem[] = PLAYBACK_RATES.map((r) => ({
    label: `${r}×`,
    checked: snapshot.rate === r,
    onSelect: () => onRate(r),
  }))

  return (
    <div className="jr-video-controls" data-theme="dark">
      <div className="jr-video-controls__top">
        <IconButton
          variant="glass"
          icon={<ChevronLeft />}
          label={t('player.back')}
          onClick={onBack}
        />
        <div className="jr-video-controls__title">
          <Text variant="headline" truncate>
            {title}
          </Text>
          {subtitle && (
            <Text variant="footnote" color="secondary" truncate>
              {subtitle}
            </Text>
          )}
        </div>
        {trailer && <span className="jr-video-trailer-badge">{t('player.trailerBadge')}</span>}
      </div>

      <div className="jr-video-controls__center">
        <IconButton
          variant="glass"
          size="large"
          icon={<RotateCcw />}
          label={t('player.rewind')}
          onClick={() => onSeekBy(-10)}
        />
        <IconButton
          variant="glass"
          size="large"
          className="jr-video-controls__play"
          icon={playing ? <Pause fill="currentColor" /> : <Play fill="currentColor" />}
          label={playing ? t('player.pause') : t('player.play')}
          onClick={onTogglePlay}
        />
        <IconButton
          variant="glass"
          size="large"
          icon={<RotateCw />}
          label={t('player.forward')}
          onClick={() => onSeekBy(10)}
        />
      </div>

      <div className="jr-video-controls__bottom">
        <div className="jr-video-controls__scrubber">
          <Text variant="footnote" tabular>
            {formatClock(position)}
          </Text>
          <Slider
            aria-label={t('player.seek')}
            onMedia
            value={position}
            max={duration || 1}
            step={1}
            buffered={snapshot.bufferedSeconds}
            disabled={duration <= 0}
            formatValue={formatClock}
            onChangeStart={() => setScrub(snapshot.positionSeconds)}
            onChange={setScrub}
            onChangeEnd={(v) => {
              setScrub(null)
              onSeek(v)
            }}
          />
          <Text variant="footnote" tabular>
            -{formatClock(Math.max(0, duration - position))}
          </Text>
        </div>
        <div className="jr-video-controls__row">
          <div className="jr-video-controls__volume">
            <IconButton
              variant="glass"
              icon={<VolumeIcon />}
              label={snapshot.muted ? t('player.unmute') : t('player.mute')}
              onClick={onToggleMute}
            />
            <Slider
              aria-label={t('player.volume')}
              variant="regular"
              onMedia
              value={snapshot.muted ? 0 : snapshot.volume}
              onChange={onVolume}
            />
          </div>
          <div className="jr-video-controls__right">
            <MenuButton
              variant="glass"
              label={t('player.speed')}
              icon={<Text variant="callout">{snapshot.rate}×</Text>}
              items={rateItems}
              placement="top-end"
            />
            {skipIntroAuto !== undefined && (
              <IconButton
                variant="glass"
                icon={<FastForward />}
                label={t('skip.auto')}
                pressed={skipIntroAuto}
                onClick={onToggleSkipIntroAuto}
              />
            )}
            {tracks && tracks.subtitles.length > 0 && (
              <TrackPopover
                label={t('player.subtitles')}
                icon={<Subtitles />}
                active={snapshot.subtitleIndex != null && snapshot.subtitleIndex >= 0}
                onOpenChange={onHoldChange}
              >
                {(close) => (
                  <>
                    <TrackOption
                      label={t('player.subtitlesOff')}
                      selected={snapshot.subtitleIndex == null || snapshot.subtitleIndex < 0}
                      onSelect={() => {
                        onSelectSubtitle(null)
                        close()
                      }}
                    />
                    {tracks.subtitles.map((s) => (
                      <TrackOption
                        key={s.index}
                        label={s.title}
                        selected={snapshot.subtitleIndex === s.index}
                        onSelect={() => {
                          onSelectSubtitle(s)
                          close()
                        }}
                      />
                    ))}
                  </>
                )}
              </TrackPopover>
            )}
            {tracks && tracks.audio.length > 1 && (
              <TrackPopover
                label={t('player.audio')}
                icon={<Languages />}
                onOpenChange={onHoldChange}
              >
                {(close) =>
                  tracks.audio.map((a) => (
                    <TrackOption
                      key={a.index}
                      label={a.title}
                      selected={snapshot.audioIndex === a.index}
                      onSelect={() => {
                        onSelectAudio(a)
                        close()
                      }}
                    />
                  ))
                }
              </TrackPopover>
            )}
            {castButton}
            {pipButton}
            <IconButton
              variant="glass"
              icon={isFullscreen ? <Minimize /> : <Maximize />}
              label={isFullscreen ? t('player.exitFullscreen') : t('player.fullscreen')}
              onClick={onToggleFullscreen}
            />
          </div>
        </div>
      </div>
    </div>
  )
}
