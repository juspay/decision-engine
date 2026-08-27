/**
 * The type scale for config surfaces.
 *
 * These screens had drifted to nine sizes (9, 10, 11, 12, 13, 14px plus three tracking values),
 * chosen per-component. The result reads as unconsidered: headings the same size as body text,
 * hints too small and too pale to actually read, and letterspaced all-caps kickers sitting above
 * headings that already said the same thing.
 *
 * Prefer these over a literal `text-[11px]`; if something genuinely needs a size that isn't here,
 * that's a signal the hierarchy is wrong, not that the scale needs another entry.
 *
 * Every role fixes a size *and* its line-height, the way Material 3's type scale does — an
 * unpaired `text-[13px]` inherits whatever leading sits above it, which is how the same copy ends
 * up set differently on two screens. Ratios track M3's: ~1.45 at label sizes, ~1.4 through body,
 * tightening as the size grows. 11px is the floor, matching M3's smallest role.
 *
 * Letter-spacing is deliberately absent. M3 hand-tunes tracking per size because its scale has to
 * work with static fonts; the family here is variable with `font-optical-sizing: auto` on the
 * document, so the typeface widens small text and tightens large text on its own. Adding a
 * tracking table would apply that correction twice.
 *
 * The roles that hold running text carry `max-w-[57ch]`. `ch` is the width of the zero glyph,
 * which runs far wider than an average character — 0.64em against 0.46em in Google Sans, 0.63em
 * against 0.48em in Inter — so a `ch` cap is roughly 1.4x wider than its number suggests: 57ch is
 * ~475px at 13px, holding 75-80 characters, around the top of the range where the eye reliably
 * finds the next line. A page subtitle is one sentence that gets scanned rather than read, so
 * `pageSubtitle` takes no measure and runs to its column.
 */

/**
 * The h1 at the top of a page. Deliberately close to a card heading: the sidebar and the tab row
 * already say where you are, so the title states the screen rather than announcing it.
 */
export const pageTitle = 'text-lg font-semibold text-slate-900 dark:text-white'

/** The sentence under a page title, saying what the screen is for. */
export const pageSubtitle = 'mt-0.5 text-[13px] leading-relaxed text-slate-500 dark:text-[#9ca7ba]'

/** Card and section titles. Carries the hierarchy, so it outranks body text rather than matching it. */
export const heading = 'text-[15px] leading-[22px] font-semibold text-slate-900 dark:text-white'

/** The supporting line under a heading. One sentence — if it needs two, the UI is doing too much. */
export const subheading =
  'max-w-[57ch] text-[13px] leading-relaxed text-slate-500 dark:text-[#9ca7ba]'

/** Form field labels. Sentence case, and never smaller than the input's own text. */
export const label = 'text-[13px] leading-[18px] font-medium text-slate-700 dark:text-[#c7cfdd]'

/**
 * Field hints and captions. 13px at slate-500 rather than 11px at slate-400: the old treatment was
 * below comfortable reading size and below WCAG AA contrast on white, which is how "small print"
 * becomes decorative instead of informative.
 */
export const hint = 'max-w-[57ch] text-[13px] leading-relaxed text-slate-500 dark:text-[#8d96aa]'

/**
 * Table column headers. Sentence case, no letterspacing — uppercase tracking-[0.14em] headers are
 * slower to read and were applied inconsistently (0.14em / 0.16em / 0.18em / tracking-wide).
 */
export const tableHeader = 'text-[12px] leading-4 font-medium text-slate-500 dark:text-[#8d96aa]'

/** Default body copy inside a card or panel. The workhorse — reach for this before anything else. */
export const body = 'text-[13px] leading-[18px] text-slate-600 dark:text-[#a8b4c8]'

/** Body copy in a dense row — a table cell, a list item, a breakdown line. */
export const bodySmall = 'text-[12px] leading-4 text-slate-500 dark:text-[#8d96aa]'

/**
 * The smallest role in the scale: 11px, for uppercase kickers and badge text. Anything that wants
 * to be smaller than this wants to be a different element.
 */
export const labelSmall =
  'text-[11px] leading-4 font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8d96aa]'

/** A figure that carries a card — a success rate, a saved-cost number, a decision count. */
export const metric =
  'text-[21px] leading-7 font-semibold tabular-nums text-slate-900 dark:text-white'

/** The headline figure on an overview tile, where one number is the whole point of the card. */
export const metricLarge =
  'text-[34px] leading-[42px] font-semibold tabular-nums text-slate-900 dark:text-white'
