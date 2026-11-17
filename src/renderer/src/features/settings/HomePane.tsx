import { Plus, RotateCcw } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button, SegmentedControl } from '@renderer/design'
import { AddRowSheet, HomeEditList, useHomeEditor, type HomeScope } from '@renderer/features/home'
import { useUserViews } from '@renderer/jellyfin'
import { HomeMiniature } from './HomeMiniature'

const VIDEO_VIEWS = new Set(['movies', 'tvshows'])

/** Settings > Home: arrange the Movies & TV and Music home screens. Every change is saved at once. */
export function HomePane() {
  const { t } = useTranslation('settings')
  const [scope, setScope] = useState<HomeScope>('video')
  const [adding, setAdding] = useState(false)
  const views = useUserViews()
  const libraries = (views.data ?? [])
    .filter((v) => VIDEO_VIEWS.has(v.CollectionType ?? ''))
    .flatMap((v) => (v.Id ? [{ id: v.Id, name: v.Name ?? '' }] : []))
  const editor = useHomeEditor(scope, scope === 'video' ? libraries : [])

  return (
    <div className="jr-settings-home">
      <SegmentedControl<HomeScope>
        aria-label={t('home.scope')}
        value={scope}
        onChange={setScope}
        options={[
          { value: 'video', label: t('home.video') },
          { value: 'music', label: t('home.music') },
        ]}
      />
      <div className="jr-settings-home__body">
        <div className="jr-settings-home__main">
          <HomeEditList key={scope} scope={scope} editor={editor} />
          <div className="jr-settings-home__actions">
            <Button variant="filled" size="small" icon={<Plus />} onClick={() => setAdding(true)}>
              {t('home.addRow')}
            </Button>
            <Button variant="plain" size="small" icon={<RotateCcw />} onClick={editor.reset}>
              {t('home.reset')}
            </Button>
          </div>
        </div>
        <HomeMiniature scope={scope} layout={editor.layout} />
      </div>
      <AddRowSheet
        open={adding}
        onClose={() => setAdding(false)}
        scope={scope}
        layout={editor.layout}
        libraries={scope === 'video' ? libraries : undefined}
        onAdd={editor.add}
      />
    </div>
  )
}
