import clsx from 'clsx'
import { AnimatePresence, Reorder, motion, useDragControls } from 'framer-motion'
import {
  ChevronLeft,
  Eye,
  EyeOff,
  GripVertical,
  Pencil,
  Plus,
  RotateCcw,
  SlidersHorizontal,
  Trash2,
} from 'lucide-react'
import { useState, type KeyboardEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, IconButton, SearchField, SegmentedControl, Sheet, springs } from '@renderer/design'
import { flattenPages, useAlbumArtists, useCollections, useGenres } from '@renderer/jellyfin'
import {
  ROW_DEFS,
  SIZE_OPTIONS,
  countChoices,
  countOf,
  defOf,
  hasRow,
  type HomeLayout,
  type HomeRow,
  type HomeScope,
  type RowDef,
  type RowSize,
  type RowSpec,
} from './layout'
import { rowDescription, rowIcon, rowName, rowTitle } from './rowMeta'
import type { HomeEditor } from './useHomeLayout'
import './home.css'

// ---------------------------------------------------------------- toolbar

/** Toolbar buttons of a home: "Edit", or "Add a row / Reset / Cancel / Done" while editing. */
export function HomeEditActions({ editor, onAdd }: { editor: HomeEditor; onAdd(): void }) {
  const { t } = useTranslation('home')
  if (!editor.editing) {
    return (
      <Button variant="plain" size="small" icon={<Pencil />} onClick={editor.start}>
        {t('edit')}
      </Button>
    )
  }
  return (
    <>
      <Button variant="plain" size="small" icon={<Plus />} onClick={onAdd}>
        {t('addRow')}
      </Button>
      <Button variant="plain" size="small" icon={<RotateCcw />} onClick={editor.reset}>
        {t('reset')}
      </Button>
      <Button variant="plain" size="small" onClick={editor.cancel}>
        {t('cancel')}
      </Button>
      <Button variant="filled" size="small" onClick={editor.done}>
        {t('done')}
      </Button>
    </>
  )
}

// ---------------------------------------------------------------- the list

/** The rows as draggable cards: handle, name, options, hide / show, remove. */
export function HomeEditList({ scope, editor }: { scope: HomeScope; editor: HomeEditor }) {
  const { t } = useTranslation('home')
  const rows = editor.layout.rows
  return (
    <div className="jr-home-edit">
      <p className="jr-home-edit__hint">{t('hint')}</p>
      {rows.length === 0 ? (
        <p className="jr-home-edit__empty">{t('empty')}</p>
      ) : (
        <Reorder.Group
          as="ul"
          axis="y"
          values={rows}
          onReorder={(next: HomeRow[]) => editor.reorder(next.map((r) => r.id))}
          className="jr-home-edit__list"
        >
          {rows.map((row, i) => (
            <EditRow
              key={row.id}
              scope={scope}
              row={row}
              index={i}
              last={i === rows.length - 1}
              editor={editor}
            />
          ))}
        </Reorder.Group>
      )}
    </div>
  )
}

function EditRow({
  scope,
  row,
  index,
  last,
  editor,
}: {
  scope: HomeScope
  row: HomeRow
  index: number
  last: boolean
  editor: HomeEditor
}) {
  const { t } = useTranslation('home')
  const controls = useDragControls()
  const [open, setOpen] = useState(false)
  const [dragging, setDragging] = useState(false)
  const def = defOf(scope, row.type)
  const Icon = rowIcon(scope, row.type)
  const title = rowTitle(t, scope, row)
  const configurable = def?.sized || def?.count !== undefined

  const onHandleKey = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === 'ArrowUp' && index > 0) {
      e.preventDefault()
      editor.move(row.id, index - 1)
    } else if (e.key === 'ArrowDown' && !last) {
      e.preventDefault()
      editor.move(row.id, index + 1)
    }
  }

  return (
    <Reorder.Item
      as="li"
      value={row}
      dragListener={false}
      dragControls={controls}
      onDragStart={() => setDragging(true)}
      onDragEnd={() => setDragging(false)}
      transition={springs.smooth}
      whileDrag={{ scale: 1.012 }}
      className={clsx(
        'jr-home-row',
        row.hidden && 'jr-home-row--hidden',
        dragging && 'jr-home-row--dragging',
      )}
    >
      <div className="jr-home-row__main">
        <button
          type="button"
          className="jr-home-row__handle"
          aria-label={t('dragHandle', { title })}
          title={t('dragHandleHint')}
          onPointerDown={(e) => controls.start(e)}
          onKeyDown={onHandleKey}
        >
          <GripVertical aria-hidden="true" />
        </button>
        <span className="jr-home-row__icon" aria-hidden="true">
          <Icon />
        </span>
        <span className="jr-home-row__text">
          <span className="jr-home-row__title">{title}</span>
          <span className="jr-home-row__sub">
            {row.hidden ? t('hiddenRow') : rowSummary(t, scope, row, def)}
          </span>
        </span>
        <div className="jr-home-row__actions">
          {configurable && (
            <IconButton
              size="small"
              variant={open ? 'tinted' : 'plain'}
              icon={<SlidersHorizontal />}
              label={t('options')}
              pressed={open}
              aria-expanded={open}
              onClick={() => setOpen((o) => !o)}
            />
          )}
          <IconButton
            size="small"
            icon={row.hidden ? <EyeOff /> : <Eye />}
            label={row.hidden ? t('show', { title }) : t('hide', { title })}
            onClick={() => editor.setHidden(row.id, !row.hidden)}
          />
          <IconButton
            size="small"
            icon={<Trash2 />}
            label={t('remove', { title })}
            onClick={() => editor.remove(row.id)}
          />
        </div>
      </div>
      <AnimatePresence initial={false}>
        {open && configurable && (
          <motion.div
            className="jr-home-row__options"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2 }}
          >
            <RowOptions scope={scope} row={row} def={def} editor={editor} />
          </motion.div>
        )}
      </AnimatePresence>
    </Reorder.Item>
  )
}

function rowSummary(
  t: ReturnType<typeof useTranslation>['t'],
  scope: HomeScope,
  row: HomeRow,
  def: RowDef | undefined,
): string {
  const parts: string[] = [t('summary.items', { count: countOf(row, scope) })]
  if (def?.sized && row.size && row.size !== 'auto') parts.push(t(`size.${row.size}`))
  return parts.join(' · ')
}

function RowOptions({
  scope,
  row,
  def,
  editor,
}: {
  scope: HomeScope
  row: HomeRow
  def: RowDef | undefined
  editor: HomeEditor
}) {
  const { t } = useTranslation('home')
  const choices = countChoices(row.type)
  const count = countOf(row, scope)
  return (
    <div className="jr-home-row__options-inner">
      {def?.sized && (
        <div className="jr-home-row__option">
          <span className="jr-home-row__option-label">{t('cardSize')}</span>
          <SegmentedControl<RowSize>
            size="small"
            aria-label={t('cardSize')}
            value={row.size ?? 'auto'}
            onChange={(size) => editor.setOptions(row.id, { size })}
            options={SIZE_OPTIONS.map((s) => ({ value: s, label: t(`size.${s}`) }))}
          />
          {(row.size ?? 'auto') === 'auto' && (
            <span className="jr-home-row__option-note">{t('sizeAutoNote')}</span>
          )}
        </div>
      )}
      <div className="jr-home-row__option">
        <span className="jr-home-row__option-label">{t('itemCount')}</span>
        <SegmentedControl<string>
          size="small"
          aria-label={t('itemCount')}
          value={String(count)}
          onChange={(v) => editor.setOptions(row.id, { count: Number(v) })}
          options={choices.map((n) => ({ value: String(n), label: String(n) }))}
        />
      </div>
    </div>
  )
}

// ---------------------------------------------------------------- "Add a row"

export interface AddRowSheetProps {
  open: boolean
  onClose(): void
  scope: HomeScope
  layout: HomeLayout
  /** Libraries offered for a "Recently Added in…" row. */
  libraries?: readonly { id: string; name: string }[]
  onAdd(spec: RowSpec): void
}

/** Lists the rows that can be added; a row about a genre, library, collection or artist asks which. */
export function AddRowSheet(props: AddRowSheetProps) {
  const { t } = useTranslation('home')
  const [picking, setPicking] = useState<RowDef | null>(null)
  const close = () => {
    props.onClose()
    // Keep the content while the sheet fades out.
    window.setTimeout(() => setPicking(null), 200)
  }
  const add = (spec: RowSpec) => {
    props.onAdd(spec)
    close()
  }

  return (
    <Sheet
      open={props.open}
      onClose={close}
      size="medium"
      title={picking ? rowName(t, props.scope, picking.type) : t('addRow')}
      description={picking ? t(`pick.${picking.param}`) : t('addRowHint')}
      footer={
        picking ? (
          <Button variant="gray" icon={<ChevronLeft />} onClick={() => setPicking(null)}>
            {t('back')}
          </Button>
        ) : undefined
      }
    >
      {picking ? (
        <Picker
          def={picking}
          scope={props.scope}
          layout={props.layout}
          libraries={props.libraries ?? []}
          onPick={(param, label) => add({ type: picking.type, param, label })}
        />
      ) : (
        <ul className="jr-home-add">
          {ROW_DEFS[props.scope].map((def) => {
            const Icon = rowIcon(props.scope, def.type)
            const present = def.unique && hasRow(props.layout, def.type)
            return (
              <li key={def.type}>
                <button
                  type="button"
                  className="jr-home-add__item"
                  disabled={present}
                  onClick={() => (def.param ? setPicking(def) : add({ type: def.type }))}
                >
                  <span className="jr-home-row__icon" aria-hidden="true">
                    <Icon />
                  </span>
                  <span className="jr-home-row__text">
                    <span className="jr-home-row__title">{rowName(t, props.scope, def.type)}</span>
                    <span className="jr-home-row__sub">
                      {present ? t('alreadyAdded') : rowDescription(t, props.scope, def.type)}
                    </span>
                  </span>
                  {def.param && (
                    <span className="jr-home-add__more" aria-hidden="true">
                      ›
                    </span>
                  )}
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </Sheet>
  )
}

interface Choice {
  value: string
  label: string
}

function Picker({
  def,
  scope,
  layout,
  libraries,
  onPick,
}: {
  def: RowDef
  scope: HomeScope
  layout: HomeLayout
  libraries: readonly { id: string; name: string }[]
  onPick(param: string, label: string): void
}) {
  const { t } = useTranslation('home')
  const [query, setQuery] = useState('')
  const isArtist = def.param === 'artist'
  const genres = useGenres(scope, def.param === 'genre')
  const collections = useCollections(def.param === 'collection')
  const artists = useAlbumArtists({
    searchTerm: query.trim() || undefined,
    pageSize: 40,
    enabled: isArtist,
  })

  let choices: Choice[] = []
  let loading = false
  if (def.param === 'library') {
    choices = libraries.map((l) => ({ value: l.id, label: l.name }))
  } else if (def.param === 'genre') {
    loading = genres.isLoading
    choices = (genres.data ?? []).flatMap((g) => (g.Name ? [{ value: g.Name, label: g.Name }] : []))
  } else if (def.param === 'collection') {
    loading = collections.isLoading
    choices = flattenPages(collections.data).flatMap((c) =>
      c.Id && c.Name ? [{ value: c.Id, label: c.Name }] : [],
    )
  } else if (isArtist) {
    loading = artists.isLoading
    choices = flattenPages(artists.data).flatMap((a) =>
      a.Id && a.Name ? [{ value: a.Id, label: a.Name }] : [],
    )
  }
  const q = query.trim().toLowerCase()
  if (q && !isArtist) choices = choices.filter((c) => c.label.toLowerCase().includes(q))

  return (
    <div className="jr-home-pick">
      {(isArtist || choices.length > 8 || q) && (
        <SearchField
          value={query}
          onChange={setQuery}
          placeholder={t('search')}
          aria-label={t('search')}
        />
      )}
      {loading ? (
        <p className="jr-home-edit__empty">{t('loading')}</p>
      ) : choices.length === 0 ? (
        <p className="jr-home-edit__empty">{t('nothingToPick')}</p>
      ) : (
        <ul className="jr-home-add jr-home-pick__list">
          {choices.map((c) => {
            const present = hasRow(layout, def.type, c.value)
            return (
              <li key={c.value}>
                <button
                  type="button"
                  className="jr-home-add__item"
                  disabled={present}
                  onClick={() => onPick(c.value, c.label)}
                >
                  <span className="jr-home-row__text">
                    <span className="jr-home-row__title">{c.label}</span>
                    {present && <span className="jr-home-row__sub">{t('alreadyAdded')}</span>}
                  </span>
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
