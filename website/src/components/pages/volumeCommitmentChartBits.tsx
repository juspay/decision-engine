import type { CommitmentAuditEvent } from '../../types/api'

/**
 * Shared chart bits: solid = unaided, hatched = steered in, dashed = promise; a PSP keeps its
 * color whatever its standing.
 */

/** Series palette by contract position (CVD/contrast-checked in both themes). */
export const COMMITMENT_SERIES_COLORS = ['#0069ED', '#0d9488', '#ea580c', '#8b5cf6']
/** Ink for annotations that belong to no PSP — the "dropped" caption on a tail, for instance. */
export const NEUTRAL_INK = '#94a3b8'

/** Seconds in a calendar contract day; anything shorter is a `test_minutes` cycle. */
export const SECS_PER_DAY = 86_400

/** True on a `test_minutes` cycle, where one contract day lasts a minute. */
export function isTestCycle(daySecs?: number | null) {
  return (daySecs ?? SECS_PER_DAY) < SECS_PER_DAY
}

/** Sub-day buckets for the series: five-second ones on a test cycle, hourly on a calendar one. */
export function bucketsPerDay(daySecs?: number | null) {
  return isTestCycle(daySecs) ? 12 : 24
}

/** The word for one contract day on axes and captions. A test cycle's minutes *are* its contract
 *  days, and are shown as days so a demo reads exactly like production. */
export function dayUnit(_daySecs?: number | null): { word: 'day'; short: 'Day' } {
  return { word: 'day', short: 'Day' }
}

/** When each PSP was first eliminated, within `runId` only (an old cycle's drop must not pin day 0). */
export function firstEliminationByConnector(events: CommitmentAuditEvent[], runId?: string) {
  const out = new Map<string, number>()
  for (const e of events) {
    if (e.kind !== 'eliminated' || !e.connector) continue
    if (runId && e.runId !== runId) continue
    const prev = out.get(e.connector)
    if (prev == null || e.atEpochMs < prev) out.set(e.connector, e.atEpochMs)
  }
  return out
}

export function compactAmount(value: number) {
  if (!Number.isFinite(value)) return '0'
  if (Math.abs(value) >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (Math.abs(value) >= 1_000) return `${(value / 1_000).toFixed(0)}k`
  return value.toFixed(0)
}

/** A `<pattern>` id that is safe whatever characters the connector name carries. */
export function hatchId(scope: string, connector: string) {
  return `vc-hatch-${scope}-${connector.replace(/[^a-zA-Z0-9_-]/g, '_')}`
}

/** Per-PSP hatch patterns; mount via `<Customized>` — recharts drops a bare `<defs>` child. */
export function HatchDefs({ entries }: { entries: Array<{ id: string; color: string }> }) {
  return (
    <defs>
      {entries.map(({ id, color }) => (
        <pattern
          key={id}
          id={id}
          patternUnits="userSpaceOnUse"
          width={6}
          height={6}
          patternTransform="rotate(45)"
        >
          <rect width={6} height={6} fill={color} opacity={0.22} />
          <line x1={0} y1={0} x2={0} y2={6} stroke={color} strokeWidth={2} />
        </pattern>
      ))}
    </defs>
  )
}

/** The little swatches the legends and tables use. */
export function SolidSwatch({ color }: { color: string }) {
  return <span className="inline-block h-2.5 w-2.5 rounded-[3px]" style={{ backgroundColor: color }} />
}

export function HatchSwatch({ color }: { color: string }) {
  return (
    <span
      className="inline-block h-2.5 w-2.5 rounded-[3px]"
      style={{
        backgroundColor: `${color}38`,
        backgroundImage: `repeating-linear-gradient(45deg, ${color} 0 1.5px, transparent 1.5px 4px)`,
      }}
    />
  )
}

export function DashSwatch() {
  return (
    <span className="inline-block h-0 w-4 border-t-[1.5px] border-dashed border-slate-600 dark:border-slate-300" />
  )
}

/** Currencies whose minor unit is the major unit — no cents to divide away. */
const ZERO_DECIMAL = new Set(['JPY', 'KRW', 'VND', 'CLP', 'ISK', 'HUF', 'UGX', 'XAF', 'XOF'])

/** Stored minor units → major units for display. */
function toMajor(minor: number, currency: string) {
  return ZERO_DECIMAL.has(currency) ? minor : minor / 100
}

/** The narrow symbol for a currency code, or the code itself when Intl does not know it. */
function currencySymbol(currency: string) {
  try {
    const parts = new Intl.NumberFormat('en', { style: 'currency', currency, currencyDisplay: 'narrowSymbol' })
      .formatToParts(0)
    return parts.find((p) => p.type === 'currency')?.value ?? currency
  } catch {
    return currency
  }
}

/** Minor units → compact money ("$8.0M"); the plain compact number when there is no currency. */
export function formatMoney(minor: number, currency?: string | null) {
  if (!Number.isFinite(minor)) minor = 0
  if (!currency) return compactAmount(minor)
  const major = toMajor(minor, currency)
  const abs = Math.abs(major)
  const symbol = currencySymbol(currency)
  const sign = major < 0 ? '-' : ''
  if (abs >= 1_000_000) return `${sign}${symbol}${(abs / 1_000_000).toFixed(1)}M`
  if (abs >= 10_000) return `${sign}${symbol}${(abs / 1_000).toFixed(0)}k`
  if (abs >= 1_000) return `${sign}${symbol}${(abs / 1_000).toFixed(1)}k`
  return `${sign}${symbol}${abs.toFixed(0)}`
}

/** Full-precision money for a headline figure: 2000000 USD → "$20,000". */
export function formatMoneyExact(minor: number, currency?: string | null) {
  if (!Number.isFinite(minor)) minor = 0
  if (!currency) return Math.round(minor).toLocaleString()
  const major = toMajor(minor, currency)
  try {
    return new Intl.NumberFormat('en', {
      style: 'currency',
      currency,
      currencyDisplay: 'narrowSymbol',
      maximumFractionDigits: 0,
    }).format(major)
  } catch {
    return `${currency} ${Math.round(major).toLocaleString()}`
  }
}
