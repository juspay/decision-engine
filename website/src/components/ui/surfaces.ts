/**
 * The surface scale for config screens — the padding and corner radius a bordered box gets.
 *
 * A surface is named for what it holds, not for its measurements, so the question at a call site
 * is "is this a row or a card?" rather than "was it px-3 py-2 or px-4 py-3 last time?". Reach for
 * a role here before writing padding by hand; a box that fits none of them is usually two boxes.
 *
 * Every value sits on the 4px grid, which is what Tailwind's numeric steps and the rest of the app
 * already use. Radius climbs with padding so that a row nested inside a panel keeps a visibly
 * tighter corner than its container.
 *
 * Spacing *between* surfaces belongs to the parent's `gap`, not to margins on the children — a
 * flex or grid parent with one `gap` cannot drift the way per-child margins do.
 */

/** A dense row: a table cell, a list item, the body of a chip. The tightest surface in the set. */
export const row = 'rounded-lg px-3 py-2'

/** A card holding a title and a line or two of copy — the default for a self-contained block. */
export const card = 'rounded-xl px-4 py-3'

/** A panel grouping several cards, or a page-level section that needs to read as one region. */
export const panel = 'rounded-2xl px-5 py-4'

/** A dialog or modal body, where content sits away from the edge on all four sides. */
export const dialog = 'rounded-2xl p-6'

/** The gap between sibling surfaces inside a panel. */
export const stackGap = 'gap-3'

/** The gap between panels down a page. */
export const sectionGap = 'gap-6'
