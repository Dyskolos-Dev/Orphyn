import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ACCENT_PRESETS, List, ListRow, SegmentedControl, useTheme } from '@renderer/design'
import {
  LANGUAGE_CHOICES,
  getLanguageChoice,
  setLanguageChoice,
  type LanguageChoice,
} from './language'

const HEX = /^#[0-9a-f]{6}$/i

export function GeneralPane() {
  const { t } = useTranslation('settings')
  const { mode, accent, setMode, setAccent } = useTheme()
  const [language, setLanguage] = useState<LanguageChoice>(getLanguageChoice)
  const custom = HEX.test(accent)

  return (
    <>
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

      <List variant="inset" header={t('general.appearance.header')}>
        <ListRow
          title={t('general.appearance.label')}
          trailing={
            <SegmentedControl
              aria-label={t('general.appearance.label')}
              size="small"
              value={mode}
              onChange={setMode}
              options={[
                { value: 'system', label: t('general.appearance.system') },
                { value: 'light', label: t('general.appearance.light') },
                { value: 'dark', label: t('general.appearance.dark') },
              ]}
            />
          }
        />
      </List>

      <List variant="inset" header={t('general.accent.header')}>
        <li className="jr-list__item">
          <div className="jr-swatches" role="radiogroup" aria-label={t('general.accent.label')}>
            {ACCENT_PRESETS.map((preset) => (
              <button
                key={preset}
                type="button"
                role="radio"
                aria-checked={accent === preset}
                aria-label={t(`general.accent.colors.${preset}`)}
                title={t(`general.accent.colors.${preset}`)}
                data-accent={preset}
                className="jr-swatch"
                onClick={() => setAccent(preset)}
              />
            ))}
            <label className="jr-swatch jr-swatch--custom" data-checked={custom || undefined}>
              <input
                type="color"
                aria-label={t('general.accent.custom')}
                title={t('general.accent.custom')}
                value={custom ? accent : '#fa2d48'}
                onChange={(e) => setAccent(e.target.value as `#${string}`)}
              />
            </label>
          </div>
        </li>
      </List>
    </>
  )
}
