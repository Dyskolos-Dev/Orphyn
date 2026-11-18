import { useTranslation } from 'react-i18next'
import { Shelf, SquareCard } from '@renderer/design'
import { RowFrame, countOf, type HomeRow } from '@renderer/features/home'
import { musicPlayer } from '../music/player/api'
import { MixCover } from './MixCover'
import { useMixLabels, useMixes } from './useMixes'
import type { Mix, MixKind } from './types'

export const mixPath = (id: string): string => `/music?mix=${encodeURIComponent(id)}`

export function MixCard({ mix }: { mix: Mix }) {
  const labels = useMixLabels()
  const { name, subtitle } = labels(mix)
  return (
    <SquareCard
      title={name}
      subtitle={subtitle}
      fallback={<MixCover mix={mix} name={name} />}
      to={mixPath(mix.id)}
      onPlay={() => musicPlayer.play(mix.tracks, 0, { shuffle: false })}
    />
  )
}

/** Home row types of the mixes and the kinds of mix each one shows. */
export const MIX_ROWS: Record<string, readonly MixKind[]> = {
  mixForYou: ['forYou'],
  mixDaily: ['discovery', 'onRepeat', 'rediscover', 'topYear'],
  mixGenres: ['genre'],
}

/** A shelf of the music home made of mixes. Nothing is drawn while there are none. */
export function MixRow({ row }: { row: HomeRow }) {
  const { t } = useTranslation('mixes')
  const mixes = useMixes()
  const kinds = MIX_ROWS[row.type] ?? []
  const shown = (mixes.data ?? [])
    .filter((m) => kinds.includes(m.kind))
    .slice(0, countOf(row, 'music'))
  if (shown.length === 0) return null
  return (
    <RowFrame size={row.size}>
      <Shelf title={t(`rows.${row.type}.title`)} kind="square">
        {shown.map((m) => (
          <MixCard key={m.id} mix={m} />
        ))}
      </Shelf>
    </RowFrame>
  )
}
