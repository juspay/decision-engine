import { useMemo, useRef, useState } from 'react'
import {
  ComposedChart, Bar, Line, CartesianGrid, XAxis, YAxis, Tooltip, ResponsiveContainer, Legend,
} from 'recharts'
import { Card, CardBody, SurfaceLabel } from '../ui/Card'

// Pure-frontend tuning illustrator (no routing/config touched). Sizes the SRV3 bucket B
// and per-PG hedge h from a 24-hour volume profile using the derived formulas, and
// contrasts a single static config (sized for the leanest hour) against per-hour automatic
// sizing — surfacing the over-hedge bleed and noise a fixed config leaks.

const SAMPLE_DAY = [
  420, 260, 180, 140, 130, 210, 480, 950, 1700, 2400, 2900, 3200,
  3400, 3300, 3100, 2950, 3050, 3350, 3600, 3450, 2800, 1900, 1100, 640,
]

const round25 = (x: number) => Math.round(x / 25) * 25

// B = max(100, round₂₅(5·√(V·S/(N−1)))), then spread-reduced when D>6pp & B≥200.
function autoBucket(vHour: number, split: number, pgs: number, spread: number): number {
  if (pgs <= 1 || vHour <= 0) return 100
  let b = Math.max(100, round25(5 * Math.sqrt((vHour * split) / (pgs - 1))))
  if (spread > 6 && b >= 200) b = Math.min(b, round25(1875 / (spread * spread)))
  return b
}

// Per-PG hedge h = max(1%, min(B/(V·S), per-PG cap)); total cap = 0.10 + 0.20·S.
function perPgHedge(bucket: number, vHour: number, split: number, pgs: number): number {
  const perPgCap = (0.1 + 0.2 * split) / Math.max(1, pgs - 1)
  const denom = vHour * split
  const raw = denom > 0 ? bucket / denom : perPgCap
  return Math.max(0.01, Math.min(raw, perPgCap))
}

function refreshTone(min: number): string {
  if (!Number.isFinite(min)) return 'text-slate-400'
  if (min <= 60) return 'text-emerald-500'
  if (min <= 120) return 'text-amber-500'
  return 'text-red-500'
}

type ParseResult = { hours: number[]; note: string } | { error: string }

function normalizeVolumes(values: number[]): ParseResult {
  const v = values.filter((n) => Number.isFinite(n) && n >= 0)
  if (v.length === 24) return { hours: v, note: 'Loaded 24 hourly values.' }
  if (v.length > 24 && v.length % 24 === 0) {
    const days = v.length / 24
    const avg = Array.from({ length: 24 }, (_, h) => {
      let s = 0
      for (let d = 0; d < days; d++) s += v[d * 24 + h]
      return s / days
    })
    return { hours: avg, note: `Averaged ${days} days → 24 hourly values.` }
  }
  if (v.length > 24) return { hours: v.slice(0, 24), note: `Using first 24 of ${v.length} values.` }
  return { error: `Expected 24 values — found ${v.length}. Paste one day (row/column) or N×24.` }
}

// Flexible parser: single row, single column, N×24 grid, or a time-column export.
function parseVolumes(text: string): ParseResult {
  const raw = text.trim()
  if (!raw) return { error: 'Paste hourly volumes or upload a CSV.' }
  const lines = raw.split(/\r?\n/).map((l) => l.trim()).filter((l) => /\d/.test(l))
  // Many content lines → one value per line (last numeric token handles "00:00,420").
  if (lines.length >= 12) {
    const vals = lines
      .map((l) => {
        const nums = l.match(/-?\d+(?:\.\d+)?/g)
        return nums ? Number(nums[nums.length - 1]) : NaN
      })
      .filter((n) => Number.isFinite(n))
    return normalizeVolumes(vals)
  }
  // Few lines → flatten every number (single row, or a small N×24 grid).
  return normalizeVolumes((raw.match(/-?\d+(?:\.\d+)?/g) || []).map(Number))
}

// Stream-parse a raw Adyen PAR export (40 cols incl. a quoted JSON field) directly: count
// `Received` rows (= payment attempts, the routing volume) per hour of `Booking Date`,
// grouped by date, then return the single **busiest weekday** (Mon–Fri) — a real day's
// shape, not a weekend-diluted average. Only the two needed columns are accumulated, so it
// stays cheap on large files. Quote-aware so the embedded `ICSF details` JSON doesn't break
// column counting.
async function parsePar(file: File, onProgress: (rows: number) => void): Promise<ParseResult> {
  const reader = file.stream().pipeThrough(new TextDecoderStream()).getReader()
  let inQuotes = false
  let headerDone = false
  let headerAccum: string[] = []
  let field = ''
  let col = 0
  let capture = true // before the header is known, capture every field
  let bdIdx = -1
  let rtIdx = -1
  let isPar = false
  let bdVal = ''
  let rtVal = ''
  const perDay = new Map<string, number[]>() // date → 24 hourly Received counts
  let rows = 0

  const needed = (c: number) => headerDone && (c === bdIdx || c === rtIdx)
  const endField = () => {
    if (!headerDone) headerAccum.push(field)
    else if (col === bdIdx) bdVal = field
    else if (col === rtIdx) rtVal = field
    field = ''
  }
  const endRecord = () => {
    if (!headerDone) {
      bdIdx = headerAccum.indexOf('Booking Date')
      rtIdx = headerAccum.indexOf('Record Type')
      isPar = bdIdx >= 0 && rtIdx >= 0
      headerDone = true
      headerAccum = []
    } else if (isPar) {
      rows++
      if (rtVal === 'Received' && bdVal.length >= 13) {
        const h = (bdVal.charCodeAt(11) - 48) * 10 + (bdVal.charCodeAt(12) - 48)
        if (h >= 0 && h < 24) {
          const date = bdVal.slice(0, 10)
          let arr = perDay.get(date)
          if (!arr) { arr = new Array(24).fill(0); perDay.set(date, arr) }
          arr[h]++
        }
      }
      if (rows % 200000 === 0) onProgress(rows)
    }
    col = 0
    capture = !headerDone || needed(0)
    bdVal = ''
    rtVal = ''
  }

  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    const s = value
    for (let i = 0; i < s.length; i++) {
      const c = s[i]
      if (inQuotes) {
        if (c === '"') {
          if (s[i + 1] === '"') { if (capture) field += '"'; i++ } else inQuotes = false
        } else if (capture) field += c
      } else if (c === '"') {
        inQuotes = true
      } else if (c === ',') {
        endField(); col++; capture = !headerDone || needed(col)
      } else if (c === '\n') {
        endField(); endRecord()
      } else if (c !== '\r' && capture) {
        field += c
      }
    }
  }
  if (field.length || col > 0) { endField(); endRecord() }

  if (!isPar) return { error: 'Not a PAR export (no "Booking Date" / "Record Type" columns).' }
  if (perDay.size === 0) return { error: 'PAR parsed, but found no "Received" rows.' }

  // Pick the busiest weekday (Mon–Fri); fall back to busiest day if the file is weekend-only.
  // Partial boundary days have lower totals, so they don't win — no extra coverage guard needed.
  const WD = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
  const dow = (date: string) => new Date(`${date}T00:00:00Z`).getUTCDay()
  let bestDate = ''
  let bestArr: number[] = new Array(24).fill(0)
  let bestTotal = -1
  let bestIsWeekday = false
  for (const [date, arr] of perDay) {
    const total = arr.reduce((a, b) => a + b, 0)
    const isWeekday = dow(date) >= 1 && dow(date) <= 5
    const better =
      bestTotal < 0 ||
      (isWeekday && !bestIsWeekday) ||
      (isWeekday === bestIsWeekday && total > bestTotal)
    if (better) { bestDate = date; bestArr = arr; bestTotal = total; bestIsWeekday = isWeekday }
  }
  return {
    hours: bestArr.slice(),
    note: `PAR detected — busiest weekday ${bestDate} (${WD[dow(bestDate)]}): ${bestTotal.toLocaleString()} Received attempts (of ${perDay.size} day(s) scanned).`,
  }
}

type OverlayMetric = 'bucket' | 'hedge' | 'noise'

export function BucketHedgingTuner() {
  const [pgs, setPgs] = useState(3)
  // Split defaults to 100% (all traffic through the SR engine); only shown under Advanced
  // for merchants who actually split. Spread and min-vol are kept at fixed internal
  // defaults — they barely move the result (large-spread reduction is an edge case, and
  // there's no quiet-hour cutoff by default), so they're no longer asked for.
  const [splitPct, setSplitPct] = useState(100)
  const spread = 5
  const minVol = 0
  const [volumes, setVolumes] = useState<number[]>(SAMPLE_DAY)
  const [overlay, setOverlay] = useState<OverlayMetric>('bucket')
  const [showTable, setShowTable] = useState(false)
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [parStatus, setParStatus] = useState<string | null>(null)
  const [inputNote, setInputNote] = useState<string>('Sample day')
  const [inputError, setInputError] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)

  const split = Math.min(1, Math.max(0, splitPct / 100))

  const loadSample = () => {
    setVolumes(SAMPLE_DAY)
    setInputNote('Sample day')
    setInputError(null)
    setParStatus(null)
  }

  const model = useMemo(() => {
    const hours = volumes
    const nHedged = Math.max(1, pgs - 1)
    const active = hours.map((v, h) => ({ v, h })).filter(({ v }) => v > 0 && v >= minVol)

    const leanV = active.length ? Math.min(...active.map((a) => a.v)) : 0
    const staticB = active.length
      ? Math.min(...active.map((a) => autoBucket(a.v, split, pgs, spread)))
      : 100
    const staticH = perPgHedge(staticB, leanV || 1, split, pgs)

    const health = (bucket: number, h: number, vHour: number) => {
      const srv3 = vHour * split
      const totalHedge = h * nHedged
      const bestPerHr = srv3 * Math.max(0, 1 - totalHedge)
      const worstPerHr = srv3 * h
      return {
        bucket,
        h,
        noise: Math.sqrt(1875 / bucket),
        bestRefresh: bestPerHr > 0 ? (bucket / bestPerHr) * 60 : Infinity,
        worstRefresh: worstPerHr > 0 ? (bucket / worstPerHr) * 60 : Infinity,
        hedged: srv3 * totalHedge,
      }
    }

    const rows = hours.map((v, h) => {
      const on = v > 0 && v >= minVol
      const aB = on ? autoBucket(v, split, pgs, spread) : staticB
      const aH = on ? perPgHedge(aB, v, split, pgs) : staticH
      return { v, hour: h, on, static: health(staticB, staticH, v), auto: health(aB, aH, v) }
    })

    const activeRows = rows.filter((r) => r.on)
    const sum = (f: (r: (typeof rows)[number]) => number) => activeRows.reduce((a, r) => a + f(r), 0)
    const totalVol = sum((r) => r.v)
    const wNoise = (key: 'static' | 'auto') => (totalVol > 0 ? sum((r) => r.v * r[key].noise) / totalVol : 0)
    const hedgedDay = (key: 'static' | 'auto') => sum((r) => r[key].hedged)
    const autoBuckets = activeRows.map((r) => r.auto.bucket)
    const autoHedges = activeRows.map((r) => r.auto.h)

    const staticHedged = hedgedDay('static')
    const autoHedged = hedgedDay('auto')
    const hedgedSaved = staticHedged - autoHedged
    const recovered = hedgedSaved * (Math.max(0, spread) / 100 / 2)

    const totalDay = hours.reduce((a, b) => a + b, 0)
    const peak = hours.reduce((m, v, h) => (v > m.v ? { v, h } : m), { v: 0, h: 0 })

    return {
      rows,
      staticB,
      staticH,
      autoBMin: autoBuckets.length ? Math.min(...autoBuckets) : staticB,
      autoBMax: autoBuckets.length ? Math.max(...autoBuckets) : staticB,
      autoHMin: autoHedges.length ? Math.min(...autoHedges) : staticH,
      autoHMax: autoHedges.length ? Math.max(...autoHedges) : staticH,
      staticNoise: wNoise('static'),
      autoNoise: wNoise('auto'),
      staticHedged,
      autoHedged,
      hedgedSaved,
      recovered,
      nHedged,
      totalDay,
      avgV: hours.length ? totalDay / hours.length : 0,
      peak,
      activeCount: activeRows.length,
      hoursCount: hours.length,
    }
  }, [volumes, pgs, split, spread, minVol])

  const onFile = async (file: File) => {
    setInputError(null)
    // Sniff the header: a raw PAR export is parsed/crunched directly; anything else is
    // treated as a plain 24-value layout and read as text.
    let isPar = false
    try {
      const head = await file.slice(0, 1 << 16).text()
      const cols = (head.split(/\r?\n/, 1)[0] ?? '').split(',').map((s) => s.trim())
      isPar = cols.includes('Booking Date') && cols.includes('Record Type')
    } catch {
      isPar = false
    }
    if (isPar) {
      setParStatus('Crunching PAR…')
      try {
        const res = await parsePar(file, (r) => setParStatus(`Crunching PAR… ${r.toLocaleString()} rows`))
        if ('hours' in res) {
          setVolumes(res.hours.map((h) => Math.round(h)))
          setInputNote(res.note)
        } else {
          setInputError(res.error)
        }
      } catch {
        setInputError('Could not read the file (too large for the browser? try a smaller slice).')
      } finally {
        setParStatus(null)
      }
      return
    }
    // Non-PAR: small 24-value layout. Guard against accidentally loading a huge file.
    setParStatus(null)
    if (file.size > 2_000_000) {
      setInputError('Unrecognised large file — expected a PAR export or a small 24-value CSV.')
      return
    }
    try {
      const res = parseVolumes(await file.text())
      if ('hours' in res) { setVolumes(res.hours); setInputNote(`Loaded ${file.name}`) }
      else setInputError(res.error)
    } catch {
      setInputError('Could not read the file.')
    }
  }

  const chartData = model.rows.map((r) => ({
    hour: `${String(r.hour).padStart(2, '0')}`,
    volume: r.v,
    staticBucket: r.static.bucket,
    autoBucket: r.auto.bucket,
    staticHedge: r.static.h * 100,
    autoHedge: r.auto.h * 100,
    staticNoise: r.static.noise,
    autoNoise: r.auto.noise,
  }))

  const overlayCfg: Record<OverlayMetric, { label: string; staticKey: string; autoKey: string; unit: string; fmt: (n: number) => string }> = {
    bucket: { label: 'Bucket B', staticKey: 'staticBucket', autoKey: 'autoBucket', unit: '', fmt: (n) => n.toFixed(0) },
    hedge: { label: 'Hedge %/PG', staticKey: 'staticHedge', autoKey: 'autoHedge', unit: '%', fmt: (n) => n.toFixed(1) },
    noise: { label: 'Noise ±pp', staticKey: 'staticNoise', autoKey: 'autoNoise', unit: 'pp', fmt: (n) => n.toFixed(2) },
  }
  const ov = overlayCfg[overlay]

  const numCls =
    'w-full rounded-lg border border-slate-200 bg-slate-50 px-2.5 py-1.5 text-sm font-semibold text-slate-800 focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-100'

  return (
    <div className="space-y-5">
      <Card>
        <CardBody>
          <div className="mb-3">
            <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Auto-tuning — Static vs Automatic</h3>
            {/* <p className="text-[13px] text-slate-400 dark:text-slate-500">
              A fixed bucket size and hedging % are sized for one hour and leak on every other. Feed a day of volume to see what deriving them per-hour saves. Illustration only — does not change routing.
            </p> */}
          </div>

          <div className="flex flex-wrap items-end gap-4">
            <label className="flex flex-col gap-1">
              <SurfaceLabel>Processors (N)</SurfaceLabel>
              <input type="number" min={2} max={12} value={pgs} onChange={(e) => setPgs(Math.max(2, Math.min(12, parseInt(e.target.value || '2', 10))))} className={`${numCls} w-[90px]`} />
            </label>
            <button
              type="button"
              onClick={() => setShowAdvanced((s) => !s)}
              className="self-end pb-1.5 text-xs font-medium text-brand-600 hover:underline dark:text-brand-400"
            >
              {showAdvanced ? '▾ Advanced' : '▸ Advanced'}
            </button>
            {showAdvanced && (
              <label className="flex flex-col gap-1">
                <SurfaceLabel>SRV3 split %</SurfaceLabel>
                <input type="number" min={1} max={100} value={splitPct} onChange={(e) => setSplitPct(Math.max(1, Math.min(100, parseInt(e.target.value || '100', 10))))} className={`${numCls} w-[90px]`} />
              </label>
            )}
          </div>

          {/* Volume input: CSV/PAR upload + sample button */}
          <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-stretch">
            <div
              onClick={() => fileRef.current?.click()}
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => { e.preventDefault(); const f = e.dataTransfer.files?.[0]; if (f) onFile(f) }}
              className="flex flex-1 cursor-pointer flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-slate-300 bg-slate-50/60 px-4 py-4 text-center text-xs text-slate-500 hover:border-brand-400 hover:bg-brand-50/30 dark:border-[#222226] dark:bg-[#0b0b10] dark:text-slate-400"
            >
              <input ref={fileRef} type="file" accept=".csv,.txt,text/csv,text/plain" className="hidden" onChange={(e) => { const f = e.target.files?.[0]; if (f) onFile(f) }} />
              <span className="font-medium text-slate-600 dark:text-slate-300">Drop a CSV or click to browse</span>
              <span>A raw PAR export (auto-detected — busiest weekday), or any 24-value layout — row, column, N×24, time-column.</span>
              {parStatus && <span className="mt-1 font-medium text-amber-600 dark:text-amber-400">{parStatus}</span>}
            </div>
            <button
              type="button"
              onClick={loadSample}
              className="shrink-0 rounded-xl border border-slate-200 px-4 py-2 text-sm font-medium text-slate-600 hover:bg-slate-50 dark:border-[#222226] dark:text-slate-300 dark:hover:bg-[#0d0d13] sm:self-stretch"
            >
              Load sample day
            </button>
          </div>

          <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8d96aa]">
            {inputError ? (
              <span className="text-red-500">{inputError}</span>
            ) : (
              <>
                <span className="text-emerald-600 dark:text-emerald-400">{inputNote}</span>
                {' · '}Total {model.totalDay.toLocaleString()} · avg {model.avgV.toFixed(0)}/hr · peak {model.peak.v.toLocaleString()} @ {model.peak.h}h · active {model.activeCount}/{model.hoursCount}h
              </>
            )}
          </p>
        </CardBody>
      </Card>

      {/* Volume chart with static-vs-auto overlay */}
      <Card>
        <CardBody>
          <div className="mb-2 flex flex-wrap items-start justify-between gap-3">
            <div>
              <h4 className="text-sm font-medium text-slate-800 dark:text-white">Daily volume &amp; {ov.label.toLowerCase()}</h4>
              <p className="text-[13px] text-slate-400 dark:text-slate-500">Bars = traffic. Lines = the static (flat) vs automatic (per-hour) value — watch automatic track the volume.</p>
            </div>
            <div className="inline-flex rounded-md border border-slate-200 p-0.5 dark:border-[#1f1f29]">
              {(['bucket', 'hedge', 'noise'] as OverlayMetric[]).map((m) => (
                <button key={m} type="button" onClick={() => setOverlay(m)} className={`px-2.5 py-1 text-[11px] font-medium rounded transition-colors ${overlay === m ? 'bg-brand-500 text-white' : 'text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200'}`}>
                  {overlayCfg[m].label}
                </button>
              ))}
            </div>
          </div>
          <div className="h-[300px] w-full">
            <ResponsiveContainer width="100%" height="100%">
              <ComposedChart data={chartData} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" className="dark:opacity-20" vertical={false} />
                <XAxis dataKey="hour" tick={{ fontSize: 11, fill: '#94a3b8' }} tickLine={false} axisLine={{ stroke: '#e2e8f0' }} minTickGap={16} />
                <YAxis yAxisId="vol" tick={{ fontSize: 11, fill: '#94a3b8' }} tickLine={false} axisLine={false} width={48} tickFormatter={(v: number) => (v >= 1000 ? `${(v / 1000).toFixed(1)}k` : `${v}`)} />
                <YAxis yAxisId="ov" orientation="right" tick={{ fontSize: 11, fill: '#94a3b8' }} tickLine={false} axisLine={false} width={44} tickFormatter={(v: number) => `${ov.fmt(v)}${ov.unit}`} />
                <Tooltip
                  formatter={(value: number, name: string) => {
                    if (name === 'Volume') return [Math.round(value).toLocaleString(), name]
                    return [`${ov.fmt(value)}${ov.unit}`, name]
                  }}
                  labelFormatter={(h) => `${h}:00`}
                  contentStyle={{ fontSize: 12, borderRadius: 8 }}
                />
                <Legend wrapperStyle={{ fontSize: 11 }} />
                <Bar yAxisId="vol" dataKey="volume" name="Volume" fill="#c7d2fe" radius={[3, 3, 0, 0]} isAnimationActive={false} />
                <Line yAxisId="ov" dataKey={ov.staticKey} name={`Static ${ov.label}`} stroke="#94a3b8" strokeWidth={2} strokeDasharray="5 4" dot={false} isAnimationActive={false} />
                <Line yAxisId="ov" dataKey={ov.autoKey} name={`Auto ${ov.label}`} stroke="#10b981" strokeWidth={2.5} dot={false} isAnimationActive={false} />
              </ComposedChart>
            </ResponsiveContainer>
          </div>
        </CardBody>
      </Card>

      {/* Comparison cards + verdict */}
      <Card>
        <CardBody>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="rounded-xl border border-slate-200 bg-white px-4 py-3 dark:border-[#1c1c24] dark:bg-[#0b0b10]">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-semibold text-slate-600 dark:text-slate-300">Static</span>
                <span className="rounded bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-500 dark:bg-[#1f1f29] dark:text-slate-400">sized for leanest hour</span>
              </div>
              <div className="mt-2 grid grid-cols-2 gap-y-2 text-sm">
                <span className="text-slate-400">Bucket B</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">{model.staticB}</span>
                <span className="text-slate-400">Hedge / PG</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">{(model.staticH * 100).toFixed(1)}% <span className="text-slate-400">(tot {(model.staticH * model.nHedged * 100).toFixed(0)}%)</span></span>
                <span className="text-slate-400">Hedged / day</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">{Math.round(model.staticHedged).toLocaleString()}</span>
                <span className="text-slate-400">Vol-wt noise</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">±{model.staticNoise.toFixed(2)}pp</span>
              </div>
            </div>
            <div className="rounded-xl border border-emerald-200 bg-emerald-50/40 px-4 py-3 dark:border-emerald-900 dark:bg-emerald-950/20">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-semibold text-emerald-600 dark:text-emerald-300">Automatic</span>
                <span className="rounded bg-emerald-100 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-200">per-hour sizing</span>
              </div>
              <div className="mt-2 grid grid-cols-2 gap-y-2 text-sm">
                <span className="text-slate-400">Bucket B</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">{model.autoBMin === model.autoBMax ? model.autoBMin : `${model.autoBMin}–${model.autoBMax}`}</span>
                <span className="text-slate-400">Hedge / PG</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">{(model.autoHMin * 100).toFixed(1)}–{(model.autoHMax * 100).toFixed(1)}%</span>
                <span className="text-slate-400">Hedged / day</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">{Math.round(model.autoHedged).toLocaleString()}</span>
                <span className="text-slate-400">Vol-wt noise</span><span className="text-right font-mono font-semibold text-slate-800 dark:text-slate-100">±{model.autoNoise.toFixed(2)}pp</span>
              </div>
            </div>
          </div>

          <div className="mt-3 rounded-xl border border-slate-200 bg-slate-50/60 px-4 py-2.5 text-sm text-slate-700 dark:border-[#1c1c24] dark:bg-[#0b0b10] dark:text-slate-200">
            Automatic trims <strong className="font-mono">{Math.round(model.hedgedSaved).toLocaleString()}</strong> hedged txns/day
            (≈ <strong className="font-mono text-emerald-600 dark:text-emerald-400">{Math.round(model.recovered).toLocaleString()}</strong> recovered approvals) and cuts volume-weighted noise from
            <strong className="font-mono"> ±{model.staticNoise.toFixed(2)}pp</strong> → <strong className="font-mono text-emerald-600 dark:text-emerald-400">±{model.autoNoise.toFixed(2)}pp</strong> by enlarging buckets in busy hours.
            {' '}Lean-hour refresh is volume-limited, so both stay equal there — the win is the over-hedge trim and busy-hour accuracy.
          </div>

          <button type="button" onClick={() => setShowTable((s) => !s)} className="mt-3 text-xs font-medium text-brand-600 hover:underline dark:text-brand-400">
            {showTable ? 'Hide' : 'Show'} per-hour breakdown
          </button>

          {showTable && (
            <div className="mt-2 overflow-x-auto">
              <table className="w-full text-right text-xs tabular-nums">
                <thead className="text-slate-400">
                  <tr className="border-b border-slate-200 dark:border-[#1c1c24]">
                    <th className="py-1 pr-3 text-left font-medium">Hour</th>
                    <th className="py-1 pr-3 font-medium">Volume</th>
                    <th className="py-1 pr-3 font-medium">B (S→A)</th>
                    <th className="py-1 pr-3 font-medium">h% (S→A)</th>
                    <th className="py-1 pr-3 font-medium">Noise (S→A)</th>
                    <th className="py-1 pr-3 font-medium">Worst refresh (S→A)</th>
                  </tr>
                </thead>
                <tbody className="text-slate-600 dark:text-slate-300">
                  {model.rows.filter((r) => r.on).map((r) => (
                    <tr key={r.hour} className="border-b border-slate-100 dark:border-[#141418]">
                      <td className="py-1 pr-3 text-left font-mono">{String(r.hour).padStart(2, '0')}:00</td>
                      <td className="py-1 pr-3 font-mono">{Math.round(r.v).toLocaleString()}</td>
                      <td className="py-1 pr-3 font-mono">{r.static.bucket} → <span className="text-slate-800 dark:text-slate-100">{r.auto.bucket}</span></td>
                      <td className="py-1 pr-3 font-mono">{(r.static.h * 100).toFixed(1)} → <span className="text-slate-800 dark:text-slate-100">{(r.auto.h * 100).toFixed(1)}</span></td>
                      <td className="py-1 pr-3 font-mono">±{r.static.noise.toFixed(1)} → <span className="text-slate-800 dark:text-slate-100">±{r.auto.noise.toFixed(1)}</span></td>
                      <td className="py-1 pr-3 font-mono">
                        <span className={refreshTone(r.static.worstRefresh)}>{r.static.worstRefresh.toFixed(0)}</span>
                        {' → '}
                        <span className={refreshTone(r.auto.worstRefresh)}>{r.auto.worstRefresh.toFixed(0)}</span>
                        <span className="text-slate-400"> min</span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardBody>
      </Card>
    </div>
  )
}
