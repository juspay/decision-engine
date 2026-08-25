import { useMemo, useState } from 'react'
import useSWR, { useSWRConfig } from 'swr'
import {
  CalendarClock,
  ChevronDown,
  ChevronUp,
  Layers,
  Plus,
  Target,
  Trash2,
} from 'lucide-react'
import { apiPost } from '../../lib/api'
import { useMerchantStore } from '../../store/merchantStore'
import { useCanEditRouting } from '../../store/authStore'
import type {
  CreateRoutingRequest,
  RoutingAlgorithm,
  VolumeContract,
  VolumeContractConfig,
  VolumeContractTier,
} from '../../types/api'
import { Card, CardBody, CardHeader, InsetPanel } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
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

export function VolumeContractsPage() {
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
  const [routingMode, setRoutingMode] = useState<'pace_guarded' | 'volume_commitment'>('pace_guarded')
  const [tolerancePp, setTolerancePp] = useState('5')
  const [metric, setMetric] = useState<'gmv' | 'volume'>('gmv')
  const [currency, setCurrency] = useState('USD')
  const [amountUnits, setAmountUnits] = useState<'major' | 'minor'>('major')
  const [expectedDailyTraffic, setExpectedDailyTraffic] = useState('')
  const [forecastInterval, setForecastInterval] = useState('')
  const [contracts, setContracts] = useState<ContractForm[]>([emptyContract()])

  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [createdName, setCreatedName] = useState<string | null>(null)

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
      routing_mode: routingMode,
      tolerance: `${Math.round(parseFloat(tolerancePp || '0') * 100)}bps`,
      metric,
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
    return config
  }

  function resetBuilder() {
    setDocName('')
    setDocDesc('')
    setRoutingMode('pace_guarded')
    setTolerancePp('5')
    setMetric('gmv')
    setCurrency('USD')
    setAmountUnits('major')
    setExpectedDailyTraffic('')
    setForecastInterval('')
    setContracts([emptyContract()])
  }

  // ── Mutations ───────────────────────────────────────────────────────────────
  async function handleCreate() {
    if (!merchantId) return
    setSubmitting(true)
    setSubmitError(null)
    setCreatedName(null)
    try {
      const payload: CreateRoutingRequest = {
        name: docName.trim(),
        description: docDesc.trim() || `Volume commitments for ${merchantId}`,
        created_by: merchantId,
        algorithm_for: 'volume_commitment',
        algorithm: { type: 'volume_contract', data: buildConfig() },
      }
      await apiPost('/routing/create', payload)
      setCreatedName(docName.trim())
      resetBuilder()
      revalidate()
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

  const unitHint = metric === 'volume' ? 'transaction count' : `${amountUnits} ${currency.toUpperCase()} units`

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight text-slate-900 dark:text-white">Volume Contracts</h1>
          <p className={`mt-1 ${type.subheading}`}>
            Express PSP volume-commitment contracts — goals, rebates and billing cycles. The routing engine
            reads the active document to pace and steer traffic; nothing here changes routing until activated.
          </p>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
        {/* ── Existing documents ── */}
        <Card className="xl:col-span-1">
          <CardHeader>
            <div className="flex items-center gap-2.5">
              <Layers size={18} className="text-slate-400 dark:text-[#8a8a93]" />
              <h2 className={type.heading}>Contract Documents</h2>
            </div>
          </CardHeader>
          <CardBody>
            {!merchantId ? (
              <p className={type.hint}>Set a merchant ID to see its contract documents.</p>
            ) : isLoading ? (
              <div className="flex items-center gap-2 py-6 text-sm text-slate-500">
                <Spinner size={16} /> Loading…
              </div>
            ) : documents.length === 0 ? (
              <p className={type.hint}>No contract documents yet. Build one on the right.</p>
            ) : (
              <ul className="space-y-2">
                {documents.map((doc) => {
                  const isActive = doc.id === activeDocumentId
                  const isExpanded = expandedId === doc.id
                  const data = (doc.algorithm_data ?? doc.algorithm)?.data as VolumeContractConfig | undefined
                  return (
                    <li
                      key={doc.id}
                      className={`rounded-xl border px-3 py-2.5 transition-colors ${
                        isActive
                          ? 'border-emerald-200 bg-emerald-50/50 dark:border-emerald-500/25 dark:bg-emerald-500/5'
                          : 'border-slate-200 dark:border-[#222226]'
                      }`}
                    >
                      <div className="flex items-center justify-between gap-2">
                        <button
                          type="button"
                          className="min-w-0 flex-1 text-left"
                          onClick={() => setExpandedId(isExpanded ? null : doc.id)}
                        >
                          <span className="block truncate text-sm font-medium text-slate-800 dark:text-white">
                            {doc.name}
                          </span>
                          <span className="block text-[11px] text-slate-400 dark:text-[#8d96aa]">
                            {data?.volume_contracts?.length ?? 0} contract{(data?.volume_contracts?.length ?? 0) === 1 ? '' : 's'}
                            {doc.created_at ? ` · ${new Date(doc.created_at).toLocaleDateString()}` : ''}
                          </span>
                        </button>
                        {isActive ? <Badge variant="green">Active</Badge> : <Badge variant="gray">Inactive</Badge>}
                        <button
                          type="button"
                          className="text-slate-400 hover:text-slate-600 dark:hover:text-white"
                          onClick={() => setExpandedId(isExpanded ? null : doc.id)}
                          aria-label={isExpanded ? 'Collapse' : 'Expand'}
                        >
                          {isExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                        </button>
                      </div>

                      {isExpanded ? (
                        <div className="mt-2 space-y-2">
                          <pre className="max-h-64 overflow-auto rounded-lg bg-slate-50 p-2.5 text-[11px] leading-relaxed text-slate-600 dark:bg-[#0a0d12] dark:text-[#9ca7ba]">
                            {JSON.stringify(data, null, 2)}
                          </pre>
                          <div className="flex flex-wrap items-center gap-2">
                            {isActive ? (
                              <Button
                                size="sm"
                                variant="danger"
                                disabled={deactivatingId === doc.id || !canEditRouting}
                                onClick={() => setPendingDeactivateId(doc.id)}
                              >
                                {deactivatingId === doc.id ? <Spinner size={12} /> : null} Deactivate
                              </Button>
                            ) : (
                              <>
                                <Button
                                  size="sm"
                                  disabled={activatingId === doc.id || !canEditRouting}
                                  onClick={() =>
                                    activeDocumentId ? setPendingActivateId(doc.id) : doActivate(doc.id)
                                  }
                                >
                                  {activatingId === doc.id ? <Spinner size={12} /> : null} Activate
                                </Button>
                                <Button
                                  size="sm"
                                  variant="ghost"
                                  disabled={deletingId === doc.id || !canEditRouting}
                                  onClick={() => setPendingDeleteId(doc.id)}
                                >
                                  <Trash2 size={12} /> Delete
                                </Button>
                              </>
                            )}
                          </div>
                        </div>
                      ) : null}
                    </li>
                  )
                })}
              </ul>
            )}
            <div className="mt-3">
              <ErrorMessage error={actionError} />
            </div>
          </CardBody>
        </Card>

        {/* ── Builder ── */}
        <Card className="xl:col-span-2">
          <CardHeader>
            <div className="flex items-center gap-2.5">
              <Target size={18} className="text-slate-400 dark:text-[#8a8a93]" />
              <h2 className={type.heading}>New Contract Document</h2>
            </div>
          </CardHeader>
          <CardBody>
            <div className="space-y-6">
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

              {/* Merchant-level settings */}
              <InsetPanel>
                <div className="mb-3 flex items-center gap-2">
                  <CalendarClock size={14} className="text-slate-400 dark:text-[#8a8a93]" />
                  <span className={type.label}>Merchant-level settings</span>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  <div>
                    {fieldLabel('Routing mode')}
                    <SearchableSelect
                      triggerClassName={inputClass}
                      value={routingMode}
                      onChange={(v) => setRoutingMode(v as typeof routingMode)}
                      options={[
                        { value: 'pace_guarded', label: 'Pace-guarded (auth-rate first)' },
                        { value: 'volume_commitment', label: 'Volume-commitment first' },
                      ]}
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
                    {fieldLabel('Metric')}
                    <SearchableSelect
                      triggerClassName={inputClass}
                      value={metric}
                      onChange={(v) => setMetric(v as typeof metric)}
                      options={[
                        { value: 'gmv', label: 'GMV (money processed)' },
                        { value: 'volume', label: 'Volume (transaction count)' },
                      ]}
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
                </div>
              </InsetPanel>

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

              {/* Submit */}
              <div className="space-y-3 border-t border-slate-100 pt-4 dark:border-[#1a1d25]">
                <ErrorMessage error={submitError} />
                {createdName ? (
                  <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-600 dark:text-emerald-400">
                    “{createdName}” created. Activate it from the list to hand it to the routing engine.
                  </div>
                ) : null}
                <div className="flex items-center justify-end gap-3">
                  <Button variant="ghost" onClick={resetBuilder} disabled={submitting}>
                    Reset
                  </Button>
                  <Button
                    onClick={handleCreate}
                    disabled={submitting || !merchantId || !docName.trim() || !canEditRouting}
                  >
                    {submitting ? <Spinner size={14} /> : null} Create Document
                  </Button>
                </div>
              </div>
            </div>
          </CardBody>
        </Card>
      </div>

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
