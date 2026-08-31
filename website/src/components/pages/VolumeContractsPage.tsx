import { useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import useSWR, { useSWRConfig } from 'swr'
import {
  ArrowLeft,
  ChevronDown,
  ChevronRight,
  Plus,
  PowerOff,
  SlidersHorizontal,
  Trash2,
  Zap,
} from 'lucide-react'
import { apiPost } from '../../lib/api'
import { useMerchantStore } from '../../store/merchantStore'
import { useCanEditRouting } from '../../store/authStore'
import type {
  CreateRoutingRequest,
  RoutingAlgorithm,
  VolumeContract,
  VolumeContractConfig,
  VolumeContractReward,
  VolumeContractTier,
} from '../../types/api'
import { Card, CardBody, InsetPanel } from '../ui/Card'
import { Button } from '../ui/Button'
import { PageHeading } from '../ui/PageHeading'
import { HeaderFilter, HeaderSearch, RowMenu } from '../ui/TableControls'
import { parseBackendTimestamp } from '../../lib/routingRuleTimestamps'
import { formatMoney } from './volumeCommitmentChartBits'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { SearchableSelect } from '../ui/SearchableSelect'
import { Combobox } from '../ui/Combobox'
import * as type from '../ui/typography'

// Matches the input styling used across the routing config pages (SRRoutingPage).
const inputClass =
  'w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]'

const CONNECTOR_SUGGESTIONS = [
  'adyen', 'stripe', 'checkout', 'worldpay', 'braintree', 'paypal', 'chase',
  'cybersource', 'globalpay', 'nuvei', 'rapyd', 'fiserv', 'authorizedotnet',
]

const CURRENCY_SUGGESTIONS = [
  'USD', 'EUR', 'GBP', 'INR', 'JPY', 'AUD', 'CAD', 'SGD', 'AED', 'BHD', 'KWD',
]

const TIMEZONE_SUGGESTIONS = [
  'UTC', 'America/New_York', 'America/Chicago', 'America/Los_Angeles',
  'Europe/London', 'Europe/Amsterdam', 'Europe/Berlin', 'Asia/Kolkata',
  'Asia/Singapore', 'Asia/Tokyo', 'Australia/Sydney',
]

const ANCHOR_HELP: Record<string, string> = {
  calendar_month: 'Day of month the cycle starts (1–30)',
  calendar_quarter: 'Month within the quarter the cycle starts (1–3)',
  calendar_year: 'Month the cycle starts (1–12)',
  test_minutes: 'Cycle length in minutes (2–240) — one minute per contract day',
}

interface TierForm {
  kind: 'retroactive' | 'marginal'
  bps: string
  threshold: string
  targeted: boolean
  rebateLagDays: string
  rebateSettlement: 'cash' | 'credit_note'
}

interface ContractForm {
  key: number
  id: string
  connector: string
  status: 'active' | 'inactive'
  cycleType: 'calendar_month' | 'calendar_quarter' | 'calendar_year' | 'test_minutes'
  anchor: string
  timezone: string
  archetype: 'lumpsum' | 'tiered'
  // lumpsum
  target: string
  rewardKind: 'flat' | 'percentage'
  flatAmount: string
  rebateBps: string
  // tiered
  tiers: TierForm[]
}

let contractKeySeq = 1

function emptyTier(targeted: boolean): TierForm {
  return { kind: 'retroactive', bps: '', threshold: '', targeted, rebateLagDays: '0', rebateSettlement: 'cash' }
}

function emptyContract(): ContractForm {
  return {
    key: contractKeySeq++,
    id: '',
    connector: '',
    status: 'active',
    cycleType: 'calendar_month',
    anchor: '1',
    timezone: 'UTC',
    archetype: 'lumpsum',
    target: '',
    rewardKind: 'flat',
    flatAmount: '',
    rebateBps: '',
    tiers: [emptyTier(true)],
  }
}

// Amounts travel as strings so decimals survive ("6000000.50"); the backend
// canonicalizes them to integer minor units.
function amount(value: string): string {
  return value.trim()
}

type StatusFilter = 'all' | 'active' | 'inactive'

/**
 * When the document was written. Deliberately `created_at`, not `modified_at`: activation stamps
 * `modified_at` (test cycles anchor to it), so "last modified" would read as "last activated".
 */
function formatCreated(doc: RoutingAlgorithm) {
  const ms = doc.created_at ? parseBackendTimestamp(doc.created_at) : 0
  if (!ms) return null
  const date = new Date(ms)
  return {
    date: date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' }),
    time: date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }),
    full: date.toLocaleString(undefined, { dateStyle: 'full', timeStyle: 'medium' }),
  }
}

/** What a PSP promised and earns, in words. Stored amounts are minor units. */
type CommitmentLine = { connector: string; promise: string; reward: string; status: 'active' | 'inactive' }

function commitmentLines(config: VolumeContractConfig | undefined): CommitmentLine[] {
  if (!config?.volume_contracts?.length) return []
  const currency = config.metric === 'volume' ? null : config.currency?.denomination
  const money = (value: unknown) => formatMoney(Number(value ?? 0), currency)
  const rewardText = (reward: VolumeContractReward) =>
    reward.kind === 'flat' ? `${money(reward.value.flat_amount)} flat` : `${reward.value.rebate_bps / 100}% rebate`
  return config.volume_contracts.map((c) => {
    const status = c.status ?? 'active'
    if (c.archetype === 'lumpsum') {
      return { connector: c.connector, promise: money(c.terms.target), reward: rewardText(c.terms.reward), status }
    }
    if (c.archetype === 'tiered') {
      const targeted = c.terms.tiers.find((t) => t.targeted) ?? c.terms.tiers[0]
      const rate = targeted ? ('rebate_bps' in targeted.rate ? targeted.rate.rebate_bps : targeted.rate.rate_bps) : 0
      return {
        connector: c.connector,
        promise: targeted ? money(targeted.threshold) : '—',
        reward: `${rate / 100}% ${targeted?.kind === 'marginal' ? 'marginal' : 'retroactive'} · ${c.terms.tiers.length} tier${c.terms.tiers.length === 1 ? '' : 's'}`,
        status,
      }
    }
    return { connector: c.connector, promise: money(c.terms.floor), reward: rewardText(c.terms.reward), status }
  })
}

/** Document-level settings as label/value pairs for the detail panel. */
function documentSettings(config: VolumeContractConfig) {
  const currency = config.metric === 'volume' ? null : config.currency?.denomination
  const toleranceBps = config.tolerance_bps ?? (config.tolerance ? parseFloat(config.tolerance) : undefined)
  return [
    { label: 'Routing mode', value: config.routing_mode === 'pace_guarded' ? 'Pace‑guarded (auth‑rate first)' : 'Volume‑commitment first' },
    { label: 'Tolerance', value: toleranceBps != null ? `${(toleranceBps / 100).toFixed(toleranceBps % 100 ? 2 : 0)} pp` : '—' },
    { label: 'Metric', value: config.metric === 'volume' ? 'Transaction count' : 'GMV' },
    { label: 'Currency', value: config.currency?.denomination ?? '—' },
    { label: 'Expected daily traffic', value: formatMoney(Number(config.expected_daily_traffic ?? 0), currency) },
    { label: 'Forecast interval', value: config.forecast_interval_secs ? `${config.forecast_interval_secs}s` : 'default' },
    { label: 'Steering interval', value: config.steering_interval_secs ? `${config.steering_interval_secs}s` : 'default' },
    { label: 'Billing cycle', value: summarizeCycle(config) },
  ]
}

function cycleWords(cycle: VolumeContract['billing_cycle']) {
  switch (cycle.type) {
    case 'test_minutes':
      return `${cycle.anchor}-minute test cycle`
    case 'calendar_month':
      return `Monthly from day ${cycle.anchor}`
    case 'calendar_quarter':
      return `Quarterly from month ${cycle.anchor}`
    case 'calendar_year':
      return `Yearly from month ${cycle.anchor}`
    default:
      return String(cycle.type)
  }
}

/**
 * What an expanded row shows: the document's settings as a compact spec strip, one row per PSP
 * commitment, and the raw JSON only on request.
 */
function ContractDocumentDetail({ config }: { config: VolumeContractConfig | undefined }) {
  const [showJson, setShowJson] = useState(false)
  if (!config) return <p className="text-sm text-slate-500">This document has no readable configuration.</p>
  const currency = config.metric === 'volume' ? null : config.currency?.denomination
  const money = (value: unknown) => formatMoney(Number(value ?? 0), currency)
  const lines = commitmentLines(config)
  return (
    <div className="space-y-4">
      <dl className="grid grid-cols-2 gap-x-6 gap-y-3 sm:grid-cols-4 lg:grid-cols-8">
        {documentSettings(config).map((item) => (
          <div key={item.label} className="min-w-0">
            <dt className="text-[11px] font-medium uppercase tracking-wide text-slate-400 dark:text-[#78849a]">{item.label}</dt>
            <dd className="mt-0.5 truncate text-sm text-slate-800 dark:text-slate-200" title={item.value}>{item.value}</dd>
          </div>
        ))}
      </dl>

      <div className="overflow-hidden rounded-xl border border-slate-200 dark:border-[#1e2330]">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-slate-100 bg-slate-50/70 text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:border-[#1e2330] dark:bg-[#11151d] dark:text-[#78849a]">
              <th className="px-4 py-2.5">PSP</th>
              <th className="px-4 py-2.5">Promise</th>
              <th className="px-4 py-2.5">Reward</th>
              <th className="px-4 py-2.5">Cycle</th>
              <th className="px-4 py-2.5">Timezone</th>
              <th className="px-4 py-2.5">Status</th>
            </tr>
          </thead>
          <tbody>
            {config.volume_contracts.map((c, i) => {
              const line = lines[i]
              return (
                <tr key={c.id || c.connector} className="border-b border-slate-100 last:border-b-0 dark:border-[#1e2330]">
                  <td className="px-4 py-2.5 font-medium text-slate-900 dark:text-white">{c.connector}</td>
                  <td className="px-4 py-2.5 tabular-nums text-slate-800 dark:text-slate-200">{line?.promise}</td>
                  <td className="px-4 py-2.5 text-slate-800 dark:text-slate-200">
                    {line?.reward}
                    {c.archetype === 'tiered' && (
                      <span className="mt-1 block text-xs text-slate-500 dark:text-[#8d96a8]">
                        {c.terms.tiers.map((t) => `${money(t.threshold)} → ${('rebate_bps' in t.rate ? t.rate.rebate_bps : t.rate.rate_bps) / 100}%${t.targeted ? ' (target)' : ''}`).join(' · ')}
                      </span>
                    )}
                  </td>
                  <td className="px-4 py-2.5 text-slate-800 dark:text-slate-200">{cycleWords(c.billing_cycle)}</td>
                  <td className="px-4 py-2.5 text-slate-600 dark:text-[#8d96a8]">{c.billing_cycle.timezone}</td>
                  <td className="px-4 py-2.5">
                    <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-semibold leading-4 ${
                      (c.status ?? 'active') === 'active'
                        ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400'
                        : 'bg-slate-100 text-slate-500 dark:bg-[#1a1f2a] dark:text-[#8090a8]'
                    }`}>
                      {(c.status ?? 'active') === 'active' ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      <div>
        <button
          type="button"
          onClick={() => setShowJson((v) => !v)}
          className="text-xs font-medium text-slate-500 transition-colors hover:text-brand-600 dark:text-[#8d96a8] dark:hover:text-brand-400"
        >
          {showJson ? 'Hide JSON' : 'Show JSON'}
        </button>
        {showJson && (
          <pre className="mt-2 max-h-80 overflow-auto rounded-lg border border-slate-200 bg-white p-3 text-[11px] leading-relaxed text-slate-600 dark:border-[#1e2330] dark:bg-[#0a0d12] dark:text-[#9ca7ba]">
            {JSON.stringify(config, null, 2)}
          </pre>
        )}
      </div>
    </div>
  )
}

/** The billing cycle in words, from the first contract — documents share one in practice. */
function summarizeCycle(config: VolumeContractConfig | undefined) {
  const cycle = config?.volume_contracts?.[0]?.billing_cycle
  if (!cycle) return '—'
  switch (cycle.type) {
    case 'test_minutes':
      return `${cycle.anchor}-minute test cycle`
    case 'calendar_month':
      return `Monthly from day ${cycle.anchor} (${cycle.timezone})`
    case 'calendar_quarter':
      return `Quarterly from month ${cycle.anchor} (${cycle.timezone})`
    case 'calendar_year':
      return `Yearly from month ${cycle.anchor} (${cycle.timezone})`
    default:
      return String(cycle.type)
  }
}

/**
 * The contract editor. `embedded` drops the page heading so it can sit as a tab on the Multi
 * Objective Routing page, where that page already supplies the title; everything else — data,
 * validation, activation — is identical either way.
 */
export function VolumeContractsPage({ embedded = false }: { embedded?: boolean } = {}) {
  const merchantId = useMerchantStore((s) => s.merchantId)
  const canEditRouting = useCanEditRouting()
  const { mutate: mutateCache } = useSWRConfig()

  // ── Document list ───────────────────────────────────────────────────────────
  const { data: algorithms, isLoading } = useSWR(
    merchantId ? `/routing/list/${merchantId}` : null,
    (url: string) => apiPost<RoutingAlgorithm[]>(url),
    { revalidateOnFocus: false }
  )
  const { data: activeAlgorithms } = useSWR(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    (url: string) => apiPost<RoutingAlgorithm[]>(url),
    { revalidateOnFocus: false }
  )

  const documents = useMemo(
    () =>
      (algorithms ?? [])
        .filter((algo) => algo.algorithm_for === 'volume_commitment')
        .sort((a, b) => new Date(b.created_at ?? 0).getTime() - new Date(a.created_at ?? 0).getTime()),
    [algorithms]
  )
  const activeDocumentId = useMemo(
    () => (activeAlgorithms ?? []).find((algo) => algo.algorithm_for === 'volume_commitment')?.id ?? null,
    [activeAlgorithms]
  )

  function revalidate() {
    mutateCache(`/routing/list/${merchantId}`)
    mutateCache(`/routing/list/active/${merchantId}`)
  }

  // ── Builder state ───────────────────────────────────────────────────────────
  const [docName, setDocName] = useState('')
  const [docDesc, setDocDesc] = useState('')
  const [tolerancePp, setTolerancePp] = useState('5')
  const [currency, setCurrency] = useState('USD')
  const [amountUnits, setAmountUnits] = useState<'major' | 'minor'>('major')
  const [expectedDailyTraffic, setExpectedDailyTraffic] = useState('')
  const [forecastInterval, setForecastInterval] = useState('')
  const [steeringInterval, setSteeringInterval] = useState('')
  const [contracts, setContracts] = useState<ContractForm[]>([emptyContract()])

  // ── Merchant-level settings live outside the document builder ────────────────────────────
  // Edited on their own screen and kept per merchant in this browser; every new document is
  // stamped with them (the engine still reads them from the document). Seeded from the active
  // document the first time, so an existing setup carries over.
  type MerchantSettings = {
    tolerancePp: string
    currency: string
    amountUnits: 'major' | 'minor'
    expectedDailyTraffic: string
    forecastInterval: string
    steeringInterval: string
  }
  const settingsKey = merchantId ? `vc_merchant_settings_${merchantId}` : null
  const activeConfig = useMemo(() => {
    const doc = documents.find((d) => d.id === activeDocumentId)
    return (doc?.algorithm_data ?? doc?.algorithm)?.data as VolumeContractConfig | undefined
  }, [documents, activeDocumentId])
  function applySettings(next: Partial<MerchantSettings>) {
    if (next.tolerancePp != null) setTolerancePp(next.tolerancePp)
    if (next.currency != null) setCurrency(next.currency)
    if (next.amountUnits) setAmountUnits(next.amountUnits)
    if (next.expectedDailyTraffic != null) setExpectedDailyTraffic(next.expectedDailyTraffic)
    if (next.forecastInterval != null) setForecastInterval(next.forecastInterval)
    if (next.steeringInterval != null) setSteeringInterval(next.steeringInterval)
  }
  useEffect(() => {
    if (!settingsKey) return
    let stored: Partial<MerchantSettings> | null = null
    try {
      const raw = window.localStorage.getItem(settingsKey)
      stored = raw ? (JSON.parse(raw) as Partial<MerchantSettings>) : null
    } catch {
      stored = null
    }
    if (stored) {
      applySettings(stored)
      return
    }
    if (activeConfig) {
      const toleranceBps = activeConfig.tolerance_bps ?? (activeConfig.tolerance ? parseFloat(activeConfig.tolerance) : undefined)
      applySettings({
        tolerancePp: toleranceBps != null ? String(toleranceBps / 100) : '5',
        currency: activeConfig.currency?.denomination ?? 'USD',
        amountUnits: activeConfig.currency?.amount_units === 'minor' ? 'minor' : 'major',
        expectedDailyTraffic: activeConfig.expected_daily_traffic != null ? String(activeConfig.expected_daily_traffic) : '',
        forecastInterval: activeConfig.forecast_interval_secs != null ? String(activeConfig.forecast_interval_secs) : '',
        steeringInterval: activeConfig.steering_interval_secs != null ? String(activeConfig.steering_interval_secs) : '',
      })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsKey, activeConfig])
  function saveMerchantSettings() {
    if (!settingsKey) return
    const next: MerchantSettings = { tolerancePp, currency, amountUnits, expectedDailyTraffic, forecastInterval, steeringInterval }
    try {
      window.localStorage.setItem(settingsKey, JSON.stringify(next))
    } catch {
      // Nothing to do: the values stay in memory for this session.
    }
    setActionSuccess('Merchant settings saved — every new contract document will use them.')
    closeBuilder()
  }

  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [actionSuccess, setActionSuccess] = useState<string | null>(null)

  const [searchParams, setSearchParams] = useSearchParams()
  const [nameFilter, setNameFilter] = useState('')
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [activatingId, setActivatingId] = useState<string | null>(null)
  const [deactivatingId, setDeactivatingId] = useState<string | null>(null)
  const [deletingId, setDeletingId] = useState<string | null>(null)
  const [pendingActivateId, setPendingActivateId] = useState<string | null>(null)
  const [pendingDeactivateId, setPendingDeactivateId] = useState<string | null>(null)
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)

  function patchContract(key: number, patch: Partial<ContractForm>) {
    setContracts((list) => list.map((c) => (c.key === key ? { ...c, ...patch } : c)))
  }

  function patchTier(contractKey: number, index: number, patch: Partial<TierForm>) {
    setContracts((list) =>
      list.map((c) => {
        if (c.key !== contractKey) return c
        const tiers = c.tiers.map((t, i) => {
          if (patch.targeted === true) {
            // Exactly one targeted tier per contract: selecting one clears the rest.
            return i === index ? { ...t, ...patch } : { ...t, targeted: false }
          }
          return i === index ? { ...t, ...patch } : t
        })
        return { ...c, tiers }
      })
    )
  }

  // ── Payload ─────────────────────────────────────────────────────────────────
  function buildConfig(): VolumeContractConfig {
    const config: VolumeContractConfig = {
      schema_version: 1,
      // The engine implements only pace-guarded steering measured in money (GMV); the form shows
      // both fields pre-filled and locked to these values.
      routing_mode: 'pace_guarded',
      tolerance: `${Math.round(parseFloat(tolerancePp || '0') * 100)}bps`,
      metric: 'gmv',
      currency: { denomination: currency.trim().toUpperCase(), amount_units: amountUnits },
      expected_daily_traffic: amount(expectedDailyTraffic),
      volume_contracts: contracts.map((c): VolumeContract => {
        const base = {
          id: c.id.trim(),
          connector: c.connector.trim(),
          status: c.status,
          billing_cycle: {
            type: c.cycleType,
            anchor: parseInt(c.anchor, 10) || 0,
            timezone: c.timezone.trim(),
          },
        }
        if (c.archetype === 'lumpsum') {
          return {
            ...base,
            archetype: 'lumpsum',
            terms: {
              target: amount(c.target),
              reward:
                c.rewardKind === 'flat'
                  ? { kind: 'flat', value: { flat_amount: amount(c.flatAmount) } }
                  : { kind: 'percentage', value: { rebate_bps: parseInt(c.rebateBps, 10) || 0 } },
            },
          }
        }
        return {
          ...base,
          archetype: 'tiered',
          terms: {
            tiers: c.tiers.map((t): VolumeContractTier => ({
              kind: t.kind,
              rate:
                t.kind === 'retroactive'
                  ? { rebate_bps: parseInt(t.bps, 10) || 0 }
                  : { rate_bps: parseInt(t.bps, 10) || 0 },
              threshold: amount(t.threshold),
              targeted: t.targeted,
              rebate_lag_days: parseInt(t.rebateLagDays, 10) || 0,
              rebate_settlement: t.rebateSettlement,
            })),
          },
        }
      }),
    }
    if (forecastInterval.trim()) config.forecast_interval_secs = parseInt(forecastInterval, 10)
    if (steeringInterval.trim()) config.steering_interval_secs = parseInt(steeringInterval, 10)
    return config
  }

  // Clears the document only; the merchant-level settings are not the document's to reset.
  function resetBuilder() {
    setDocName('')
    setDocDesc('')
    setContracts([emptyContract()])
  }

  // ── Mutations ───────────────────────────────────────────────────────────────
  async function handleCreate() {
    if (!merchantId) return
    setSubmitting(true)
    setSubmitError(null)
    setActionSuccess(null)
    try {
      const payload: CreateRoutingRequest = {
        name: docName.trim(),
        description: docDesc.trim() || `Volume commitments for ${merchantId}`,
        created_by: merchantId,
        algorithm_for: 'volume_commitment',
        algorithm: { type: 'volume_contract', data: buildConfig() },
      }
      await apiPost('/routing/create', payload)
      setActionSuccess(`“${docName.trim()}” created — activate it from the list to hand it to the routing engine.`)
      resetBuilder()
      revalidate()
      closeBuilder()
    } catch (e) {
      setSubmitError(e instanceof Error ? e.message : 'Failed to create contract document')
    } finally {
      setSubmitting(false)
    }
  }

  async function doActivate(id: string) {
    setActivatingId(id)
    setActionError(null)
    try {
      await apiPost('/routing/activate', { created_by: merchantId, routing_algorithm_id: id })
      revalidate()
    } catch (e) {
      setActionError(e instanceof Error ? e.message : 'Failed to activate')
    } finally {
      setActivatingId(null)
    }
  }

  async function doDeactivate(id: string) {
    setDeactivatingId(id)
    setActionError(null)
    try {
      await apiPost('/routing/deactivate', { created_by: merchantId, routing_algorithm_id: id })
      revalidate()
    } catch (e) {
      setActionError(e instanceof Error ? e.message : 'Failed to deactivate')
    } finally {
      setDeactivatingId(null)
    }
  }

  async function doDelete(id: string) {
    setDeletingId(id)
    setActionError(null)
    try {
      await apiPost('/routing/delete', { created_by: merchantId, routing_algorithm_id: id })
      revalidate()
    } catch (e) {
      setActionError(e instanceof Error ? e.message : 'Failed to delete')
    } finally {
      setDeletingId(null)
    }
  }

  // ── Render helpers ──────────────────────────────────────────────────────────
  function fieldLabel(text: string, hint?: string) {
    return (
      <div className="mb-1.5 flex items-baseline justify-between gap-2">
        <span className={type.label}>{text}</span>
        {hint ? <span className="text-[11px] text-slate-400 dark:text-[#8d96aa]">{hint}</span> : null}
      </div>
    )
  }

  const unitHint = `${amountUnits} ${currency.toUpperCase()} units`

  // ── Views: the document list, or the builder as its own screen (like the rules pages) ────────
  // The builder is addressed by URL (`?contract=new`) so a reload or shared link reopens it.
  const view = searchParams.get('contract')
  const showBuilder = view === 'new'
  const showSettings = view === 'settings'
  function openView(which: 'new' | 'settings') {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      next.set('contract', which)
      return next
    })
  }
  function openBuilder() {
    openView('new')
  }
  function openSettings() {
    openView('settings')
  }
  function closeBuilder() {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      next.delete('contract')
      return next
    })
  }

  const visibleDocuments = documents.filter((doc) => {
    const isActive = doc.id === activeDocumentId
    if (statusFilter === 'active' && !isActive) return false
    if (statusFilter === 'inactive' && isActive) return false
    if (nameFilter.trim()) {
      const needle = nameFilter.trim().toLowerCase()
      if (!doc.name.toLowerCase().includes(needle) && !doc.id.toLowerCase().includes(needle)) return false
    }
    return true
  })

  const merchantSettingsFields = (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                <div>
                  {fieldLabel('Routing mode', 'only supported mode')}
                  <SearchableSelect
                    triggerClassName={inputClass}
                    value="pace_guarded"
                    onChange={() => {}}
                    disabled
                    options={[{ value: 'pace_guarded', label: 'Pace-guarded (auth-rate first)' }]}
                  />
                </div>
                <div>
                  {fieldLabel('Tolerance', 'percentage points')}
                  <input
                    className={inputClass}
                    type="number"
                    min={0}
                    step={0.5}
                    placeholder="5"
                    value={tolerancePp}
                    onChange={(e) => setTolerancePp(e.target.value)}
                  />
                </div>
                <div>
                  {fieldLabel('Metric', 'only supported metric')}
                  <SearchableSelect
                    triggerClassName={inputClass}
                    value="gmv"
                    onChange={() => {}}
                    disabled
                    options={[{ value: 'gmv', label: 'GMV (money processed)' }]}
                  />
                </div>
                <div>
                  {fieldLabel('Currency')}
                  <Combobox
                    className={inputClass}
                    value={currency}
                    onChange={setCurrency}
                    options={CURRENCY_SUGGESTIONS}
                    placeholder="USD"
                  />
                </div>
                <div>
                  {fieldLabel('Amount units')}
                  <SearchableSelect
                    triggerClassName={inputClass}
                    value={amountUnits}
                    onChange={(v) => setAmountUnits(v as typeof amountUnits)}
                    options={[
                      { value: 'major', label: 'Major (6000000 = $6M)' },
                      { value: 'minor', label: 'Minor (600000000 = $6M in cents)' },
                    ]}
                  />
                </div>
                <div>
                  {fieldLabel('Expected daily traffic', unitHint)}
                  <input
                    className={inputClass}
                    placeholder="800000"
                    value={expectedDailyTraffic}
                    onChange={(e) => setExpectedDailyTraffic(e.target.value)}
                  />
                </div>
                <div>
                  {fieldLabel('Forecast interval', 'seconds, optional')}
                  <input
                    className={inputClass}
                    type="number"
                    min={60}
                    placeholder="engine default"
                    value={forecastInterval}
                    onChange={(e) => setForecastInterval(e.target.value)}
                  />
                </div>
                <div>
                  {fieldLabel('Steering interval', 'seconds, optional')}
                  <input
                    className={inputClass}
                    type="number"
                    min={60}
                    placeholder="engine default"
                    value={steeringInterval}
                    onChange={(e) => setSteeringInterval(e.target.value)}
                  />
                </div>
              </div>
  )

  if (showSettings) {
    return (
      <div className="space-y-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <button
              type="button"
              onClick={closeBuilder}
              className="mb-2 inline-flex items-center gap-2 text-sm font-medium text-slate-500 transition-colors hover:text-brand-600 dark:text-[#8d96a8] dark:hover:text-brand-400"
            >
              <ArrowLeft size={16} /> Volume Contracts
            </button>
            <PageHeading
              title="Merchant Settings"
              description="How the engine treats every volume contract for this merchant — routing mode, approval tolerance, units and expected traffic. Applied to each new contract document."
              className="truncate"
            />
          </div>
        </div>

        <Card>
          <CardBody>
            <div className="mb-3 flex items-center gap-2">
              <SlidersHorizontal size={14} className="text-slate-400 dark:text-[#8a8a93]" />
              <span className={type.label}>Merchant-level settings</span>
            </div>
            {merchantSettingsFields}
          </CardBody>
        </Card>

        <div className="flex flex-wrap items-center gap-4">
          <Button onClick={saveMerchantSettings} disabled={!merchantId || !canEditRouting}>
            Save settings
          </Button>
          <button
            type="button"
            onClick={closeBuilder}
            className="text-sm font-medium text-slate-500 transition-colors hover:text-slate-800 dark:text-[#8d96a8] dark:hover:text-white"
          >
            Cancel
          </button>
          <p className="max-w-[57ch] text-sm text-slate-500 dark:text-[#78849a]">
            Existing documents keep the settings they were created with.
          </p>
        </div>
      </div>
    )
  }

  if (showBuilder) {
    return (
      <div className="space-y-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <button
              type="button"
              onClick={closeBuilder}
              className="mb-2 inline-flex items-center gap-2 text-sm font-medium text-slate-500 transition-colors hover:text-brand-600 dark:text-[#8d96a8] dark:hover:text-brand-400"
            >
              <ArrowLeft size={16} /> Volume Contracts
            </button>
            <PageHeading
              title="Create Contract Document"
              description="Express PSP volume-commitment contracts — goals, rebates and billing cycles. Nothing here changes routing until the document is activated."
              className="truncate"
            />
          </div>
        </div>

        <div className="space-y-6">
          {/* Merchant-level settings: set once for the merchant; the values live on their own screen. */}
          <InsetPanel>
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <SlidersHorizontal size={14} className="text-slate-400 dark:text-[#8a8a93]" />
                <span className={type.label}>Merchant-level settings</span>
                <span className="text-sm text-slate-500 dark:text-[#8d96aa]">
                  {expectedDailyTraffic.trim()
                    ? '— applied to this document when it is created.'
                    : '— expected daily traffic is not set yet; the document cannot be created without it.'}
                </span>
              </div>
              <Button variant="secondary" size="sm" onClick={openSettings}>
                Edit settings
              </Button>
            </div>
          </InsetPanel>

          {/* Document */}
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              {fieldLabel('Name')}
              <input
                className={inputClass}
                placeholder="q3_volume_contracts"
                value={docName}
                onChange={(e) => setDocName(e.target.value)}
              />
            </div>
            <div>
              {fieldLabel('Description')}
              <input
                className={inputClass}
                placeholder="Q3 PSP volume commitments"
                value={docDesc}
                onChange={(e) => setDocDesc(e.target.value)}
              />
            </div>
          </div>

          {/* Per-PSP contracts */}
          <div className="space-y-4">
            {contracts.map((contract, contractIdx) => (
              <InsetPanel key={contract.key}>
                <div className="mb-3 flex items-center justify-between">
                  <span className={type.label}>Contract {contractIdx + 1}</span>
                  {contracts.length > 1 ? (
                    <button
                      type="button"
                      className="text-slate-400 hover:text-red-500"
                      onClick={() => setContracts((list) => list.filter((c) => c.key !== contract.key))}
                      aria-label="Remove contract"
                    >
                      <Trash2 size={14} />
                    </button>
                  ) : null}
                </div>

                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  <div>
                    {fieldLabel('Contract ID')}
                    <input
                      className={inputClass}
                      placeholder="adyen_2026_lumpsum"
                      value={contract.id}
                      onChange={(e) => patchContract(contract.key, { id: e.target.value })}
                    />
                  </div>
                  <div>
                    {fieldLabel('Connector', 'exact gateway name')}
                    <Combobox
                      className={inputClass}
                      value={contract.connector}
                      onChange={(v) => patchContract(contract.key, { connector: v })}
                      options={CONNECTOR_SUGGESTIONS}
                      placeholder="adyen"
                    />
                  </div>
                  <div>
                    {fieldLabel('Status')}
                    <SearchableSelect
                  triggerClassName={inputClass}
                      value={contract.status}
                      onChange={(v) => patchContract(contract.key, { status: v as 'active' | 'inactive' })}
                      options={[
                        { value: 'active', label: 'Active' },
                        { value: 'inactive', label: 'Inactive' },
                      ]}
                    />
                  </div>
                  <div>
                    {fieldLabel('Billing cycle')}
                    <SearchableSelect
                  triggerClassName={inputClass}
                      value={contract.cycleType}
                      onChange={(v) => patchContract(contract.key, { cycleType: v as ContractForm['cycleType'] })}
                      options={[
                        { value: 'calendar_month', label: 'Calendar month' },
                        { value: 'calendar_quarter', label: 'Calendar quarter' },
                        { value: 'calendar_year', label: 'Calendar year' },
                        { value: 'test_minutes', label: 'Test cycle (minutes)' },
                      ]}
                    />
                  </div>
                  <div>
                    {fieldLabel('Anchor', ANCHOR_HELP[contract.cycleType])}
                    <input
                      className={inputClass}
                      type="number"
                      min={1}
                      value={contract.anchor}
                      onChange={(e) => patchContract(contract.key, { anchor: e.target.value })}
                    />
                  </div>
                  {contract.cycleType === 'test_minutes' && (
                    <div className="sm:col-span-3">
                      <p className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-400">
                        Testing only. The cycle lasts {contract.anchor || '?'} minutes and each
                        minute counts as one contract day, so a full period — pacing,
                        elimination, steering — plays out while you watch. Drive traffic at it
                        from the Decision Simulator; timezone is ignored.
                      </p>
                    </div>
                  )}
                  <div>
                    {fieldLabel('Timezone', 'IANA')}
                    <Combobox
                      className={inputClass}
                      value={contract.timezone}
                      onChange={(v) => patchContract(contract.key, { timezone: v })}
                      options={TIMEZONE_SUGGESTIONS}
                      placeholder="UTC"
                    />
                  </div>
                </div>

                <div className="mt-4">
                  {fieldLabel('Archetype')}
                  <div className="flex gap-2">
                    {(
                      [
                        ['lumpsum', 'Lumpsum — reward on hitting a target'],
                        ['tiered', 'Tiered — rebate ladder by threshold'],
                      ] as const
                    ).map(([value, label]) => (
                      <button
                        key={value}
                        type="button"
                        onClick={() => patchContract(contract.key, { archetype: value })}
                        className={`rounded-full border px-4 py-1.5 text-xs font-medium transition-colors ${
                          contract.archetype === value
                            ? 'border-brand-600 bg-brand-600 text-white'
                            : 'border-slate-200 text-slate-600 hover:bg-slate-50 dark:border-[#222226] dark:text-[#9ca7ba] dark:hover:bg-[#151518]'
                        }`}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                </div>

                {contract.archetype === 'lumpsum' ? (
                  <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                      {fieldLabel('Target', unitHint)}
                      <input
                        className={inputClass}
                        placeholder="6000000"
                        value={contract.target}
                        onChange={(e) => patchContract(contract.key, { target: e.target.value })}
                      />
                    </div>
                    <div>
                      {fieldLabel('Reward kind')}
                      <SearchableSelect
                  triggerClassName={inputClass}
                        value={contract.rewardKind}
                        onChange={(v) => patchContract(contract.key, { rewardKind: v as 'flat' | 'percentage' })}
                        options={[
                          { value: 'flat', label: 'Flat amount' },
                          { value: 'percentage', label: 'Percentage rebate' },
                        ]}
                      />
                    </div>
                    {contract.rewardKind === 'flat' ? (
                      <div>
                        {fieldLabel('Flat amount', unitHint)}
                        <input
                          className={inputClass}
                          placeholder="15000"
                          value={contract.flatAmount}
                          onChange={(e) => patchContract(contract.key, { flatAmount: e.target.value })}
                        />
                      </div>
                    ) : (
                      <div>
                        {fieldLabel('Rebate', 'basis points')}
                        <input
                          className={inputClass}
                          type="number"
                          min={1}
                          max={10000}
                          placeholder="25"
                          value={contract.rebateBps}
                          onChange={(e) => patchContract(contract.key, { rebateBps: e.target.value })}
                        />
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="mt-4 space-y-3">
                    {contract.tiers.map((tier, tierIdx) => (
                      <div
                        key={tierIdx}
                        className="rounded-xl border border-slate-200 p-3 dark:border-[#222226]"
                      >
                        <div className="mb-2 flex items-center justify-between">
                          <label className="flex cursor-pointer items-center gap-2 text-[12px] font-medium text-slate-600 dark:text-[#9ca7ba]">
                            <input
                              type="radio"
                              name={`targeted-${contract.key}`}
                              checked={tier.targeted}
                              onChange={() => patchTier(contract.key, tierIdx, { targeted: true })}
                              disabled={tier.kind === 'marginal'}
                            />
                            Targeted tier (the goal the engine steers for)
                          </label>
                          {contract.tiers.length > 1 ? (
                            <button
                              type="button"
                              className="text-slate-400 hover:text-red-500"
                              onClick={() =>
                                patchContract(contract.key, {
                                  tiers: contract.tiers.filter((_, i) => i !== tierIdx),
                                })
                              }
                              aria-label="Remove tier"
                            >
                              <Trash2 size={13} />
                            </button>
                          ) : null}
                        </div>
                        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-5">
                          <div>
                            {fieldLabel('Kind')}
                            <SearchableSelect
                  triggerClassName={inputClass}
                              value={tier.kind}
                              onChange={(v) =>
                                patchTier(contract.key, tierIdx, {
                                  kind: v as 'retroactive' | 'marginal',
                                  ...(v === 'marginal' ? { targeted: false } : {}),
                                })
                              }
                              options={[
                                { value: 'retroactive', label: 'Retroactive (whole period)' },
                                { value: 'marginal', label: 'Marginal (above threshold)' },
                              ]}
                            />
                          </div>
                          <div>
                            {fieldLabel('Threshold', unitHint)}
                            <input
                              className={inputClass}
                              placeholder="8000000"
                              value={tier.threshold}
                              onChange={(e) => patchTier(contract.key, tierIdx, { threshold: e.target.value })}
                            />
                          </div>
                          <div>
                            {fieldLabel(tier.kind === 'retroactive' ? 'Rebate' : 'Rate', 'bps')}
                            <input
                              className={inputClass}
                              type="number"
                              min={1}
                              max={10000}
                              placeholder="20"
                              value={tier.bps}
                              onChange={(e) => patchTier(contract.key, tierIdx, { bps: e.target.value })}
                            />
                          </div>
                          <div>
                            {fieldLabel('Rebate lag', 'days')}
                            <input
                              className={inputClass}
                              type="number"
                              min={0}
                              max={365}
                              value={tier.rebateLagDays}
                              onChange={(e) => patchTier(contract.key, tierIdx, { rebateLagDays: e.target.value })}
                            />
                          </div>
                          <div>
                            {fieldLabel('Settlement')}
                            <SearchableSelect
                  triggerClassName={inputClass}
                              value={tier.rebateSettlement}
                              onChange={(v) =>
                                patchTier(contract.key, tierIdx, { rebateSettlement: v as 'cash' | 'credit_note' })
                              }
                              options={[
                                { value: 'cash', label: 'Cash' },
                                { value: 'credit_note', label: 'Credit note' },
                              ]}
                            />
                          </div>
                        </div>
                      </div>
                    ))}
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() =>
                        patchContract(contract.key, { tiers: [...contract.tiers, emptyTier(false)] })
                      }
                    >
                      <Plus size={13} /> Add tier
                    </Button>
                  </div>
                )}
              </InsetPanel>
            ))}

            <Button variant="secondary" onClick={() => setContracts((list) => [...list, emptyContract()])}>
              <Plus size={14} /> Add PSP contract
            </Button>
          </div>
        </div>


        <ErrorMessage error={submitError} />
        <div className="flex flex-wrap items-center gap-4">
          <Button
            onClick={handleCreate}
            disabled={submitting || !merchantId || !docName.trim() || !expectedDailyTraffic.trim() || !canEditRouting}
          >
            {submitting ? <Spinner size={14} /> : null} {submitting ? 'Saving...' : 'Create Contract'}
          </Button>
          <Button variant="secondary" size="sm" onClick={resetBuilder} disabled={submitting}>
            Clear
          </Button>
          <button
            type="button"
            onClick={closeBuilder}
            className="text-sm font-medium text-slate-500 transition-colors hover:text-slate-800 dark:text-[#8d96a8] dark:hover:text-white"
          >
            Cancel
          </button>
          <p className="max-w-[57ch] text-sm text-slate-500 dark:text-[#78849a]">
            {expectedDailyTraffic.trim()
              ? 'A new document is created inactive — activate it from the contracts list.'
              : 'Set the expected daily traffic under Merchant settings before creating a document.'}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          {embedded ? (
            <p className={type.subheading}>
              PSP volume-commitment contracts — goals, rebates and billing cycles. The routing engine reads
              the active document to pace and steer traffic.
            </p>
          ) : (
            <PageHeading
              title="Volume Contracts"
              description="PSP volume-commitment contracts — goals, rebates and billing cycles. The routing engine reads the active document to pace and steer traffic."
            />
          )}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="secondary" onClick={openSettings} disabled={!canEditRouting}>
            <SlidersHorizontal size={15} /> Merchant settings
          </Button>
          <Button onClick={openBuilder} disabled={!canEditRouting}>
            <Plus size={15} /> Create Contract
          </Button>
        </div>
      </div>

      {actionError && <ErrorMessage error={actionError} />}
      {actionSuccess && (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-600 dark:text-emerald-400">
          {actionSuccess}
        </div>
      )}

      <Card className="!rounded-[18px]">
        {!merchantId ? (
          <p className="px-4 py-6 text-sm text-slate-500">Set merchant ID to load contract documents.</p>
        ) : isLoading ? (
          <p className="px-4 py-6 text-sm text-slate-500">Loading...</p>
        ) : documents.length === 0 ? (
          <p className="px-4 py-6 text-sm text-slate-500">No contract documents yet.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[720px] text-left">
              <thead>
                <tr className="border-b border-slate-100 text-[11px] font-semibold uppercase leading-4 tracking-wide text-slate-500 dark:border-[#1e2330] dark:text-[#78849a]">
                  <th className="px-5 py-3.5">
                    <HeaderSearch
                      label="Document Name & ID"
                      value={nameFilter}
                      onChange={setNameFilter}
                      ariaLabel="Filter documents by name"
                    />
                  </th>
                  <th className="px-5 py-3.5">
                    <HeaderFilter
                      label="Status"
                      value={statusFilter}
                      options={[
                        { value: 'all', label: 'All statuses' },
                        { value: 'active', label: 'Active' },
                        { value: 'inactive', label: 'Inactive' },
                      ]}
                      onChange={(v) => setStatusFilter(v as StatusFilter)}
                      ariaLabel="Filter by status"
                    />
                  </th>
                  <th className="px-5 py-3.5">Billing Cycle</th>
                  <th className="px-5 py-3.5">Created</th>
                  <th className="px-5 py-3.5 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {visibleDocuments.length === 0 && (
                  <tr>
                    <td colSpan={5} className="px-4 py-8 text-center">
                      <p className="text-sm text-slate-500">No documents match these filters.</p>
                      <button
                        type="button"
                        onClick={() => { setStatusFilter('all'); setNameFilter('') }}
                        className="mt-1.5 text-sm font-medium text-brand-600 hover:text-brand-700 dark:text-brand-400"
                      >
                        Clear filters
                      </button>
                    </td>
                  </tr>
                )}
                {visibleDocuments.map((doc) => {
                  const isActive = doc.id === activeDocumentId
                  const isExpanded = expandedId === doc.id
                  const data = (doc.algorithm_data ?? doc.algorithm)?.data as VolumeContractConfig | undefined
                  const stamp = formatCreated(doc)
                  // /routing/delete rejects an active document, so the control mirrors that.
                  const lockedReason = isActive
                    ? 'Deactivate this document first'
                    : !canEditRouting
                    ? 'You do not have permission to change routing'
                    : undefined
                  return [
                    <tr
                      key={doc.id}
                      onClick={() => setExpandedId(isExpanded ? null : doc.id)}
                      className={`cursor-pointer border-b border-slate-100 align-middle transition-colors hover:bg-slate-50 dark:border-[#1e2330] dark:hover:bg-[#11151d] ${
                        isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : ''
                      }`}
                    >
                      <td className="px-5 py-4 align-top">
                        <div className="flex items-start gap-2">
                          {isExpanded
                            ? <ChevronDown size={14} className="mt-1 shrink-0 text-slate-500" />
                            : <ChevronRight size={14} className="mt-1 shrink-0 text-slate-500" />}
                          <div className="min-w-0">
                            <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">{doc.name}</p>
                            <p className="mt-0.5 truncate font-mono text-xs text-slate-500 dark:text-[#78849a]">{doc.id}</p>
                          </div>
                        </div>
                      </td>
                      <td className="px-5 py-4 align-top">
                        <span className={`inline-flex shrink-0 items-center rounded-full px-2.5 py-1 text-[11px] font-semibold leading-4 ${
                          isActive
                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400'
                            : 'bg-slate-100 text-slate-500 dark:bg-[#1a1f2a] dark:text-[#8090a8]'
                        }`}>
                          {isActive ? 'Active' : 'Inactive'}
                        </span>
                      </td>
                      <td className="max-w-[260px] px-5 py-4 align-top">
                        <p className="break-words text-sm font-medium leading-5 text-slate-800 dark:text-slate-200">
                          {summarizeCycle(data)}
                        </p>
                        <p className="mt-0.5 text-xs text-slate-500 dark:text-[#78849a]">
                          {(data?.volume_contracts?.length ?? 0)} PSP commitment{(data?.volume_contracts?.length ?? 0) === 1 ? '' : 's'}
                        </p>
                      </td>
                      <td className="whitespace-nowrap px-5 py-4 align-top text-[13px] leading-[18px] text-slate-500 dark:text-[#78849a]">
                        {stamp ? (
                          <span title={stamp.full}>
                            {stamp.date}
                            <span className="block text-[12px] leading-4 text-slate-500 dark:text-[#78849a]">{stamp.time}</span>
                          </span>
                        ) : '—'}
                      </td>
                      <td className="px-5 py-4 align-top" onClick={(e) => e.stopPropagation()}>
                        <RowMenu
                          items={[
                            isActive
                              ? {
                                  label: deactivatingId === doc.id ? 'Deactivating…' : 'Deactivate',
                                  icon: PowerOff,
                                  onSelect: () => setPendingDeactivateId(doc.id),
                                  disabled: deactivatingId === doc.id || !canEditRouting,
                                }
                              : {
                                  label: activatingId === doc.id ? 'Activating…' : 'Activate',
                                  icon: Zap,
                                  tone: 'positive',
                                  onSelect: () => (activeDocumentId ? setPendingActivateId(doc.id) : doActivate(doc.id)),
                                  disabled: activatingId === doc.id || !canEditRouting,
                                },
                            {
                              label: deletingId === doc.id ? 'Deleting…' : 'Delete',
                              icon: Trash2,
                              tone: 'danger',
                              onSelect: () => setPendingDeleteId(doc.id),
                              disabled: Boolean(lockedReason) || deletingId === doc.id,
                              hint: lockedReason,
                            },
                          ]}
                        />
                      </td>
                    </tr>,
                    isExpanded ? (
                      <tr key={`${doc.id}-detail`} className="border-b border-slate-100 dark:border-[#1e2330]">
                        <td colSpan={5} className="bg-slate-50/60 px-5 py-5 dark:bg-[#0d1017]">
                          <ContractDocumentDetail config={data} />
                        </td>
                      </tr>
                    ) : null,
                  ]
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <ConfirmDialog
        open={pendingActivateId != null}
        title="Replace the active document?"
        description="Another contract document is currently active. Activating this one hands the engine this document instead."
        confirmLabel="Activate"
        variant="primary"
        onConfirm={() => {
          if (pendingActivateId) doActivate(pendingActivateId)
          setPendingActivateId(null)
        }}
        onCancel={() => setPendingActivateId(null)}
      />
      <ConfirmDialog
        open={pendingDeactivateId != null}
        title="Deactivate this document?"
        description="The routing engine stops receiving volume-commitment configuration until another document is activated."
        confirmLabel="Deactivate"
        variant="danger"
        onConfirm={() => {
          if (pendingDeactivateId) doDeactivate(pendingDeactivateId)
          setPendingDeactivateId(null)
        }}
        onCancel={() => setPendingDeactivateId(null)}
      />
      <ConfirmDialog
        open={pendingDeleteId != null}
        title="Delete this document?"
        description="This permanently removes the contract document. Only inactive documents can be deleted."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={() => {
          if (pendingDeleteId) doDelete(pendingDeleteId)
          setPendingDeleteId(null)
        }}
        onCancel={() => setPendingDeleteId(null)}
      />
    </div>
  )
}
