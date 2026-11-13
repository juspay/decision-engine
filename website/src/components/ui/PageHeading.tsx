import type { ReactNode } from 'react'
import * as type from './typography'

interface PageHeadingProps {
  /** The name of the screen. Plain text on almost every page. */
  title: ReactNode
  /** One sentence saying what the screen is for. Omit it where the title already says everything. */
  description?: ReactNode
  /** Sits inline after the title — a merchant chip, a BETA tag. */
  badge?: ReactNode
  /** Layout classes for the block itself, such as `min-w-0` inside a flex row. */
  className?: string
}

/**
 * The title and subtitle at the top of a page.
 *
 * Every screen renders its header through this, so the two roles it applies stay the answer to
 * "how big is a page title" in one place rather than seventeen. It owns only the text block: the
 * row that holds it, and any buttons sitting opposite, belong to the page, whose header may be a
 * two-column flex on one screen and a three-column grid on another.
 */
export function PageHeading({ title, description, badge, className = '' }: PageHeadingProps) {
  return (
    <div className={className || undefined}>
      {badge ? (
        <div className="flex flex-wrap items-center gap-2">
          <h1 className={type.pageTitle}>{title}</h1>
          {badge}
        </div>
      ) : (
        <h1 className={type.pageTitle}>{title}</h1>
      )}
      {description ? <p className={type.pageSubtitle}>{description}</p> : null}
    </div>
  )
}
