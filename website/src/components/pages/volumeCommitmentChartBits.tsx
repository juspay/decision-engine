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

/** True on a `test_minutes` cycle, where a contract day lasts seconds rather than a day. */
export function isTestCycle(daySecs?: number | null) {
  return (daySecs ?? SECS_PER_DAY) < SECS_PER_DAY
}

/**
 * Sub-day buckets for the series: one-second ones on a test cycle, hourly on a calendar one.
 *
 * A test contract day is seconds long, so a bucket is asked for per second of it — the line then
 * moves as the payments land rather than in steps. Bounded by the day's own length, and the
 * server clamps `per_day` besides.
 */
export function bucketsPerDay(daySecs?: number | null) {
  return isTestCycle(daySecs) ? Math.max(1, Math.round(daySecs ?? 0)) : 24
}

/** When each PSP was first eliminated, within `runId` only (an old cycle's drop must not pin day 0). */
export function firstEliminationByConnector(events: CommitmentAuditEvent[], runId?: string) {
  // Where each connector's *standing* elimination began — the first of the unbroken run of
  // forecasts that has been dropping it ever since.
  //
  // A drop can be reversed: traffic comes back in the days that remain, two forecasts agree, and
  // the commitment is chased again. Marking the earliest elimination in the run then draws a
  // verdict the engine has since revised, with the steering it went on to do plotted after it —
  // which is a chart contradicting itself. A forecast that did not drop the connector ends its
  // streak, and any later drop starts a new one.
  const started = new Map<string, number>()
  const inRun = events
    .filter((e) => !runId || e.runId === runId)
    .slice()
    .sort((a, b) => a.atEpochMs - b.atEpochMs)

  for (const e of inRun) {
    if (e.kind === 'eliminated' && e.connector) {
      if (!started.has(e.connector)) started.set(e.connector, e.atEpochMs)
      continue
    }
    // A forecast carries its eliminations at the same instant, so anyone not named among them
    // was being chased at that moment.
    if (e.kind !== 'forecast') continue
    for (const connector of [...started.keys()]) {
      const droppedHere = inRun.some(
        (other) =>
          other.kind === 'eliminated' &&
          other.connector === connector &&
          other.atEpochMs === e.atEpochMs,
      )
      if (!droppedHere) started.delete(connector)
    }
  }
  return started
}

/** Percent of goal for display. Rounding must never contradict the verdict: a shortfall never
 *  reads "100%" — within half a point of the goal it keeps one decimal ("99.6%") — and only an
 *  actually-met goal prints "100%". */
export function pctOfGoal(achieved: number, goal: number): string {
  if (!(goal > 0)) return '0'
  if (achieved >= goal) return '100'
  const pct = Math.max(0, (achieved / goal) * 100)
  if (Math.round(pct) >= 100) return Math.min(99.9, Math.floor(pct * 10) / 10).toFixed(1)
  return Math.round(pct).toString()
}

/** The achieved amount beside its goal. When compact rounding would print a shortfall as the
 *  goal itself ("$100/$100" while missed), the achieved side keeps its minor units instead. */
export function formatAchieved(achieved: number, goal: number, currency?: string | null): string {
  const compact = formatMoney(achieved, currency)
  if (achieved >= goal || compact !== formatMoney(goal, currency)) return compact
  if (!currency) return achieved.toLocaleString()
  const major = toMajorUnits(achieved, currency)
  try {
    return new Intl.NumberFormat('en', {
      style: 'currency',
      currency,
      currencyDisplay: 'narrowSymbol',
      maximumFractionDigits: 2,
    }).format(major)
  } catch {
    return `${currency} ${major.toLocaleString()}`
  }
}

function compactAmount(value: number) {
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

/**
 * Contract amounts → the units payments are actually denominated in.
 *
 * Every figure the volume-commitment API returns is in the document's canonical minor units,
 * while a payment reaches `/decide-gateway` in major ones. Display uses this; so does the
 * simulator, which has to turn a contract's daily rate back into a ticket size it can send.
 * A `metric: volume` contract counts transactions and has no currency, so nothing is converted.
 */
export function toMajorUnits(minor: number, currency?: string | null) {
  if (!currency) return minor
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
  const major = toMajorUnits(minor, currency)
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
  const major = toMajorUnits(minor, currency)
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
