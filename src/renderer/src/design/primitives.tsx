import clsx from 'clsx'
import type { CSSProperties, ElementType, ReactNode } from 'react'

/** text styles (see the type scale in docs/DESIGN.md). */
export type TextStyle =
  | 'large-title'
  | 'title-1'
  | 'title-2'
  | 'title-3'
  | 'headline'
  | 'body'
  | 'callout'
  | 'subheadline'
  | 'footnote'
  | 'caption'

const DEFAULT_TAG: Record<TextStyle, ElementType> = {
  'large-title': 'h1',
  'title-1': 'h1',
  'title-2': 'h2',
  'title-3': 'h3',
  headline: 'p',
  body: 'p',
  callout: 'p',
  subheadline: 'p',
  footnote: 'p',
  caption: 'span',
}

export interface TextProps {
  /** Text style; default `body`. */
  variant?: TextStyle
  /** Element to render (defaults: h1 for large-title / title-1, h2, h3, p, span for caption). */
  as?: ElementType
  /** Label colour level. */
  color?: 'primary' | 'secondary' | 'tertiary' | 'accent' | 'danger'
  weight?: 'regular' | 'medium' | 'semibold' | 'bold'
  /** One line with ellipsis, or clamp to 2 / 3 lines. */
  truncate?: boolean | 2 | 3
  /** Allow selecting the text (chrome text is not selectable by default). */
  selectable?: boolean
  /** Tabular figures (durations, counters). */
  tabular?: boolean
  id?: string
  className?: string
  style?: CSSProperties
  children?: ReactNode
  /** `data-*` attributes are forwarded (e.g. `data-page-title` for ShellPage). */
  [data: `data-${string}`]: string | undefined
}

const WEIGHT = { regular: 400, medium: 500, semibold: 600, bold: 700 } as const

/** Text in one of the text styles. */
export function Text({
  variant = 'body',
  as,
  color = 'primary',
  weight,
  truncate,
  selectable,
  tabular,
  className,
  style,
  children,
  id,
  ...data
}: TextProps) {
  const Tag = as ?? DEFAULT_TAG[variant]
  return (
    <Tag
      {...data}
      id={id}
      className={clsx(
        `jr-text-${variant}`,
        `jr-color-${color}`,
        truncate === true && 'jr-truncate',
        truncate === 2 && 'jr-clamp-2',
        truncate === 3 && 'jr-clamp-3',
        selectable && 'jr-selectable',
        tabular && 'jr-tabular',
        className,
      )}
      style={weight ? { fontWeight: WEIGHT[weight], ...style } : style}
    >
      {children}
    </Tag>
  )
}

export type MaterialThickness = 'ultra-thin' | 'thin' | 'regular' | 'thick'

export interface MaterialProps {
  thickness?: MaterialThickness
  /** Hairline border like the window materials. */
  bordered?: boolean
  as?: ElementType
  className?: string
  style?: CSSProperties
  children?: ReactNode
}

/** Translucent blurred surface (backdrop-filter). Use over content, never over flat colour. */
export function Material({
  thickness = 'regular',
  bordered = false,
  as: Tag = 'div',
  className,
  style,
  children,
}: MaterialProps) {
  return (
    <Tag
      className={clsx(`jr-material-${thickness}`, bordered && 'jr-material-bordered', className)}
      style={style}
    >
      {children}
    </Tag>
  )
}
