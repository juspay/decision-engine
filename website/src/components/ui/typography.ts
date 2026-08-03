/**
 * The type scale for config surfaces.
 *
 * These screens had drifted to nine sizes (9, 10, 11, 12, 13, 14px plus three tracking values),
 * chosen per-component. The result reads as unconsidered: headings the same size as body text,
 * hints too small and too pale to actually read, and letterspaced all-caps kickers sitting above
 * headings that already said the same thing.
 *
 * Four roles, four sizes. Prefer these over a literal `text-[11px]`; if something genuinely needs a
 * size that isn't here, that's a signal the hierarchy is wrong, not that the scale needs a ninth
 * entry.
 */

/** Card and section titles. Carries the hierarchy, so it outranks body text rather than matching it. */
export const heading = 'text-[15px] font-semibold text-slate-900 dark:text-white'

/** The supporting line under a heading. One sentence — if it needs two, the UI is doing too much. */
export const subheading = 'text-[13px] leading-relaxed text-slate-500 dark:text-[#9ca7ba]'

/** Form field labels. Sentence case, and never smaller than the input's own text. */
export const label = 'text-[13px] font-medium text-slate-700 dark:text-[#c7cfdd]'

/**
 * Field hints and captions. 13px at slate-500 rather than 11px at slate-400: the old treatment was
 * below comfortable reading size and below WCAG AA contrast on white, which is how "small print"
 * becomes decorative instead of informative.
 */
export const hint = 'text-[13px] leading-relaxed text-slate-500 dark:text-[#8d96aa]'

/**
 * Table column headers. Sentence case, no letterspacing — uppercase tracking-[0.14em] headers are
 * slower to read and were applied inconsistently (0.14em / 0.16em / 0.18em / tracking-wide).
 */
export const tableHeader =
  'text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]'
