import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { List, ListRow, SegmentedControl } from '@renderer/design'
import {
  LANGUAGE_CHOICES,
  getLanguageChoice,
  setLanguageChoice,
  type LanguageChoice,
} from './language'

/** Settings > General: the language. (Look and feel live in the Appearance pane.) */
export function GeneralPane() {
  const { t } = useTranslation('settings')
  const [language, setLanguage] = useState<LanguageChoice>(getLanguageChoice)

  return (
    <List variant="inset" header={t('general.language.header')}>
      <ListRow
        title={t('general.language.label')}
        subtitle={t('general.language.description')}
        trailing={
          <SegmentedControl
            aria-label={t('general.language.label')}
            size="small"
            value={language}
            onChange={(next) => {
              setLanguage(next)
              void setLanguageChoice(next)
            }}
            options={LANGUAGE_CHOICES.map((c) => ({
              value: c,
              label: t(`general.language.${c}`),
            }))}
          />
        }
      />
    </List>
  )
}
