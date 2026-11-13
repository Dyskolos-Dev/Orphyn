import clsx from 'clsx'
import type { ReactNode } from 'react'
import './navigation.css'

export interface ToolbarProps {
  /** Small centred-left title (the collapsed large title). */
  title?: ReactNode
  /** Show the title (fade it in when the large title scrolled away). Default true. */
  showTitle?: boolean
  /** Left side: back / forward buttons. */
  leading?: ReactNode
  /** Right side: actions (IconButtons, SegmentedControl...). */
  trailing?: ReactNode
  /**
   * `material`: blurred bar with a hairline (content scrolled under it).
   * `transparent`: no background (over a HeroBanner or before scrolling).
   */
  appearance?: 'material' | 'transparent'
  className?: string
}

/**
 * Window toolbar, 52px. The empty area drags the window (Electron app-region); buttons, inputs and
 * links inside stay clickable. Leaves room for the Windows caption buttons.
 */
export function Toolbar({
  title,
  showTitle = true,
  leading,
  trailing,
  appearance = 'material',
  className,
}: ToolbarProps) {
  return (
    <div className={clsx('jr-toolbar', `jr-toolbar--${appearance}`, 'jr-drag', className)}>
      <div className="jr-toolbar__leading">{leading}</div>
      <div
        className={clsx('jr-toolbar__title', showTitle && title && 'jr-toolbar__title--visible')}
        aria-hidden={!showTitle || undefined}
      >
        {title}
      </div>
      <div className="jr-toolbar__trailing">{trailing}</div>
    </div>
  )
}
