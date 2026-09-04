import { Fragment, useEffect, useMemo, useRef, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import useSWR, { useSWRConfig } from 'swr'
import {
  AlertCircle,
  ArrowLeft,
  ChevronDown,
  ChevronRight,
  Pencil,
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
  VolumeContractAmount,
  VolumeContractConfig,
  VolumeContractReward,
  VolumeContractSample,
  VolumeContractTier,
} from '../../types/api'
import { Card, CardBody, InsetPanel } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { useVolumeContractSamples } from '../../hooks/useVolumeCommitment'
import { Button } from '../ui/Button'
import { PageHeading } from '../ui/PageHeading'
import { HeaderFilter, HeaderSearch, RowMenu } from '../ui/TableControls'
import { parseBackendTimestamp } from '../../lib/routingRuleTimestamps'
import { formatMoney } from './volumeCommitmentChartBits'
import {
  VolumeContractActivationSummary,
  VolumeContractFeatureNotice,
} from './VolumeContractFeatureNotice'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { Notice } from '../ui/Notice'
import { SearchableSelect } from '../ui/SearchableSelect'
import { Combobox } from '../ui/Combobox'
import * as type from '../ui/typography'

// Matches the input styling used across the routing config pages (SRRoutingPage). The border
// colour is set by the variant rather than appended, because two border-colour utilities on one
// element resolve by stylesheet order, not by their order in the class string.
const inputBase =
  'w-full rounded-lg border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-1'
const inputClass = `${inputBase} border-slate-200 focus:ring-brand-500 dark:border-[#222226]`
const inputInvalidClass = `${inputBase} border-red-500 focus:ring-red-500 dark:border-red-500/70`

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

/**
 * Document-wide values the builder stamps onto every contract it writes. Kept per merchant for
 * new documents; an edited document brings its own, since it was stored with them.
 */
type MerchantSettings = {
  routingMode: 'pace_guarded' | 'volume_commitment'
  tolerancePp: string
  metric: 'gmv' | 'volume'
  currency: string
  amountUnits: 'major' | 'minor'
  expectedDailyTraffic: string
  forecastInterval: string
  steeringInterval: string
}

/**
 * The settings a stored document was written with. A stored document is always canonical —
 * integer minor units and `tolerance_bps` — so these are read back verbatim rather than
 * reinterpreted, which is what keeps an edit's amounts identical to what is in force.
 */
function settingsFromConfig(config: VolumeContractConfig): MerchantSettings {
  const toleranceBps =
    config.tolerance_bps ?? (config.tolerance ? parseFloat(config.tolerance) : undefined)
  return {
    routingMode: config.routing_mode === 'volume_commitment' ? 'volume_commitment' : 'pace_guarded',
    tolerancePp: toleranceBps != null ? String(toleranceBps / 100) : '5',
    metric: config.metric === 'volume' ? 'volume' : 'gmv',
    currency: config.currency?.denomination ?? 'USD',
    amountUnits: config.currency?.amount_units === 'minor' ? 'minor' : 'major',
    expectedDailyTraffic:
      config.expected_daily_traffic != null ? String(config.expected_daily_traffic) : '',
    forecastInterval:
      config.forecast_interval_secs != null ? String(config.forecast_interval_secs) : '',
    steeringInterval:
      config.steering_interval_secs != null ? String(config.steering_interval_secs) : '',
  }
}

/** The archetypes the builder has controls for. `min_commitment` is storable but not editable here. */
function isBuilderArchetype(contract: VolumeContract) {
  return contract.archetype === 'lumpsum' || contract.archetype === 'tiered'
}

function tierToForm(tier: VolumeContractTier): TierForm {
  const rate = tier.rate as { rebate_bps?: number; rate_bps?: number }
  return {
    kind: tier.kind,
    bps: String((tier.kind === 'retroactive' ? rate.rebate_bps : rate.rate_bps) ?? ''),
    threshold: String(tier.threshold ?? ''),
    targeted: tier.targeted ?? true,
    rebateLagDays: String(tier.rebate_lag_days ?? 0),
    rebateSettlement: tier.rebate_settlement ?? 'cash',
  }
}

/** The inverse of the contract half of `buildConfig`, for loading a document back into the form. */
function contractToForm(contract: VolumeContract): ContractForm {
  const base = {
    key: contractKeySeq++,
    id: contract.id ?? '',
    connector: contract.connector ?? '',
    status: contract.status ?? ('active' as const),
    cycleType: contract.billing_cycle?.type ?? ('calendar_month' as const),
    anchor: String(contract.billing_cycle?.anchor ?? 1),
    timezone: contract.billing_cycle?.timezone ?? 'UTC',
  }
  if (contract.archetype === 'tiered') {
    const tiers = contract.terms.tiers ?? []
    return {
      ...base,
      archetype: 'tiered',
      target: '',
      rewardKind: 'flat',
      flatAmount: '',
      rebateBps: '',
      tiers: tiers.length ? tiers.map(tierToForm) : [emptyTier(true)],
    }
  }
  // `min_commitment` never reaches here — callers filter with `isBuilderArchetype` first.
  const terms = contract.terms as { target?: VolumeContractAmount; reward?: VolumeContractReward }
  const reward = terms.reward
  return {
    ...base,
    archetype: 'lumpsum',
    target: String(terms.target ?? ''),
    rewardKind: reward?.kind === 'percentage' ? 'percentage' : 'flat',
    flatAmount: reward?.kind === 'flat' ? String(reward.value.flat_amount ?? '') : '',
    rebateBps: reward?.kind === 'percentage' ? String(reward.value.rebate_bps ?? '') : '',
    tiers: [emptyTier(true)],
  }
}

const ARCHETYPE_LABELS: Record<ContractForm['archetype'], string> = {
  lumpsum: 'Lumpsum',
  tiered: 'Tiered',
}

/** Anchor bounds per cycle type, mirroring `validate_volume_contract_config`. */
const ANCHOR_RANGE: Record<ContractForm['cycleType'], [number, number]> = {
  calendar_month: [1, 30],
  calendar_quarter: [1, 3],
  calendar_year: [1, 12],
  test_minutes: [2, 240],
}

/**
 * The goal the summary table shows. A lumpsum contract states its target outright; a tiered one
 * is steered at its targeted tier, so that tier's threshold is the goal.
 */
function targetOf(contract: ContractForm): string {
  if (contract.archetype === 'lumpsum') return contract.target.trim()
  const targeted = contract.tiers.find((tier) => tier.targeted) ?? contract.tiers[0]
  return targeted?.threshold.trim() ?? ''
}

/** An amount as typed, grouped for reading. Left alone when it is not a number yet. */
function formatEntered(value: string): string {
  const trimmed = value.trim()
  if (!trimmed) return '\u2014'
  const parsed = Number(trimmed)
  return Number.isFinite(parsed) ? parsed.toLocaleString() : trimmed
}

const CYCLE_LABELS: Record<ContractForm['cycleType'], string> = {
  calendar_month: 'Month',
  calendar_quarter: 'Quarter',
  calendar_year: 'Year',
  test_minutes: 'Test',
}

/** The billing cycle in one cell: the cycle, and the anchor that positions it within one. */
function cycleSummary(contract: ContractForm): string {
  const label = CYCLE_LABELS[contract.cycleType]
  const anchor = contract.anchor.trim()
  if (!anchor) return label
  return contract.cycleType === 'test_minutes' ? `${label} ${anchor}m` : `${label} ${anchor}`
}

/** What the PSP pays back on the goal. A tiered contract quotes the tier it is steered at. */
function rewardSummary(contract: ContractForm): string {
  if (contract.archetype === 'lumpsum') {
    if (contract.rewardKind === 'flat')
      return contract.flatAmount.trim() ? `Flat ${formatEntered(contract.flatAmount)}` : '\u2014'
    return contract.rebateBps.trim() ? `${contract.rebateBps.trim()}bps` : '\u2014'
  }
  const targeted = contract.tiers.find((tier) => tier.targeted) ?? contract.tiers[0]
  if (!targeted?.bps.trim()) return '\u2014'
  return `${targeted.bps.trim()}bps ${targeted.kind === 'retroactive' ? 'retro' : 'marginal'}`
}

/** A contract's problems, keyed by the field that fixes each one. */
type ContractIssues = Record<string, string>

/**
 * What would make this contract fail validation. The server checks all of it too; repeating it
 * here is what lets each field state its own problem, instead of one save returning one error
 * for a document whose other contracts are collapsed out of sight.
 */
function contractIssues(contract: ContractForm): ContractIssues {
  const issues: ContractIssues = {}
  const id = contract.id.trim()
  if (!id) issues.id = 'Contract ID is required'
  else if (!/^[A-Za-z0-9_-]{1,64}$/.test(id))
    issues.id = 'Use only letters, digits, underscore and hyphen'
  if (!contract.connector.trim()) issues.connector = 'Connector is required'
  if (!contract.timezone.trim()) issues.timezone = 'Timezone is required'

  const [minAnchor, maxAnchor] = ANCHOR_RANGE[contract.cycleType]
  const anchor = Number(contract.anchor)
  if (!Number.isInteger(anchor) || anchor < minAnchor || anchor > maxAnchor)
    issues.anchor = `Must be ${minAnchor}\u2013${maxAnchor} for this billing cycle`

  if (contract.archetype === 'lumpsum') {
    if (!contract.target.trim()) issues.target = 'Target is required'
    const reward = contract.rewardKind === 'flat' ? contract.flatAmount : contract.rebateBps
    if (!reward.trim()) issues.reward = 'Reward is required'
  } else {
    if (!contract.tiers.length) issues.tiers = 'A tiered contract needs at least one tier'
    contract.tiers.forEach((tier, index) => {
      if (!tier.threshold.trim()) issues[`tier${index}.threshold`] = 'Required'
      if (!tier.bps.trim()) issues[`tier${index}.bps`] = 'Required'
    })
    if (contract.tiers.length && !contract.tiers.some((tier) => tier.targeted))
      issues.tiers = 'One tier must be the targeted tier'
    // The engine walks the ladder in order, so the thresholds have to climb.
    const thresholds = contract.tiers.map((tier) => Number(tier.threshold))
    if (
      thresholds.every((value) => Number.isFinite(value)) &&
      thresholds.some((value, index) => index > 0 && value <= thresholds[index - 1])
    )
      issues.tiers = 'Tier thresholds must increase down the ladder'
  }
  return issues
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
    // Activating, deactivating or deleting a document changes what contract routing would do, and
    // both projection surfaces below read that from one cached key.
    mutateCache(`/merchant-account/${merchantId}/volume-commitment/projection`)
  }

  // ── Builder state ───────────────────────────────────────────────────────────
  const [docName, setDocName] = useState('')
  /** Which template seeded this draft, so the picker can show it as chosen. */
  const [loadedSample, setLoadedSample] = useState<string | null>(null)
  const { samples } = useVolumeContractSamples(merchantId ?? undefined)
  const [docDesc, setDocDesc] = useState('')
  const [routingMode, setRoutingMode] = useState<'pace_guarded' | 'volume_commitment'>('pace_guarded')
  const [tolerancePp, setTolerancePp] = useState('5')
  const [metric, setMetric] = useState<'gmv' | 'volume'>('gmv')
  const [currency, setCurrency] = useState('USD')
  const [amountUnits, setAmountUnits] = useState<'major' | 'minor'>('major')
  const [expectedDailyTraffic, setExpectedDailyTraffic] = useState('')
  const [forecastInterval, setForecastInterval] = useState('')
  const [steeringInterval, setSteeringInterval] = useState('')
  const [contracts, setContracts] = useState<ContractForm[]>([emptyContract()])
  // Which contract the detail panel is editing. The panel writes straight into `contracts`, so
  // this is a view concern only — nothing is staged behind it.
  const [selectedKey, setSelectedKey] = useState<number | null>(null)
  // Row problems stay hidden until a save is attempted, so a fresh document is not born red.
  const [showIssues, setShowIssues] = useState(false)

  // ── Views: the document list, or the builder as its own screen (like the rules pages) ────────
  // The builder is addressed by URL so a reload or shared link reopens it: `?contract=new` for a
  // fresh document, `?contract=settings` for the merchant settings, and `?contract=<id>` to edit
  // an existing one.
  const [searchParams, setSearchParams] = useSearchParams()
  const view = searchParams.get('contract')
  const showSettings = view === 'settings'
  const editingId = view && view !== 'new' && view !== 'settings' ? view : null
  const showBuilder = view === 'new' || editingId != null

  const editingDoc = useMemo(
    () => (editingId ? documents.find((doc) => doc.id === editingId) : undefined),
    [documents, editingId],
  )
  const editingConfig = useMemo(
    () => (editingDoc?.algorithm_data ?? editingDoc?.algorithm)?.data as VolumeContractConfig | undefined,
    [editingDoc],
  )
  // /routing/update runs ensure_routing_algorithm_inactive, so say so up front rather than
  // letting Save fail.
  const editingActiveDoc = editingId != null && editingId === activeDocumentId
  // A document holding an archetype the builder has no controls for would be silently rewritten
  // as something else on save, so it stays read-only instead.
  const editingUnsupported = Boolean(
    editingConfig && !(editingConfig.volume_contracts ?? []).every(isBuilderArchetype),
  )

  // ── Merchant-level settings live outside the document builder ────────────────────────────
  // Edited on their own screen and kept per merchant in this browser; every new document is
  // stamped with them (the engine still reads them from the document). Seeded from the active
  // document the first time, so an existing setup carries over.
  const settingsKey = merchantId ? `vc_merchant_settings_${merchantId}` : null
  const activeConfig = useMemo(() => {
    const doc = documents.find((d) => d.id === activeDocumentId)
    return (doc?.algorithm_data ?? doc?.algorithm)?.data as VolumeContractConfig | undefined
  }, [documents, activeDocumentId])
  function applySettings(next: Partial<MerchantSettings>) {
    if (next.routingMode) setRoutingMode(next.routingMode)
    if (next.tolerancePp != null) setTolerancePp(next.tolerancePp)
    if (next.metric) setMetric(next.metric)
    if (next.currency != null) setCurrency(next.currency)
    if (next.amountUnits) setAmountUnits(next.amountUnits)
    if (next.expectedDailyTraffic != null) setExpectedDailyTraffic(next.expectedDailyTraffic)
    if (next.forecastInterval != null) setForecastInterval(next.forecastInterval)
    if (next.steeringInterval != null) setSteeringInterval(next.steeringInterval)
  }
  useEffect(() => {
    if (!settingsKey) return
    // An edited document carries the settings it was stored with; the merchant defaults must not
    // overwrite them, or a document written in minor units would be re-saved as major ones and
    // every amount in it would move by two decimal places. Leaving the editor restores them.
    if (editingId) return
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
    if (activeConfig) applySettings(settingsFromConfig(activeConfig))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsKey, activeConfig, editingId])

  // Load the document under edit into the form. Seeded once per document — SWR revalidation
  // would otherwise throw away whatever the user has typed.
  const hydratedFrom = useRef<string | null>(null)
  useEffect(() => {
    if (!editingId) {
      hydratedFrom.current = null
      return
    }
    if (!editingDoc || !editingConfig || hydratedFrom.current === editingId) return
    hydratedFrom.current = editingId
    setDocName(editingDoc.name ?? '')
    setDocDesc(editingDoc.description && editingDoc.description !== 'N/A' ? editingDoc.description : '')
    applySettings(settingsFromConfig(editingConfig))
    const forms = (editingConfig.volume_contracts ?? []).filter(isBuilderArchetype).map(contractToForm)
    setContracts(forms.length ? forms : [emptyContract()])
    // A freshly loaded document has not been saved from here yet, so it starts unflagged.
    setShowIssues(false)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editingId, editingDoc, editingConfig])
  function saveMerchantSettings() {
    if (!settingsKey) return
    const next: MerchantSettings = { routingMode, tolerancePp, metric, currency, amountUnits, expectedDailyTraffic, forecastInterval, steeringInterval }
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

  /** Every contract's problems by key, so a row can show its own and Save can jump to the first. */
  const contractProblems = useMemo(() => {
    const idCounts = new Map<string, number>()
    for (const contract of contracts) {
      const id = contract.id.trim()
      if (id) idCounts.set(id, (idCounts.get(id) ?? 0) + 1)
    }
    return new Map(
      contracts.map((contract) => {
        const issues = contractIssues(contract)
        if ((idCounts.get(contract.id.trim()) ?? 0) > 1)
          issues.id = 'Already used by another contract in this document'
        return [contract.key, issues] as const
      }),
    )
  }, [contracts])

  // Keep the panel pointed at a real contract: seed it when the builder opens, and move it when
  // whatever it was editing goes away — removed, or replaced by a document loading into the form.
  // `selectedKey` is deliberately not a dependency: closing the panel changes only that, so a
  // close stays closed until the contract list itself changes.
  useEffect(() => {
    if (!showBuilder) return
    setSelectedKey((current) =>
      current !== null && contracts.some((contract) => contract.key === current)
        ? current
        : contracts[0]?.key ?? null,
    )
  }, [showBuilder, contracts])

  /** The document's own required values. Both are outside any contract, so they flag in place. */
  const documentIssues = useMemo(() => {
    const issues: Record<string, string> = {}
    if (!docName.trim()) issues.name = 'Name is required'
    if (!expectedDailyTraffic.trim())
      issues.expectedDailyTraffic = 'Required \u2014 set it under Merchant settings'
    return issues
  }, [docName, expectedDailyTraffic])

  function addContract() {
    const contract = emptyContract()
    setContracts((list) => [...list, contract])
    setSelectedKey(contract.key)
  }

  /** A document needs at least one contract, so the last one cannot be removed. */
  function removeContract(key: number) {
    setContracts((list) => (list.length > 1 ? list.filter((c) => c.key !== key) : list))
  }

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

  // A template is a document like any other, so it is loaded the way an edited one is: its
  // settings become the merchant settings for this draft, its contracts become the forms. What it
  // does not touch is anything already stored — nothing is written until Create is pressed.
  function loadSample(sample: VolumeContractSample) {
    applySettings(settingsFromConfig(sample.contract))
    const forms = (sample.contract.volume_contracts ?? [])
      .filter(isBuilderArchetype)
      .map(contractToForm)
    setContracts(forms.length ? forms : [emptyContract()])
    setDocName(sample.id)
    setDocDesc(sample.title)
    setShowIssues(false)
    setLoadedSample(sample.id)
  }

  // Clears the document only; the merchant-level settings are not the document's to reset.
  function resetBuilder() {
    setDocName('')
    setDocDesc('')
    setContracts([emptyContract()])
    setShowIssues(false)
    setLoadedSample(null)
  }

  // ── Mutations ───────────────────────────────────────────────────────────────
  async function handleSave() {
    if (!merchantId) return
    const failing = contracts.find(
      (c) => Object.keys(contractProblems.get(c.key) ?? {}).length > 0,
    )
    if (failing || Object.keys(documentIssues).length > 0) {
      setShowIssues(true)
      // Only jump to a contract when one is actually at fault; a missing document name is not.
      if (failing) setSelectedKey(failing.key)
      setSubmitError('Fix the highlighted fields, then save again.')
      return
    }
    setSubmitting(true)
    setSubmitError(null)
    setActionSuccess(null)
    try {
      const algorithm = { type: 'volume_contract', data: buildConfig() }
      if (editingId) {
        // `algorithm_for` is not sent: the handler keeps the slot the row already occupies.
        await apiPost('/routing/update', {
          created_by: merchantId,
          routing_algorithm_id: editingId,
          name: docName.trim(),
          description: docDesc.trim() || `Volume commitments for ${merchantId}`,
          algorithm,
        })
        setActionSuccess(`“${docName.trim()}” updated — activate it to hand the change to the routing engine.`)
      } else {
        const payload: CreateRoutingRequest = {
          name: docName.trim(),
          description: docDesc.trim() || `Volume commitments for ${merchantId}`,
          created_by: merchantId,
          algorithm_for: 'volume_commitment',
          algorithm,
        }
        await apiPost('/routing/create', payload)
        setActionSuccess(`“${docName.trim()}” created — activate it from the list to hand it to the routing engine.`)
      }
      resetBuilder()
      revalidate()
      closeBuilder()
    } catch (e) {
      const verb = editingId ? 'update' : 'create'
      setSubmitError(e instanceof Error ? e.message : `Failed to ${verb} contract document`)
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
  /** An input's problem, shown under the input it belongs to. */
  function fieldError(message?: string) {
    if (!message) return null
    return <p className="mt-1 text-[11px] text-red-600 dark:text-red-400">{message}</p>
  }

  function fieldLabel(text: string, hint?: string) {
    return (
      <div className="mb-1.5 flex items-baseline justify-between gap-2">
        <span className={type.label}>{text}</span>
        {hint ? <span className="text-[11px] text-slate-400 dark:text-[#8d96aa]">{hint}</span> : null}
      </div>
    )
  }

  /**
   * The settings every amount in this document is read against. Shown on the builder because a
   * target of `500000` is $500k or $5k depending on `amountUnits`, and is not money at all under
   * `metric: volume`.
   */
  const inheritedSettings = [
    { label: 'Metric', value: metric === 'volume' ? 'Transaction count' : 'GMV' },
    ...(metric === 'volume'
      ? []
      : [{ label: 'Currency', value: `${currency.toUpperCase()} (${amountUnits} units)` }]),
    { label: 'Tolerance', value: `${tolerancePp || '0'}pp` },
    {
      label: 'Expected daily traffic',
      value: expectedDailyTraffic.trim() ? formatEntered(expectedDailyTraffic) : 'not set',
      invalid: showIssues && !expectedDailyTraffic.trim(),
    },
    { label: 'Routing mode', value: routingMode === 'pace_guarded' ? 'Pace-guarded' : 'Commitment-first' },
  ]

  const unitHint = metric === 'volume' ? 'transaction count' : `${amountUnits} ${currency.toUpperCase()} units`

  function openView(which: string) {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      next.set('contract', which)
      return next
    })
  }
  function openBuilder() {
    openView('new')
  }
  function openEditor(documentId: string) {
    openView(documentId)
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

  /** One contract's fields. Rendered for whichever row the summary table has selected. */
  function contractFields(contract: ContractForm, errors: ContractIssues) {
    return (
      <>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <div>
          {fieldLabel('Contract ID')}
          <input
            className={errors.id ? inputInvalidClass : inputClass}
            placeholder="adyen_2026_lumpsum"
            value={contract.id}
            onChange={(e) => patchContract(contract.key, { id: e.target.value })}
          />
          {fieldError(errors.id)}
        </div>
        <div>
          {fieldLabel('Connector', 'exact gateway name')}
          <Combobox
            className={errors.connector ? inputInvalidClass : inputClass}
            value={contract.connector}
            onChange={(v) => patchContract(contract.key, { connector: v })}
            options={CONNECTOR_SUGGESTIONS}
            placeholder="adyen"
          />
          {fieldError(errors.connector)}
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
            className={errors.anchor ? inputInvalidClass : inputClass}
            type="number"
            min={1}
            value={contract.anchor}
            onChange={(e) => patchContract(contract.key, { anchor: e.target.value })}
          />
          {fieldError(errors.anchor)}
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
            className={errors.timezone ? inputInvalidClass : inputClass}
            value={contract.timezone}
            onChange={(v) => patchContract(contract.key, { timezone: v })}
            options={TIMEZONE_SUGGESTIONS}
            placeholder="UTC"
          />
          {fieldError(errors.timezone)}
        </div>
      </div>

      <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <div>
          {fieldLabel('Contract type')}
          <SearchableSelect
            triggerClassName={inputClass}
            value={contract.archetype}
            onChange={(v) =>
              patchContract(contract.key, { archetype: v as ContractForm['archetype'] })
            }
            options={[
              { value: 'lumpsum', label: 'Lumpsum — reward on hitting a target' },
              { value: 'tiered', label: 'Tiered — rebate ladder by threshold' },
            ]}
          />
        </div>
      </div>

      {contract.archetype === 'lumpsum' ? (
        <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <div>
            {fieldLabel('Target', unitHint)}
            <input
              className={errors.target ? inputInvalidClass : inputClass}
              placeholder="6000000"
              value={contract.target}
              onChange={(e) => patchContract(contract.key, { target: e.target.value })}
            />
            {fieldError(errors.target)}
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
                className={errors.reward ? inputInvalidClass : inputClass}
                placeholder="15000"
                value={contract.flatAmount}
                onChange={(e) => patchContract(contract.key, { flatAmount: e.target.value })}
              />
              {fieldError(errors.reward)}
            </div>
          ) : (
            <div>
              {fieldLabel('Rebate', 'basis points')}
              <input
                className={errors.reward ? inputInvalidClass : inputClass}
                type="number"
                min={1}
                max={10000}
                placeholder="25"
                value={contract.rebateBps}
                onChange={(e) => patchContract(contract.key, { rebateBps: e.target.value })}
              />
              {fieldError(errors.reward)}
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
                    className={errors[`tier${tierIdx}.threshold`] ? inputInvalidClass : inputClass}
                    placeholder="8000000"
                    value={tier.threshold}
                    onChange={(e) => patchTier(contract.key, tierIdx, { threshold: e.target.value })}
                  />
                  {fieldError(errors[`tier${tierIdx}.threshold`])}
                </div>
                <div>
                  {fieldLabel(tier.kind === 'retroactive' ? 'Rebate' : 'Rate', 'bps')}
                  <input
                    className={errors[`tier${tierIdx}.bps`] ? inputInvalidClass : inputClass}
                    type="number"
                    min={1}
                    max={10000}
                    placeholder="20"
                    value={tier.bps}
                    onChange={(e) => patchTier(contract.key, tierIdx, { bps: e.target.value })}
                  />
                  {fieldError(errors[`tier${tierIdx}.bps`])}
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
          {fieldError(errors.tiers)}
        </div>
      )}
      </>
    )
  }

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
              title={editingId ? 'Edit Contract Document' : 'Create Contract Document'}
              description="Express PSP volume-commitment contracts — goals, rebates and billing cycles. Nothing here changes routing until the document is activated."
              className="truncate"
            />
          </div>
        </div>

        {editingId && !editingDoc && algorithms && (
          <ErrorMessage error="That contract document no longer exists for this merchant." />
        )}
        {editingActiveDoc && (
          <Notice tone="warning">
            <strong>This document is active</strong> — active documents cannot be edited. Deactivate
            it from the contracts list first, then come back.
          </Notice>
        )}
        {editingUnsupported && (
          <Notice tone="warning">
            <strong>This document uses a minimum-commitment contract</strong>, which this builder has
            no controls for. Editing it here would drop that contract, so it stays read-only — change
            it through the routing API instead.
          </Notice>
        )}

        <div className="space-y-6">
          {/* Merchant-level settings: set once for the merchant; the values live on their own
              screen. They are shown here because every amount typed below is read in these units
              — a target means nothing without the metric and unit it is counted in. */}
          <InsetPanel>
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <SlidersHorizontal size={14} className="text-slate-400 dark:text-[#8a8a93]" />
                <span className={type.label}>Merchant-level settings</span>
                <span className="text-sm text-slate-500 dark:text-[#8d96aa]">
                  {editingId
                    ? '— read back from this document and saved with it.'
                    : expectedDailyTraffic.trim()
                    ? '— applied to this document when it is created.'
                    : '— expected daily traffic is not set yet; the document cannot be created without it.'}
                </span>
              </div>
              {/* Hidden while editing: the settings screen is its own view, so opening it would
                  drop the document being edited along with any unsaved changes to it. */}
              {!editingId && (
                <Button variant="secondary" size="sm" onClick={openSettings}>
                  Edit settings
                </Button>
              )}
            </div>
            <dl className="mt-3 flex flex-wrap gap-x-6 gap-y-2 border-t border-slate-200 pt-3 text-sm dark:border-[#222226]">
              {inheritedSettings.map((setting) => (
                <div key={setting.label} className="flex items-baseline gap-1.5">
                  <dt className="text-slate-500 dark:text-[#78849a]">{setting.label}</dt>
                  <dd
                    className={
                      'invalid' in setting && setting.invalid
                        ? 'font-medium text-red-600 dark:text-red-400'
                        : 'font-medium text-slate-800 dark:text-[#e6e6ea]'
                    }
                  >
                    {setting.value}
                  </dd>
                </div>
              ))}
            </dl>
            {showIssues && fieldError(documentIssues.expectedDailyTraffic)}
          </InsetPanel>

          {/* Templates. Absent in production, where the deployment configures none. */}
          {!editingId && samples.length > 0 && (
            <InsetPanel>
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <span className="text-sm font-medium text-slate-800 dark:text-white">
                  Start from a template
                </span>
                <span className="text-xs text-slate-500 dark:text-[#78849a]">
                  Each one puts the engine in a different situation. Nothing is written until you
                  press Create.
                </span>
              </div>
              <div className="mt-3 grid gap-3 sm:grid-cols-2">
                {samples.map((sample) => {
                  const chosen = loadedSample === sample.id
                  return (
                    <button
                      key={sample.id}
                      type="button"
                      onClick={() => loadSample(sample)}
                      className={`rounded-xl border px-3.5 py-3 text-left transition-colors ${
                        chosen
                          ? 'border-brand-500 bg-brand-500/5'
                          : 'border-slate-200 hover:border-slate-300 dark:border-[#222226] dark:hover:border-[#33333d]'
                      }`}
                    >
                      <span className="flex items-center gap-2 text-sm font-medium text-slate-800 dark:text-white">
                        {sample.title}
                        {chosen && <Badge variant="blue">Loaded</Badge>}
                      </span>
                      <span className="mt-1 block text-xs text-slate-600 dark:text-[#8d96a8]">
                        {sample.summary}
                      </span>
                      <span className="mt-2 block text-xs text-slate-500 dark:text-[#78849a]">
                        <strong className="font-medium">What to watch:</strong>{' '}
                        {sample.expectedOutcome}
                      </span>
                    </button>
                  )
                })}
              </div>
            </InsetPanel>
          )}

          {/* Document */}
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              {fieldLabel('Name')}
              <input
                className={showIssues && documentIssues.name ? inputInvalidClass : inputClass}
                placeholder="q3_volume_contracts"
                value={docName}
                onChange={(e) => setDocName(e.target.value)}
              />
              {showIssues && fieldError(documentIssues.name)}
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

          {/* Per-PSP contracts. The table is the whole document at a glance; selecting a row
              expands that contract's fields inline, at the full width of the page. The fields
              write directly into the form, so a contract has no save of its own — the document's
              Save is the only commit. */}
          <div className="space-y-4">
            <Card className="!rounded-[18px]">
              <div className="flex items-center justify-between gap-3 border-b border-slate-100 px-4 py-3 dark:border-[#1e2330]">
                <span className={type.label}>Contracts ({contracts.length})</span>
                <Button size="sm" variant="secondary" onClick={addContract}>
                  <Plus size={13} /> Add contract
                </Button>
              </div>
              <div className="overflow-x-auto">
                <table className="w-full min-w-[680px] table-fixed text-left">
                  <thead>
                    <tr className="border-b border-slate-100 text-[11px] font-semibold uppercase leading-4 tracking-wide text-slate-500 dark:border-[#1e2330] dark:text-[#78849a]">
                      <th className="w-[20%] px-4 py-3">Contract ID</th>
                      <th className="w-[12%] px-4 py-3">Connector</th>
                      <th className="hidden w-[9%] px-4 py-3 sm:table-cell">Status</th>
                      <th className="hidden w-[14%] px-4 py-3 sm:table-cell">Type</th>
                      <th className="hidden w-[10%] px-4 py-3 lg:table-cell">Cycle</th>
                      <th className="hidden w-[13%] px-4 py-3 lg:table-cell">Timezone</th>
                      <th className="hidden w-[11%] px-4 py-3 lg:table-cell">Reward</th>
                      <th className="w-[11%] px-4 py-3 text-right">
                        Target
                        {/* The unit the typed amounts are in — without it a target is unreadable. */}
                        <span className="block text-[10px] font-medium normal-case tracking-normal text-slate-400 dark:text-[#6b7688]">
                          {unitHint}
                        </span>
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {contracts.map((contract, contractIdx) => {
                      const issues = contractProblems.get(contract.key) ?? {}
                      const flagged = showIssues && Object.keys(issues).length > 0
                      const isExpanded = contract.key === selectedKey
                      return (
                        <Fragment key={contract.key}>
                          <tr
                            onClick={() => setSelectedKey(isExpanded ? null : contract.key)}
                            className={`cursor-pointer border-b border-slate-100 text-sm dark:border-[#1e2330] ${
                              isExpanded
                                ? 'bg-brand-50/70 dark:bg-brand-500/10'
                                : 'hover:bg-slate-50 dark:hover:bg-[#16161a]'
                            }`}
                          >
                            <td className="px-4 py-3">
                              <div className="flex min-w-0 items-center gap-1.5">
                                {isExpanded ? (
                                  <ChevronDown size={14} className="shrink-0 text-slate-400" />
                                ) : (
                                  <ChevronRight size={14} className="shrink-0 text-slate-400" />
                                )}
                                {flagged && (
                                  <AlertCircle
                                    size={13}
                                    className="shrink-0 text-red-500"
                                    aria-label={Object.values(issues).join('; ')}
                                  />
                                )}
                                <span
                                  className={`truncate ${
                                    contract.id.trim()
                                      ? 'font-medium text-slate-800 dark:text-[#e6e6ea]'
                                      : 'text-slate-400 dark:text-[#6b7688]'
                                  }`}
                                >
                                  {contract.id.trim() || `Contract ${contractIdx + 1}`}
                                </span>
                              </div>
                            </td>
                            <td className="truncate px-4 py-3 text-slate-600 dark:text-[#9ca7ba]">
                              {contract.connector.trim() || '—'}
                            </td>
                            <td className="hidden px-4 py-3 sm:table-cell">
                              <span
                                className={`inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium ${
                                  contract.status === 'active'
                                    ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                                    : 'bg-slate-400/10 text-slate-500 dark:text-[#8d96aa]'
                                }`}
                              >
                                {contract.status === 'active' ? 'Active' : 'Inactive'}
                              </span>
                            </td>
                            <td className="hidden px-4 py-3 text-slate-600 sm:table-cell dark:text-[#9ca7ba]">
                              {ARCHETYPE_LABELS[contract.archetype]}
                              {/* A tiered contract is steered at one tier, so name which one. */}
                              {contract.archetype === 'tiered' && contract.tiers.length > 1 && (
                                <span className="text-slate-400 dark:text-[#6b7688]">
                                  {' '}
                                  (tier{' '}
                                  {Math.max(0, contract.tiers.findIndex((tier) => tier.targeted)) + 1} of{' '}
                                  {contract.tiers.length})
                                </span>
                              )}
                            </td>
                            <td className="hidden truncate px-4 py-3 text-slate-600 lg:table-cell dark:text-[#9ca7ba]">
                              {cycleSummary(contract)}
                            </td>
                            <td className="hidden truncate px-4 py-3 text-slate-600 lg:table-cell dark:text-[#9ca7ba]">
                              {/* A test cycle repeats from the epoch, so its timezone is ignored. */}
                              {contract.cycleType === 'test_minutes'
                                ? '—'
                                : contract.timezone.trim() || '—'}
                            </td>
                            <td className="hidden truncate px-4 py-3 text-slate-600 lg:table-cell dark:text-[#9ca7ba]">
                              {rewardSummary(contract)}
                            </td>
                            <td className="px-4 py-3 text-right font-medium tabular-nums text-slate-800 dark:text-[#e6e6ea]">
                              {formatEntered(targetOf(contract))}
                            </td>
                          </tr>
                          {isExpanded && (
                            <tr className="border-b border-slate-100 dark:border-[#1e2330]">
                              <td colSpan={8} className="bg-slate-50/70 px-5 py-4 dark:bg-[#131317]">
                                {contractFields(contract, flagged ? issues : {})}

                                <div className="mt-5 border-t border-slate-200 pt-4 dark:border-[#222226]">
                                  <button
                                    type="button"
                                    onClick={() => removeContract(contract.key)}
                                    disabled={contracts.length < 2}
                                    className="inline-flex items-center gap-1.5 text-sm font-medium text-red-500 transition-colors hover:text-red-600 disabled:cursor-not-allowed disabled:text-slate-300 dark:disabled:text-[#3a3a42]"
                                    title={
                                      contracts.length < 2
                                        ? 'A document needs at least one contract'
                                        : undefined
                                    }
                                  >
                                    <Trash2 size={14} /> Remove contract
                                  </button>
                                </div>
                              </td>
                            </tr>
                          )}
                        </Fragment>
                      )
                    })}
                  </tbody>
                </table>
              </div>
            </Card>

            <ErrorMessage error={submitError} />
            <div className="flex flex-wrap items-center gap-4">
              <Button
                onClick={handleSave}
                disabled={
                  submitting ||
                  !merchantId ||
                  !canEditRouting ||
                  editingActiveDoc ||
                  editingUnsupported ||
                  (editingId != null && !editingDoc)
                }
              >
                {submitting ? <Spinner size={14} /> : null}{' '}
                {submitting ? 'Saving...' : editingId ? 'Save Changes' : 'Create Contract'}
              </Button>
              {!editingId && (
                <Button variant="secondary" size="sm" onClick={resetBuilder} disabled={submitting}>
                  Clear
                </Button>
              )}
              <button
                type="button"
                onClick={closeBuilder}
                className="text-sm font-medium text-slate-500 transition-colors hover:text-slate-800 dark:text-[#8d96a8] dark:hover:text-white"
              >
                Cancel
              </button>
              <p className="max-w-[57ch] text-sm text-slate-500 dark:text-[#78849a]">
                {!expectedDailyTraffic.trim()
                  ? 'Set the expected daily traffic under Merchant settings before creating a document.'
                  : editingId
                  ? 'Saved changes reach the routing engine when this document is activated.'
                  : 'A new document is created inactive — activate it from the contracts list.'}
              </p>
            </div>
          </div>
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

      <VolumeContractFeatureNotice merchantId={merchantId} />
      <VolumeContractActivationSummary merchantId={merchantId} />

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
                  // /routing/delete and /routing/update both reject an active document, so the
                  // controls mirror that.
                  const lockedReason = isActive
                    ? 'Deactivate this document first'
                    : !canEditRouting
                    ? 'You do not have permission to change routing'
                    : undefined
                  const isBuilderDocument = (data?.volume_contracts ?? []).every(isBuilderArchetype)
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
                            {
                              label: 'Edit',
                              icon: Pencil,
                              onSelect: () => openEditor(doc.id),
                              // /routing/update rejects an active document, and the builder has no
                              // controls for a min_commitment contract.
                              disabled: Boolean(lockedReason) || !isBuilderDocument,
                              hint: lockedReason ?? (isBuilderDocument ? undefined : 'This document uses a minimum-commitment contract'),
                            },
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
