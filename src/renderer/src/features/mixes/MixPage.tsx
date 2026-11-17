import { ListPlus, Play, Shuffle, Sparkles } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { Button, EmptyState, Spinner, Text } from '@renderer/design'
import { ShellPage } from '@renderer/shell'
import { useFormatTotal } from '../music/components/format'
import { SongRows } from '../music/components/TrackRows'
import { musicPlayer } from '../music/player/api'
import { trackDuration } from '../music/player/track'
import { usePlaylistToasts } from '../music/playlists/actions'
import { MixCover } from './MixCover'
import { useMixLabels, useMixes } from './useMixes'
import './mixes.css'

/** A mix, laid out like a playlist: cover, Play / Shuffle / Save as playlist, then the songs. */
export function MixPage({ id }: { id: string }) {
  const { t } = useTranslation('mixes')
  const { t: tm } = useTranslation('music')
  const navigate = useNavigate()
  const mixes = useMixes()
  const labels = useMixLabels()
  const formatTotal = useFormatTotal()
  const { createWith } = usePlaylistToasts()

  if (mixes.isLoading) {
    return (
      <ShellPage title="" header="inline">
        <div className="jrm-center">
          <Spinner size="large" />
        </div>
      </ShellPage>
    )
  }
  const mix = mixes.data?.find((m) => m.id === id)
  if (!mix) {
    return (
      <ShellPage title={t('title')} header="inline">
        <EmptyState icon={<Sparkles />} title={t('notFound')} />
      </ShellPage>
    )
  }
  const { name, subtitle } = labels(mix)
  const total = mix.tracks.reduce((sum, s) => sum + trackDuration(s), 0)

  const save = async () => {
    const created = await createWith(name, mix.tracks.map((x) => x.Id ?? '').filter(Boolean))
    if (created) void navigate(`/music/playlist/${created}`)
  }

  return (
    <ShellPage title={name} header="inline" gutter>
      <header className="jrx-detail">
        <div className="jrx-detail__art">
          <MixCover mix={mix} name={name} />
        </div>
        <div className="jrx-detail__info">
          <Text variant="title-1" data-page-title="">
            {name}
          </Text>
          <Text variant="title-3" color="secondary">
            {subtitle}
          </Text>
          <Text variant="footnote" color="secondary">
            {`${tm('album.footer', { count: mix.tracks.length })} · ${formatTotal(total)}`}
          </Text>
          <div className="jrx-detail__actions">
            <Button
              icon={<Play fill="currentColor" />}
              onClick={() => musicPlayer.play(mix.tracks, 0, { shuffle: false })}
            >
              {tm('actions.play')}
            </Button>
            <Button
              variant="tinted"
              icon={<Shuffle />}
              onClick={() => musicPlayer.play(mix.tracks, undefined, { shuffle: true })}
            >
              {tm('actions.shuffle')}
            </Button>
            <Button variant="tinted" icon={<ListPlus />} onClick={() => void save()}>
              {t('saveAsPlaylist')}
            </Button>
          </div>
        </div>
      </header>
      <SongRows tracks={mix.tracks} lead="artwork" showAlbum aria-label={name} />
    </ShellPage>
  )
}
