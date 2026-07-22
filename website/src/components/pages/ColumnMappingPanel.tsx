import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertTriangle, ArrowRight, Check, Info } from 'lucide-react'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  previewColumnMapping,
  setColumnMapping,
  type ColumnMapping,
  type PreflightReport,
  type PreviewReport,
} from '../../hooks/useCostRouting'
import { inputClass } from './CostRoutingShared'

/**
 * Column mapping: pair the merchant's report headers with the ones this connector reads, when their
 * export uses different labels for the same data.
 *
 * The layout follows the shape of the problem: **their column on the left, ours on the right**, with
 * a live example value from their own file under each choice — the example is what actually settles
 * "is this the right column?", far more than the label does. Required columns still unmatched come
 * first (that is the work); ones that already line up are collapsed underneath, present for
 * reassurance rather than attention.
 *
 * ## Why this component insists on a preview
 *
 * A mapping is not a cosmetic rename. The fee columns are a decomposition — interchange, scheme,
 * markup and commission sum into `total_fee`, which the fit regresses against `gross` to produce the
 * price the router serves. A merchant scanning a dropdown will reasonably map their `Fee` column
 * onto `Commission (SC)` because the names look close; if that column is actually the all-in fee,
 * nothing errors. It parses, it fits, it grades GOOD, and it silently misprices real routing
 * decisions.
 *
 * The server can reject a *malformed* mapping (unknown column, absent target, two columns sharing
 * one source) but cannot possibly know a well-formed one is semantically wrong. So this panel will
 * not enable Save until the merchant has seen a preview of what their mapping produces — the derived
 * gross, fee and effective rate, not just the pairing — and it surfaces the server's warning
 * prominently when those numbers don't look like card processing. Loud friction here is much cheaper
 * than a quiet wrong cost model.
 */
export function ColumnMappingPanel({
  merchantId,
  connector,
  account,
  preflight,
  sampleText,
  truncated,
  initialMapping,
  onSaved,
  onCancel,
}: {
  merchantId: string
  connector: string
  account: string
  preflight: PreflightReport
  /** The leading bytes of the merchant's file, as text — the same sample preflight ran on. */
  sampleText: string
  /**
   * Whether `sampleText` is the head of a bigger file. Passed through to the server rather than
   * left for it to infer from the sample's byte length: text decoding can shift that length off the
   * cap, and a wrong answer silently disables the server's handling of groups the cut left
   * half-assembled — which then shows a correct mapping as a bogus 0% fee warning.
   */
  truncated: boolean
  initialMapping?: ColumnMapping
  onSaved: () => void
  onCancel: () => void
}) {
  const [columns, setColumns] = useState<ColumnMapping>(initialMapping ?? {})
  const [preview, setPreview] = useState<PreviewReport | null>(null)
  const [previewing, setPreviewing] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [showMatched, setShowMatched] = useState(false)

  // Example values come from parsing the sample in the browser — the merchant's file is already
  // here, so asking the server for values it would have to be sent anyway would be a wasted trip.
  const examples = useMemo(
    () => exampleValues(sampleText, preflight.found),
    [sampleText, preflight.found],
  )

  // Prefill obvious pairings. A suggestion is only ever a starting point: it is shown as a
  // pre-selected dropdown the merchant can override, never applied silently, and it still has to
  // survive the preview before it can be saved.
  useEffect(() => {
    if (initialMapping && Object.keys(initialMapping).length > 0) return
    const taken = new Set<string>()
    const guesses: ColumnMapping = {}
    for (const expected of preflight.missing) {
      const candidate = bestGuess(expected, preflight.found, taken)
      if (candidate) {
        guesses[expected] = candidate
        taken.add(candidate)
      }
    }
    if (Object.keys(guesses).length > 0) setColumns((c) => ({ ...guesses, ...c }))
  }, [preflight.missing, preflight.found, initialMapping])

  const stillMissing = preflight.missing.filter((c) => !columns[c])
  const complete = stillMissing.length === 0

  // Any edit invalidates the preview: what was approved is no longer what would be saved.
  const setColumn = useCallback((expected: string, theirs: string) => {
    setPreview(null)
    setError(null)
    setColumns((c) => {
      const next = { ...c }
      if (theirs) next[expected] = theirs
      else delete next[expected]
      return next
    })
  }, [])

  // A source column may only feed one expected column — mapping one column into two fee components
  // would double-count it into `total_fee`. The server enforces this too; doing it here as well
  // means the merchant sees it as an unavailable option rather than a rejected save.
  const usedSources = new Set(Object.values(columns))

  async function handlePreview() {
    setPreviewing(true)
    setError(null)
    try {
      setPreview(
        await previewColumnMapping(merchantId, connector, columns, sampleText, truncated),
      )
    } catch (e: unknown) {
      setPreview(null)
      setError(e instanceof Error ? e.message : 'Could not preview this mapping')
    } finally {
      setPreviewing(false)
    }
  }

  async function handleSave() {
    setSaving(true)
    setError(null)
    try {
      await setColumnMapping(merchantId, connector, account, columns, sampleText, truncated)
      onSaved()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Could not save this mapping')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-4 rounded-lg border border-amber-200 bg-amber-50/50 p-4 dark:border-amber-900/40 dark:bg-amber-950/20">
      <div className="flex items-start gap-2">
        <AlertTriangle size={16} className="mt-0.5 shrink-0 text-amber-500" />
        <div>
          <h3 className="font-medium text-slate-800 dark:text-white">
            This file's columns don't match {connector}
          </h3>
          <p className="mt-1 text-sm text-slate-600 dark:text-[#9ca7ba]">
            {preflight.missing.length} required{' '}
            {preflight.missing.length === 1 ? 'column is' : 'columns are'} missing. Map them to the
            matching columns in your file below — we'll remember this mapping for future{' '}
            {connector} / {account} reports.
          </p>
        </div>
      </div>

      {/* The wrong connector is a far more common cause than drifted labels, and mapping would
          paper over it rather than fix it — so this is offered before the mapping work, not after. */}
      {preflight.suggested_connectors.length > 0 && (
        <div className="flex items-start gap-2 rounded-lg border border-blue-200 bg-blue-50 p-3 dark:border-blue-900/40 dark:bg-blue-950/30">
          <Info size={15} className="mt-0.5 shrink-0 text-blue-500" />
          <p className="text-sm text-blue-800 dark:text-blue-300">
            This file matches all{' '}
            {preflight.suggested_connectors[0].matched_required} columns of{' '}
            <strong>{preflight.suggested_connectors[0].connector}</strong>. If it's a{' '}
            {preflight.suggested_connectors[0].connector} report, switch the connector above instead
            of mapping columns — mapping it as {connector} would produce a wrong cost model.
          </p>
        </div>
      )}

      {/* Unmatched required columns — the actual work. */}
      <div className="space-y-2">
        <div className="grid grid-cols-[1fr_auto_1fr] gap-3 px-1 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]">
          <span>Your column</span>
          <span />
          <span>Maps to ({connector})</span>
        </div>

        {preflight.missing.map((expected) => {
          const selected = columns[expected] ?? ''
          const example = selected ? examples[selected] : undefined
          return (
            <div
              key={expected}
              className="grid grid-cols-[1fr_auto_1fr] items-center gap-3 rounded-lg border border-slate-200 bg-white p-3 dark:border-[#232833] dark:bg-[#0b1017]"
            >
              <div className="min-w-0">
                <select
                  className={inputClass}
                  value={selected}
                  onChange={(e) => setColumn(expected, e.target.value)}
                >
                  <option value="">— Select your column —</option>
                  {preflight.found.map((theirs) => (
                    <option
                      key={theirs}
                      value={theirs}
                      // Already feeding a different expected column.
                      disabled={usedSources.has(theirs) && theirs !== selected}
                    >
                      {theirs}
                    </option>
                  ))}
                </select>
                {/* The example is what actually resolves "is this the right column?". */}
                <p className="mt-1 truncate text-xs text-slate-400" title={example}>
                  {selected
                    ? example
                      ? `e.g. ${example}`
                      : 'no example value in this file'
                    : ' '}
                </p>
              </div>

              <ArrowRight size={15} className="shrink-0 text-slate-300 dark:text-slate-600" />

              <div className="min-w-0">
                <div className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm font-medium text-slate-700 dark:border-[#232833] dark:bg-[#141a24] dark:text-[#c7cfdd]">
                  {expected}
                </div>
                <p className="mt-1 text-xs text-amber-600 dark:text-amber-500">Required</p>
              </div>
            </div>
          )
        })}
      </div>

      {/* Optional columns the file also lacks. These do not fail an ingestion, which is exactly why
          they need surfacing — their absence is silent, but it degrades the model (a missing
          terminal id makes every transaction look online; a missing booking date loses the report's
          period). Presented as clearly not-required so they never compete with the real work. */}
      {preflight.optional_missing.length > 0 && (
        <details className="rounded-lg border border-slate-200 bg-white p-3 dark:border-[#232833] dark:bg-[#0b1017]">
          <summary className="cursor-pointer text-sm text-slate-600 dark:text-[#9ca7ba]">
            {preflight.optional_missing.length} optional{' '}
            {preflight.optional_missing.length === 1 ? 'column is' : 'columns are'} also missing —
            not required, but they improve the model
          </summary>
          <div className="mt-3 space-y-2">
            {preflight.optional_missing.map((expected) => {
              const selected = columns[expected] ?? ''
              const example = selected ? examples[selected] : undefined
              return (
                <div
                  key={expected}
                  className="grid grid-cols-[1fr_auto_1fr] items-center gap-3 rounded-lg border border-slate-100 p-2 dark:border-[#1a1f29]"
                >
                  <div className="min-w-0">
                    <select
                      className={inputClass}
                      value={selected}
                      onChange={(e) => setColumn(expected, e.target.value)}
                    >
                      <option value="">— Not in my file —</option>
                      {preflight.found.map((theirs) => (
                        <option
                          key={theirs}
                          value={theirs}
                          disabled={usedSources.has(theirs) && theirs !== selected}
                        >
                          {theirs}
                        </option>
                      ))}
                    </select>
                    <p className="mt-1 truncate text-xs text-slate-400" title={example}>
                      {selected && example ? `e.g. ${example}` : ' '}
                    </p>
                  </div>
                  <ArrowRight size={15} className="shrink-0 text-slate-300 dark:text-slate-600" />
                  <div className="min-w-0">
                    <div className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-600 dark:border-[#232833] dark:bg-[#141a24] dark:text-[#9ca7ba]">
                      {expected}
                    </div>
                    <p className="mt-1 text-xs text-slate-400">Optional</p>
                  </div>
                </div>
              )
            })}
          </div>
        </details>
      )}

      {/* Already-matched required columns: reassurance, not attention. Collapsed by default. */}
      {preflight.matched.length > 0 && (
        <div>
          <button
            type="button"
            onClick={() => setShowMatched((s) => !s)}
            className="flex items-center gap-1.5 text-sm text-slate-500 hover:text-slate-700 dark:text-[#9ca7ba] dark:hover:text-white"
          >
            <Check size={14} className="text-emerald-500" />
            {preflight.matched.length} required{' '}
            {preflight.matched.length === 1 ? 'column' : 'columns'} already matched
            <span className="text-xs">{showMatched ? '▲' : '▼'}</span>
          </button>
          {showMatched && (
            <ul className="mt-2 grid gap-1 sm:grid-cols-2">
              {preflight.matched.map((c) => (
                <li
                  key={c}
                  className="flex items-center gap-2 rounded-md bg-white px-2.5 py-1.5 text-xs text-slate-600 dark:bg-[#0b1017] dark:text-[#9ca7ba]"
                >
                  <Check size={12} className="shrink-0 text-emerald-500" />
                  <span className="truncate">{c}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button variant="secondary" onClick={handlePreview} disabled={!complete || previewing}>
          {previewing ? (
            <>
              <Spinner size={14} />
              Checking…
            </>
          ) : (
            'Preview result'
          )}
        </Button>
        <Button onClick={handleSave} disabled={!preview || saving}>
          {saving ? (
            <>
              <Spinner size={14} />
              Saving…
            </>
          ) : (
            'Save mapping'
          )}
        </Button>
        <button
          type="button"
          onClick={onCancel}
          className="text-sm text-slate-500 hover:text-slate-700 dark:text-[#9ca7ba] dark:hover:text-white"
        >
          Cancel
        </button>
        <span className="text-xs text-slate-400">
          {!complete
            ? `${stillMissing.length} still to map`
            : !preview
              ? 'Preview your mapping before saving'
              : 'Applied to this and future reports for this account'}
        </span>
      </div>

      <ErrorMessage error={error} />

      {preview && <MappingPreview preview={preview} />}
    </div>
  )
}

/**
 * What the mapping actually produces. `Gross`, `Fee` and `Effective` are **derived**, not columns
 * copied across — they are the values the fit consumes, so they are where a plausible-looking but
 * wrong mapping gives itself away.
 */
function MappingPreview({ preview }: { preview: PreviewReport }) {
  return (
    <div className="space-y-2 rounded-lg border border-slate-200 bg-white p-3 dark:border-[#232833] dark:bg-[#0b1017]">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium text-slate-700 dark:text-[#c7cfdd]">
          Result of this mapping
        </h4>
        {preview.median_effective_pct != null && (
          <span className="text-xs text-slate-500 dark:text-[#9ca7ba]">
            median effective fee{' '}
            <strong className="text-slate-700 dark:text-white">
              {preview.median_effective_pct.toFixed(3)}%
            </strong>
          </span>
        )}
      </div>

      {preview.warning && (
        <div className="flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 p-3 dark:border-red-900/40 dark:bg-red-950/30">
          <AlertTriangle size={15} className="mt-0.5 shrink-0 text-red-500" />
          <p className="text-sm text-red-700 dark:text-red-400">{preview.warning}</p>
        </div>
      )}

      {preview.rows.length > 0 ? (
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="text-slate-400">
              <tr>
                <th className="py-1 pr-3 font-medium">Network</th>
                <th className="py-1 pr-3 font-medium">Currency</th>
                <th className="py-1 pr-3 text-right font-medium">Gross</th>
                <th className="py-1 pr-3 text-right font-medium">Fee</th>
                <th className="py-1 text-right font-medium">Effective</th>
              </tr>
            </thead>
            <tbody className="text-slate-600 dark:text-[#c7cfdd]">
              {preview.rows.map((r, i) => (
                <tr key={i} className="border-t border-slate-100 dark:border-[#1a1f29]">
                  <td className="py-1 pr-3">{r.card_network || '—'}</td>
                  <td className="py-1 pr-3">{r.currency || '—'}</td>
                  <td className="py-1 pr-3 text-right tabular-nums">{r.gross.toFixed(2)}</td>
                  <td className="py-1 pr-3 text-right tabular-nums">{r.total_fee.toFixed(2)}</td>
                  <td className="py-1 text-right tabular-nums">{r.effective_pct.toFixed(3)}%</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="mt-2 text-xs text-slate-400">
            Gross, Fee and Effective are derived from the columns you mapped — check they look right
            for your business before saving.
          </p>
        </div>
      ) : (
        <p className="text-sm text-slate-500 dark:text-[#9ca7ba]">
          No rows were produced from this file sample.
        </p>
      )}
    </div>
  )
}

/**
 * One example value per header label, read from the first data row of the sample.
 *
 * Locates the header row by matching the labels the server reported rather than assuming line 1 —
 * some reports (J.P. Morgan's preset exports) are wrapped in a `BEGIN` / metadata envelope that the
 * server's parser strips, so the header is not the first line of the raw file the browser holds.
 */
function exampleValues(sampleText: string, found: string[]): Record<string, string> {
  const out: Record<string, string> = {}
  if (found.length === 0) return out

  const lines = sampleText.split(/\r?\n/)
  const headerAt = lines.findIndex((line) => {
    const cells = parseCsvLine(line)
    return cells.length === found.length && cells[0] === found[0]
  })
  if (headerAt < 0) return out

  const values = parseCsvLine(lines[headerAt + 1] ?? '')
  found.forEach((label, i) => {
    const v = (values[i] ?? '').trim()
    if (v) out[label] = v.length > 40 ? `${v.slice(0, 40)}…` : v
  })
  return out
}

/** Minimal RFC4180-ish split: comma-separated, double-quoted fields, `""` as an escaped quote. */
function parseCsvLine(line: string): string[] {
  const cells: string[] = []
  let cur = ''
  let quoted = false
  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (quoted) {
      if (ch === '"') {
        if (line[i + 1] === '"') {
          cur += '"'
          i++
        } else quoted = false
      } else cur += ch
    } else if (ch === '"') quoted = true
    else if (ch === ',') {
      cells.push(cur)
      cur = ''
    } else cur += ch
  }
  cells.push(cur)
  // Strip a leading UTF-8 BOM so the first label compares equal to the server's.
  if (cells.length > 0) cells[0] = cells[0].replace(/^﻿/, '')
  return cells
}

/**
 * Best guess at which of the merchant's columns corresponds to `expected`, or `undefined` when
 * nothing scores well enough.
 *
 * Scoring is deliberately conservative — token overlap, requiring a clear majority of the expected
 * label's words to appear. A missing suggestion costs one dropdown selection; a confident wrong one
 * is likely to be accepted without much thought, and that is the failure this whole flow exists to
 * avoid.
 */
function bestGuess(
  expected: string,
  found: string[],
  taken: Set<string>,
): string | undefined {
  const want = tokens(expected)
  if (want.length === 0) return undefined

  let best: string | undefined
  let bestScore = 0
  for (const candidate of found) {
    if (taken.has(candidate)) continue
    const have = new Set(tokens(candidate))
    const overlap = want.filter((t) => have.has(t)).length
    const score = overlap / want.length
    if (score > bestScore) {
      bestScore = score
      best = candidate
    }
  }
  // Over half the expected label's words must be present. `Settlement Currency` ↔ `Settlement Ccy`
  // clears it; `Markup (SC)` ↔ `Commission (SC)` does not, which is the point.
  return bestScore > 0.5 ? best : undefined
}

/** Lowercased word tokens, dropping punctuation and unit suffixes like `(SC)` that carry no meaning. */
function tokens(label: string): string[] {
  return label
    .toLowerCase()
    .replace(/\(.*?\)/g, ' ')
    .split(/[^a-z0-9]+/)
    .filter((t) => t.length > 1)
}
