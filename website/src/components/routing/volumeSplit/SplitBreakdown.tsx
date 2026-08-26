import { VolumeSplitGatewayFormEntry } from '../../../features/routing/volumeSplit/types'

/** Slot colours, indexed by position so a gateway keeps its colour across the editor and the bar. */
export const SPLIT_COLORS = ['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899']

/**
 * How a volume-split rule divides traffic, drawn once for every surface that shows it — the
 * expanded row on the rules list and the builder's live rail.
 *
 * Part-to-whole with long, variable-length category names, so it is a horizontal 100% bar with the
 * names in a legend beneath. Pie labels for these would overflow their container.
 */
export function SplitBreakdown({ gateways }: { gateways: VolumeSplitGatewayFormEntry[] }) {
  const named = gateways.filter((g) => g.gatewayName.trim())
  if (named.length === 0) {
    return (
      <p className="text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
        No gateways yet — add at least one to split traffic across.
      </p>
    )
  }

  const total = named.reduce((sum, g) => sum + g.split, 0)

  return (
    <div className="space-y-3">
      <div className="flex h-3 w-full gap-0.5 overflow-hidden rounded-full">
        {named.map((g, i) => (
          <div
            key={g.id}
            className="h-full"
            style={{
              // flex-grow rather than width, so the 2px gaps come out of the available space
              // instead of pushing the last segment past the rounded clip.
              flex: `${total > 0 ? g.split : 1} 1 0`,
              backgroundColor: SPLIT_COLORS[i % SPLIT_COLORS.length],
            }}
            title={`${g.gatewayName}: ${g.split}%`}
          />
        ))}
      </div>

      <div className="space-y-1.5">
        {named.map((g, i) => (
          <div key={g.id} className="flex items-center gap-2 text-xs">
            <span
              className="h-2.5 w-2.5 shrink-0 rounded-full"
              style={{ backgroundColor: SPLIT_COLORS[i % SPLIT_COLORS.length] }}
            />
            <span className="min-w-0 truncate font-medium text-slate-700 dark:text-slate-200" title={g.gatewayName}>
              {g.gatewayName}
            </span>
            {g.gatewayId && (
              <span
                className="min-w-0 truncate font-mono text-[11px] text-slate-500 dark:text-[#78849a] leading-4"
                title={g.gatewayId}
              >
                {g.gatewayId}
              </span>
            )}
            <span className="ml-auto shrink-0 pl-2 font-semibold tabular-nums text-slate-700 dark:text-slate-200">
              {g.split}%
            </span>
          </div>
        ))}
      </div>

      {/* A split that misses 100% silently drops or double-counts traffic, so surface it wherever
          the distribution is shown — not just in the editor. */}
      {total !== 100 && (
        <p className="text-[11px] font-medium text-amber-700 dark:text-amber-400 leading-4">
          Splits total {total}% — must add up to 100%.
        </p>
      )}
    </div>
  )
}
