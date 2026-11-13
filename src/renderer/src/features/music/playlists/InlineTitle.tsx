import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

/** Playlist name that turns into a text field on click (Enter saves, Escape cancels). */
export function InlineTitle({
  value,
  onCommit,
  editing,
  onEditingChange,
}: {
  value: string
  onCommit(next: string): void
  editing: boolean
  onEditingChange(editing: boolean): void
}) {
  const { t } = useTranslation('music')
  const [draft, setDraft] = useState(value)
  const input = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (editing) {
      setDraft(value)
      input.current?.focus()
      input.current?.select()
    }
  }, [editing, value])

  const finish = (save: boolean) => {
    onEditingChange(false)
    const next = draft.trim()
    if (save && next && next !== value) onCommit(next)
  }

  if (!editing) {
    return (
      <button
        type="button"
        className="jrm-inline-title"
        title={t('playlist.rename')}
        onClick={() => onEditingChange(true)}
      >
        {value}
      </button>
    )
  }
  return (
    <input
      ref={input}
      className="jrm-inline-title jrm-inline-title--input"
      aria-label={t('playlist.rename')}
      value={draft}
      maxLength={120}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => finish(true)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') finish(true)
        else if (e.key === 'Escape') finish(false)
      }}
    />
  )
}
