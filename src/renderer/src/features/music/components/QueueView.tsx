import { GripVertical, ListMusic, Repeat, Repeat1, Shuffle, X } from 'lucide-react'
import { useEffect, useRef, useState, type DragEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Artwork, Button, EmptyState, formatDuration, IconButton, Text } from '@renderer/design'
import { useMusicPlayer } from '../player/api'
import { trackArt, trackArtist, trackDuration, trackTitle } from '../player/track'

/** Queue with the current track highlighted, drag-and-drop reordering and removal. */
export function QueueView() {
  const { t } = useTranslation('music')
  const queue = useMusicPlayer((s) => s.queue)
  const index = useMusicPlayer((s) => s.index)
  const status = useMusicPlayer((s) => s.status)
  const shuffle = useMusicPlayer((s) => s.shuffle)
  const repeat = useMusicPlayer((s) => s.repeat)
  const [dragFrom, setDragFrom] = useState<number | null>(null)
  const [dropAt, setDropAt] = useState<number | null>(null)
  const currentRef = useRef<HTMLLIElement>(null)

  useEffect(() => {
    currentRef.current?.scrollIntoView({ block: 'start' })
    // Only when the view opens.
  }, [])

  if (queue.length === 0) {
    return (
      <div className="jrm-queue jrm-queue--empty">
        <EmptyState size="inline" icon={<ListMusic />} title={t('queue.empty')} />
      </div>
    )
  }

  const store = useMusicPlayer.getState
  const onDragOver = (e: DragEvent, i: number) => {
    if (dragFrom === null) return
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
    if (dropAt !== i) setDropAt(i)
  }
  const finishDrag = () => {
    setDragFrom(null)
    setDropAt(null)
  }

  return (
    <div className="jrm-queue">
      <div className="jrm-queue__header">
        <Text variant="title-3">{t('queue.title')}</Text>
        <span className="jrm-queue__tools">
          <IconButton
            icon={<Shuffle />}
            label={t('player.shuffle')}
            pressed={shuffle}
            size="small"
            onClick={() => store().toggleShuffle()}
          />
          <IconButton
            icon={repeat === 'one' ? <Repeat1 /> : <Repeat />}
            label={t(`player.repeat.${repeat}`)}
            pressed={repeat !== 'off'}
            size="small"
            onClick={() => store().cycleRepeat()}
          />
          <Button
            variant="plain"
            size="small"
            disabled={index >= queue.length - 1}
            onClick={() => store().clearUpcoming()}
          >
            {t('queue.clear')}
          </Button>
        </span>
      </div>
      <ol className="jrm-queue__list" aria-label={t('queue.title')}>
        {queue.map((item, i) => {
          const isCurrent = i === index
          return (
            <li
              key={item.uid}
              ref={isCurrent ? currentRef : undefined}
              className="jrm-queue__item"
              data-current={isCurrent || undefined}
              data-played={i < index || undefined}
              data-dragging={dragFrom === i || undefined}
              data-drop={
                dropAt === i && dragFrom !== i ? (dragFrom! < i ? 'after' : 'before') : undefined
              }
              draggable
              onDragStart={(e) => {
                e.dataTransfer.effectAllowed = 'move'
                e.dataTransfer.setData('text/plain', item.uid)
                setDragFrom(i)
              }}
              onDragOver={(e) => onDragOver(e, i)}
              onDrop={(e) => {
                e.preventDefault()
                if (dragFrom !== null) store().moveItem(dragFrom, i)
                finishDrag()
              }}
              onDragEnd={finishDrag}
            >
              <GripVertical className="jrm-queue__grip" aria-hidden="true" />
              <button
                type="button"
                className="jrm-queue__play"
                onClick={() => store().jumpTo(i)}
                aria-label={t('queue.play', { title: trackTitle(item.track) })}
                aria-current={isCurrent || undefined}
              >
                <Artwork
                  src={trackArt(item.track, 96)}
                  ratio="1/1"
                  shape="rounded"
                  className="jrm-queue__art"
                />
                <span className="jrm-queue__text">
                  <Text variant="subheadline" truncate className="jrm-queue__title">
                    {trackTitle(item.track)}
                  </Text>
                  <Text variant="footnote" color="secondary" truncate>
                    {trackArtist(item.track)}
                  </Text>
                </span>
              </button>
              <Text variant="footnote" color="secondary" tabular>
                {isCurrent && status === 'playing'
                  ? t('queue.playing')
                  : formatDuration(trackDuration(item.track))}
              </Text>
              <IconButton
                icon={<X />}
                label={t('queue.remove', { title: trackTitle(item.track) })}
                size="small"
                onClick={() => store().removeAt(i)}
              />
            </li>
          )
        })}
      </ol>
    </div>
  )
}
