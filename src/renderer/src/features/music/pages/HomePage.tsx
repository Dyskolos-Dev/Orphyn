import { Music } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@renderer/design'
import { useHomeEditor } from '@renderer/features/home'
import { useAlbums } from '@renderer/jellyfin'
import { ShellPage } from '@renderer/shell'
import { MusicRow } from './HomeRows'

/** "Listen Now": the rows the user chose (see features/home), recently played first by default. */
export function HomePage() {
  const { t } = useTranslation('music')
  const editor = useHomeEditor('music')
  // Any album at all? Otherwise the library is empty and no row would show anything.
  const probe = useAlbums({ pageSize: 1 })
  const empty = !probe.isLoading && (probe.data?.pages[0]?.total ?? 0) === 0

  return (
    <ShellPage title={t('home.title')}>
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
