import { Music } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@renderer/design'
import { AddRowSheet, HomeEditActions, HomeEditList, useHomeEditor } from '@renderer/features/home'
import { useAlbums } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { MusicRow } from './HomeRows'

/** "Listen Now": the rows the user chose (see features/home), recently played first by default. */
export function HomePage() {
  const { t } = useTranslation('music')
  const editor = useHomeEditor('music')
  const [adding, setAdding] = useState(false)
  // Any album at all? Otherwise the library is empty and no row would show anything.
  const probe = useAlbums({ pageSize: 1 })
  const empty = !probe.isLoading && (probe.data?.pages[0]?.total ?? 0) === 0
  const actions = <HomeEditActions editor={editor} onAdd={() => setAdding(true)} />

  if (editor.editing) {
    return (
      <ShellPage title={t('home.title')} actions={actions}>
        <HomeEditList scope="music" editor={editor} />
        <AddRowSheet
          open={adding}
          onClose={() => setAdding(false)}
          scope="music"
          layout={editor.layout}
          onAdd={editor.add}
        />
      </ShellPage>
    )
  }

  return (
    <ShellPage title={t('home.title')} actions={actions}>
      {empty && (
        <EmptyState icon={<Music />} title={t('home.empty')} message={t('home.emptyHint')} />
      )}
      {editor.layout.rows
        .filter((r) => !r.hidden)
        .map((row) => (
          <MusicRow key={row.id} row={row} />
        ))}
    </ShellPage>
  )
}
