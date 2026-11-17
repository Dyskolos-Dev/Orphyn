import { RotateCcw } from 'lucide-react'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import {
  ACCENT_PRESETS,
  BACKDROP_OPTIONS,
  Button,
  CORNER_OPTIONS,
  DENSITY_OPTIONS,
  List,
  ListRow,
  MOTION_OPTIONS,
  SegmentedControl,
  SIDEBAR_OPTIONS,
  Slider,
  SOLID_STYLES,
  TEXT_SIZE_OPTIONS,
  Toggle,
  VIDEO_CARD_OPTIONS,
  accentHex,
  accentTokens,
  toast,
  useInterfaceStyle,
  useTheme,
  type AccentColor,
} from '@renderer/design'
import { StyleGallery, StylePreview } from './StyleGallery'

const HEX = /^#[0-9a-f]{6}$/i

/**
 * Settings > Appearance: a live preview of the current style with every option, the style
 * gallery, light / dark, accent (presets, custom colour, follow the artwork) and the appearance
 * options of the interface style store. Everything applies at once and persists.
 */
export function AppearancePane() {
  const { t } = useTranslation('settings')
  const theme = useTheme()
  const { style, appearance, overrides, setAppearance, resetAppearance } = useInterfaceStyle()
  const solidStyle = SOLID_STYLES.includes(style)
  const styleName = t(`appearance.styles.${style}.name`)
  const hasOverrides = Object.keys(overrides).length > 0

  return (
    <div className="jr-appearance">
      <figure className="jr-appearance__preview">
        <StylePreview style={style} appearance={appearance} large />
        <figcaption className="jr-appearance__caption">
          <span className="jr-appearance__caption-title">{styleName}</span>
          <span>{t('appearance.previewCaption')}</span>
        </figcaption>
      </figure>

      <List variant="inset" header={t('appearance.style.header')}>
        <li className="jr-list__item">
          <StyleGallery />
        </li>
      </List>

      <List variant="inset" header={t('appearance.mode.header')}>
        <ListRow
          title={t('appearance.mode.label')}
          trailing={
            <SegmentedControl
              aria-label={t('appearance.mode.label')}
              size="small"
              value={theme.mode}
              onChange={theme.setMode}
              options={[
                { value: 'system', label: t('appearance.mode.system') },
                { value: 'light', label: t('appearance.mode.light') },
                { value: 'dark', label: t('appearance.mode.dark') },
              ]}
            />
          }
        />
      </List>

      <AccentSection />

      <List variant="inset" header={t('appearance.materials.header')}>
        <ListRow
          title={t('appearance.materials.transparency.label')}
          subtitle={
            solidStyle
              ? t('appearance.materials.transparency.solidStyle')
              : t('appearance.materials.transparency.description')
          }
          trailing={
            <div className="jr-appearance__slider">
              <Slider
                variant="regular"
                aria-label={t('appearance.materials.transparency.label')}
                min={0}
                max={100}
                step={5}
                value={solidStyle ? 0 : appearance.transparency}
                disabled={solidStyle}
                onChange={(v) => setAppearance({ transparency: Math.round(v) })}
                formatValue={(v) =>
                  t('appearance.materials.transparency.value', { percent: Math.round(v) })
                }
              />
              <span className="jr-appearance__value jr-tabular" aria-hidden="true">
                {solidStyle ? 0 : appearance.transparency}
              </span>
            </div>
          }
        />
        <ListRow
          title={t('appearance.materials.corners.label')}
          trailing={
            <SegmentedControl
              aria-label={t('appearance.materials.corners.label')}
              size="small"
              value={appearance.corners}
              onChange={(corners) => setAppearance({ corners })}
              options={CORNER_OPTIONS.map((v) => ({
                value: v,
                label: t(`appearance.materials.corners.${v}`),
              }))}
            />
          }
        />
        <ListRow
          title={t('appearance.materials.density.label')}
          subtitle={t('appearance.materials.density.description')}
          trailing={
            <SegmentedControl
              aria-label={t('appearance.materials.density.label')}
              size="small"
              value={appearance.density}
              onChange={(density) => setAppearance({ density })}
              options={DENSITY_OPTIONS.map((v) => ({
                value: v,
                label: t(`appearance.materials.density.${v}`),
              }))}
            />
          }
        />
        <ListRow
          title={t('appearance.materials.textSize.label')}
          trailing={
            <SegmentedControl
              aria-label={t('appearance.materials.textSize.label')}
              size="small"
              value={appearance.textSize}
              onChange={(textSize) => setAppearance({ textSize })}
              options={TEXT_SIZE_OPTIONS.map((v) => ({
                value: v,
                label: t(`appearance.materials.textSize.${v}`),
                ariaLabel: t(`appearance.materials.textSize.${v}Label`),
              }))}
            />
          }
        />
      </List>

      <List variant="inset" header={t('appearance.layout.header')}>
        <ListRow
          title={t('appearance.layout.sidebar.label')}
          subtitle={
            appearance.sidebar === 'auto-hide'
              ? t('appearance.layout.sidebar.autoHideHint')
              : undefined
          }
          trailing={
            <SegmentedControl
              aria-label={t('appearance.layout.sidebar.label')}
              size="small"
              value={appearance.sidebar}
              onChange={(sidebar) => setAppearance({ sidebar })}
              options={SIDEBAR_OPTIONS.map((v) => ({
                value: v,
                label: t(`appearance.layout.sidebar.${v}`),
              }))}
            />
          }
        />
        <ListRow
          title={t('appearance.layout.backdrop.label')}
          subtitle={t('appearance.layout.backdrop.description')}
          trailing={
            <SegmentedControl
              aria-label={t('appearance.layout.backdrop.label')}
              size="small"
              value={appearance.backdrop}
              onChange={(backdrop) => setAppearance({ backdrop })}
              options={BACKDROP_OPTIONS.map((v) => ({
                value: v,
                label: t(`appearance.layout.backdrop.${v}`),
              }))}
            />
          }
        />
        <ListRow
          title={t('appearance.layout.videoCards.label')}
          trailing={
            <SegmentedControl
              aria-label={t('appearance.layout.videoCards.label')}
              size="small"
              value={appearance.videoCards}
              onChange={(videoCards) => setAppearance({ videoCards })}
              options={VIDEO_CARD_OPTIONS.map((v) => ({
                value: v,
                label: t(`appearance.layout.videoCards.${v}`),
              }))}
            />
          }
        />
        <ListRow
          title={t('appearance.layout.motion.label')}
          subtitle={t('appearance.layout.motion.description')}
          trailing={
            <SegmentedControl
              aria-label={t('appearance.layout.motion.label')}
              size="small"
              value={appearance.motion}
              onChange={(motion) => setAppearance({ motion })}
              options={MOTION_OPTIONS.map((v) => ({
                value: v,
                label: t(`appearance.layout.motion.${v}`),
              }))}
            />
          }
        />
      </List>

      <div className="jr-appearance__reset">
        <p className="jr-appearance__reset-text">
          {t('appearance.reset.description', { style: styleName })}
        </p>
        <Button
          variant="gray"
          icon={<RotateCcw />}
          disabled={!hasOverrides}
          onClick={() => {
            resetAppearance()
            toast({ title: t('appearance.reset.done', { style: styleName }) })
          }}
        >
          {t('appearance.reset.label')}
        </Button>
      </div>
    </div>
  )
}

/** Accent: preset swatches, a custom colour, and "follow artwork colour". */
function AccentSection() {
  const { t } = useTranslation('settings')
  const { accent, resolvedTheme, followArtwork, setAccent, setFollowArtwork } = useTheme()
  const hintId = useId()
  const custom = HEX.test(accent)
  const customTokens = custom ? accentTokens(accent, resolvedTheme) : null
  const adjusted =
    customTokens != null &&
    (customTokens.accent !== accent.toLowerCase() || customTokens.fill !== accent.toLowerCase())
  const swatch = (a: AccentColor) => accentTokens(accentHex(a, resolvedTheme), resolvedTheme).fill

  return (
    <List variant="inset" header={t('appearance.accent.header')}>
      <li className="jr-list__item">
        <div
          className="jr-swatches"
          role="radiogroup"
          aria-label={t('appearance.accent.label')}
          aria-describedby={adjusted ? hintId : undefined}
          data-disabled={followArtwork || undefined}
        >
          {ACCENT_PRESETS.map((preset) => (
            <button
              key={preset}
              type="button"
              role="radio"
              aria-checked={accent === preset}
              aria-label={t(`appearance.accent.colors.${preset}`)}
              title={t(`appearance.accent.colors.${preset}`)}
              className="jr-swatch"
              style={{ background: swatch(preset) }}
              onClick={() => setAccent(preset)}
            />
          ))}
          <label
            className="jr-swatch jr-swatch--custom"
            data-checked={custom || undefined}
            title={t('appearance.accent.custom')}
            style={custom ? { background: swatch(accent) } : undefined}
          >
            <input
              type="color"
              aria-label={t('appearance.accent.custom')}
              value={custom ? accent : '#fa2d48'}
              onChange={(e) => setAccent(e.target.value as `#${string}`)}
            />
          </label>
        </div>
        {adjusted && customTokens && (
          <p id={hintId} className="jr-appearance__hint">
            {t('appearance.accent.adjusted', {
              text: customTokens.accent.toUpperCase(),
              fill: customTokens.fill.toUpperCase(),
            })}
          </p>
        )}
      </li>
      <ListRow
        title={t('appearance.accent.followArtwork.label')}
        subtitle={t('appearance.accent.followArtwork.description')}
        trailing={
          <Toggle
            aria-label={t('appearance.accent.followArtwork.label')}
            checked={followArtwork}
            onChange={setFollowArtwork}
          />
        }
      />
    </List>
  )
}
