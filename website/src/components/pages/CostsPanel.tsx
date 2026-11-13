import { Fragment, useEffect, useMemo, useState, type ReactNode } from 'react'
import {
  ArrowRight,
  Check,
  ChevronDown,
  ChevronRight,
  CreditCard,
  Eye,
  Pencil,
  Plus,
  RotateCcw,
  X,
} from 'lucide-react'
import { Card } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  deleteClusterOverride,
  deleteFeeOverride,
  resetSeedCosts,
  saveSeedCosts,
  setClusterOverride,
  setFeeOverride,
  countActiveFilters,
  useClusterFacets,
  useConnectorFees,
  useCostClusters,
  useSeedCosts,
  type ClusterFacet,
  type ClusterFee,
  type ClusterFilters,
  type ConnectorFee,
  type SeedCostRow,
} from '../../hooks/useCostRouting'
import { ClusterFilterBar } from './ClusterFilterBar'
import { inputClass } from './CostRoutingShared'
import {
  blankRow,
  bpsToPct,
  ccySymbol,
  formatFee,
  pctText,
  pctToBps,
  PctInput,
  ScenarioModal,
  SeedRow,
  totalRate,
} from './SeedCostShared'

// Connectors a merchant can set a manual blended fee for even before any settlement report is
// ingested (e.g. Stripe from contract terms). The list a fee can be *added* against; connectors
// that already have a fit/override show up from the API regardless.
const KNOWN_CONNECTORS = ['adyen', 'stripe', 'checkout', 'worldpay', 'braintree', 'chase', 'cybersource']

// Pull enough clusters to aggregate per-connector Volume/Txns and populate the detail table from one
// merchant-wide fetch (grouped client-side) rather than one call per connector. This has to cover the
// merchant's WHOLE cluster set or the per-connector totals under-report — a real single-account report
// fits ~1.6k clusters. (It previously asked for 500 and was silently clamped to 50 by the API ceiling,
// so those tiles were summing only the top 50 clusters.)
const CLUSTER_LIMIT = 2000

// Rows per page in the detail table. The list is server-ordered by settled volume, so page 1 is the
// set of segments actually worth correcting.
const PAGE_SIZE = 10

const NETWORK_LABELS: Record<string, string> = {
  mc: 'Mastercard',
  mastercard: 'Mastercard',
  visa: 'Visa',
  amex: 'Amex',
  mada: 'Mada',
  diners: 'Diners',
  discover: 'Discover',
  jcb: 'JCB',
}

function titleCase(s: string): string {
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s
}

/** The card-program proxy as a rate: the issuer BIN's dominant interchange, blank when it has none. */
function programRate(cardProduct: string | null | undefined): string {
  const bps = Number(cardProduct)
  if (!cardProduct || !Number.isFinite(bps) || bps <= 0) return '—'
  return pctText(bps)
}

function networkLabel(network: string): string {
  return NETWORK_LABELS[network.toLowerCase()] ?? network.toUpperCase()
}

/**
 * What the Network column shows. Cards have a brand; wallets and APMs (Alipay, iDEAL, Klarna) do not
 * — Adyen leaves `Global Card Brand` empty and identifies them only by `Payment Method Variant`. Such
 * a row also has no funding type, issuer country or interchange category, so without this fallback
 * EVERY cell reads "—" and nothing on the row says what it is.
 */
function methodLabel(c: ClusterFee): string {
  if (c.card_network?.trim()) return networkLabel(c.card_network)
  const v = (c.variant ?? '').trim().toLowerCase()
  if (!v) return '—'
  // A blank brand does not always mean "not a card": Adyen also leaves it empty on some card rows
  // whose variant still names the network (`visadebit`). Recover it from the prefix when we can, so
  // those read "Visa" rather than "Visadebit"; a genuine wallet (`alipay`) has no prefix and keeps
  // its own name, which is the right answer for a non-card method anyway.
  const net = Object.keys(NETWORK_LABELS).find((k) => v.startsWith(k))
  return net ? NETWORK_LABELS[net] : titleCase(v)
}

/** Compact magnitude: 2.9M / 514.8K. */
function formatCompact(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`
  return n.toFixed(0)
}

type BadgeVariant = 'green' | 'gray' | 'blue' | 'red' | 'orange' | 'purple'
function verdictVariant(v: string): BadgeVariant {
  const k = v.toUpperCase()
  if (k === 'GOOD') return 'green'
  if (k === 'THIN') return 'orange'
  return 'red'
}

/** A seed row indexed by its position in the merchant's full seed table (edits target that index). */
type IndexedSeedRow = { row: SeedCostRow; index: number }

/** The default (dimensionless) seed row for a PSP, else its first scenario — the tile's baseline. */
function baselineSeedRow(rows: IndexedSeedRow[]): SeedCostRow | undefined {
  return (rows.find(({ row }) => row.is_default) ?? rows[0])?.row
}

/** Which rung of the decide-time ladder a connector's active fee comes from. */
type FeeSource = 'override' | 'reports' | 'contract' | 'none'
function feeSource(fee: ConnectorFee | undefined, hasContract: boolean): FeeSource {
  if (fee?.source === 'override') return 'override'
  if (fee?.model_pct_bps != null) return 'reports'
  if (hasContract) return 'contract'
  return 'none'
}
const SOURCE_LABELS: Record<FeeSource, string> = {
  override: 'MANUAL OVERRIDE',
  reports: 'FROM REPORTS',
  contract: 'CONTRACT BASELINE',
  none: 'NO FEE SET',
}

/**
 * Flat status pills: soft fill, coloured text, no ring. Deliberately not the shared `Badge`, which
 * draws a `ring-1 ring-inset` — the ring reads as an outlined chip and is a step louder than these
 * are meant to be next to a connector's name. `SeedCostShared`'s `Chip` exists for the same reason.
 */
type PillTone = 'indigo' | 'violet' | 'slate' | 'green'
const PILL_TONES: Record<PillTone, string> = {
  indigo: 'bg-indigo-50 text-indigo-600 dark:bg-indigo-500/12 dark:text-indigo-300',
  violet: 'bg-violet-50 text-violet-600 dark:bg-violet-500/12 dark:text-violet-300',
  slate: 'bg-slate-100 text-slate-500 dark:bg-white/[0.06] dark:text-slate-300',
  green: 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/12 dark:text-emerald-300',
}
function Pill({ tone, children }: { tone: PillTone; children: ReactNode }) {
  return (
    <span
      className={`inline-flex shrink-0 items-center rounded-md px-2 py-1 text-[11px] font-semibold uppercase tracking-wide ${PILL_TONES[tone]} leading-4`}
    >
      {children}
    </span>
  )
}

const SOURCE_TONES: Record<FeeSource, PillTone> = {
  override: 'violet',
  reports: 'indigo',
  contract: 'slate',
  none: 'slate',
}

/**
 * Costs — the one place a merchant configures what each connector charges. Laid out master–detail:
 * a strip of connector tiles carrying the **active fee** each would charge at decide time, and one
 * detail panel for the selected tile.
 *
 * The active fee follows the decide-time ladder (see `serving::lookup`): a **manual override** wins
 * over the fee **learned from reports**, which wins over the **contract baseline** seed rate. The
 * tile's badge names the rung it landed on; the detail panel is where the two editable layers
 * behind it live — learned segments (with per-segment overrides) and contract scenarios.
 */
export function CostsPanel({ merchantId }: { merchantId?: string }) {
  const { fees, isLoading, error, mutate } = useConnectorFees(merchantId)
  // Column filters, applied server-side and shared verbatim with the Ingested Data table (same bar,
  // same facets). This is the editable surface for per-cluster overrides, so reaching a low-traffic
  // cluster matters more here than anywhere else — it's the one you most want to give a contract rate.
  const [filters, setFilters] = useState<ClusterFilters>({})
  const { clusters, mutate: mutateClusters } = useCostClusters(merchantId, {
    limit: CLUSTER_LIMIT,
    order: 'gross_sum',
    filters,
  })
  const { facets } = useClusterFacets(merchantId)
  const { rows: seedRows, error: seedError, mutate: mutateSeed } = useSeedCosts(merchantId)
  const [picked, setPicked] = useState<string | null>(null)
  const [editingKey, setEditingKey] = useState<string | null>(null)
  const [feeModal, setFeeModal] = useState<FeeModalState | null>(null)

  // Contract-baseline editor. `index: null` ⇒ adding a new scenario for `connector`; a number ⇒
  // editing the seed table's row at that index. Persisted by PUTting the whole (small) seed table.
  const [seedEditor, setSeedEditor] = useState<{ connector: string; index: number | null; row: SeedCostRow } | null>(
    null,
  )
  const [seedBusy, setSeedBusy] = useState(false)
  const [seedSaveError, setSeedSaveError] = useState<string | null>(null)
  const [resetting, setResetting] = useState(false)

  // Group the merchant-wide clusters by connector so each tile reads its own aggregates and the
  // detail panel its own segments.
  const byConnector = useMemo(() => {
    const m = new Map<string, ClusterFee[]>()
    for (const c of clusters) {
      const arr = m.get(c.connector)
      if (arr) arr.push(c)
      else m.set(c.connector, [c])
    }
    return m
  }, [clusters])

  // Group seed rows by PSP, carrying each row's index in the full table (edits target that index).
  const seedByConnector = useMemo(() => {
    const m = new Map<string, IndexedSeedRow[]>()
    seedRows.forEach((row, index) => {
      const arr = m.get(row.psp)
      if (arr) arr.push({ row, index })
      else m.set(row.psp, [{ row, index }])
    })
    return m
  }, [seedRows])

  // Tile list = the union of connectors with a fee/model + PSPs that only have contract rates, so a
  // contract-only PSP (no credentials/reports) still gets a tile. Fee-carrying connectors come first.
  const feeConnectors = fees.map((f) => f.connector)
  const configured = new Set(feeConnectors)
  const seedOnlyPsps = Array.from(seedByConnector.keys()).filter((p) => p && !configured.has(p))
  const cardConnectors = [...feeConnectors, ...seedOnlyPsps]
  const known = new Set([...configured, ...seedByConnector.keys()])
  const addableConnectors = KNOWN_CONNECTORS.filter((c) => !known.has(c))

  // The selected connector, resolved rather than stored: the list arrives async and can change under
  // the selection (a connector gains a fee, contract rates are reset), so a stale pick falls back to
  // the first tile instead of leaving the detail panel pointed at nothing.
  const selected = picked && cardConnectors.includes(picked) ? picked : (cardConnectors[0] ?? null)
  const selectedFee = fees.find((f) => f.connector === selected)
  const selectedSeedRows = selected ? (seedByConnector.get(selected) ?? []) : []
  const selectedClusters = selected ? (byConnector.get(selected) ?? []) : []

  async function refresh() {
    await Promise.all([mutate(), mutateClusters(), mutateSeed()])
  }

  function select(connector: string) {
    setPicked(connector)
    setEditingKey(null)
  }

  // Persist the whole seed table after an add / edit / remove of one scenario.
  async function persistSeed(next: SeedCostRow[]) {
    if (!merchantId) return
    setSeedBusy(true)
    setSeedSaveError(null)
    try {
      await saveSeedCosts(merchantId, next)
      await mutateSeed()
      setSeedEditor(null)
    } catch (e) {
      setSeedSaveError(e instanceof Error ? e.message : 'Failed to save contract rates')
    } finally {
      setSeedBusy(false)
    }
  }

  function saveScenario(updated: SeedCostRow) {
    if (!seedEditor) return
    const next =
      seedEditor.index === null
        ? [...seedRows, updated]
        : seedRows.map((r, i) => (i === seedEditor.index ? updated : r))
    persistSeed(next)
  }

  function removeScenario() {
    if (!seedEditor || seedEditor.index === null) return
    persistSeed(seedRows.filter((_, i) => i !== seedEditor.index))
  }

  async function resetContractRates() {
    if (!merchantId) return
    setResetting(true)
    setSeedSaveError(null)
    try {
      await resetSeedCosts(merchantId)
      await mutateSeed()
    } catch (e) {
      setSeedSaveError(e instanceof Error ? e.message : 'Failed to reset contract rates')
    } finally {
      setResetting(false)
    }
  }

  /** Open the connector-wide override editor, pre-filled with whatever that connector charges now. */
  function editConnectorFee(connector: string) {
    const fee = fees.find((f) => f.connector === connector)
    const baseline = baselineSeedRow(seedByConnector.get(connector) ?? [])
    setFeeModal({
      connector,
      isNew: false,
      initialPctBps: fee?.override_pct_bps ?? fee?.model_pct_bps ?? (baseline ? totalRate(baseline) : 0),
      initialFixed: fee?.override_fixed ?? fee?.model_fixed ?? baseline?.fixed ?? 0,
      canClear: fee?.source === 'override',
    })
  }

  const hasSeed = seedRows.length > 0

  return (
    <div className="space-y-5">
      {/* Page header */}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900 dark:text-white">Costs</h2>
          <p className="mt-1 max-w-2xl text-sm text-slate-500 dark:text-[#9ca7ba]">
            What each connector costs at decide time, and where that number comes from. Pick a
            connector to override a learned fee or edit its contract baseline.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {merchantId && hasSeed && (
            <Button variant="secondary" size="sm" onClick={resetContractRates} disabled={resetting}>
              {resetting ? <Spinner size={13} /> : <RotateCcw size={13} />} Reset contract rates
            </Button>
          )}
          {merchantId && addableConnectors.length > 0 && (
            <Button
              variant="primary"
              size="sm"
              onClick={() =>
                setFeeModal({
                  connector: addableConnectors[0],
                  connectors: addableConnectors,
                  isNew: true,
                  initialPctBps: 0,
                  initialFixed: 0,
                  canClear: false,
                })
              }
            >
              <Plus size={14} /> Add connector
            </Button>
          )}
        </div>
      </div>

      {/* Precedence hint — the one idea that resolves "which layer do I edit?": the active fee is
          resolved top-down, an override beating a learned fee beating the contract baseline. */}
      <PrecedenceLegend />

      {!merchantId ? (
        <Card>
          <p className="px-5 py-4 text-sm text-slate-500">Set a merchant ID to view connector costs.</p>
        </Card>
      ) : isLoading ? (
        <div className="flex items-center gap-2 py-4 text-sm text-slate-500">
          <Spinner size={16} /> Loading connectors…
        </div>
      ) : cardConnectors.length === 0 ? (
        <Card>
          <p className="px-5 py-4 text-sm text-slate-500 max-w-[57ch]">
            No connectors yet. Add one to set its fee or contract baseline, or configure ingestion
            below to learn fees from settlement reports.
          </p>
        </Card>
      ) : (
        <>
          {/* Connector list — one row per connector, carrying its active fee. A list rather than a
              grid of tiles because the fees are a column of numbers read by comparison: scanning
              them vertically is the whole job, and a contract-only connector has no stats to fill
              a tile with. */}
          <Card className="!rounded-2xl overflow-hidden">
            <div className="divide-y divide-slate-100 dark:divide-[#1c1c23]">
              {cardConnectors.map((connector) => {
                const fee = fees.find((f) => f.connector === connector)
                const rows = seedByConnector.get(connector) ?? []
                return (
                  <ConnectorRow
                    key={connector}
                    connector={connector}
                    fee={fee}
                    clusters={byConnector.get(connector) ?? []}
                    seedRows={rows}
                    isSelected={connector === selected}
                    onSelect={() => select(connector)}
                    onEditFee={() => editConnectorFee(connector)}
                  />
                )
              })}
            </div>
          </Card>

          {selected && (
            <ConnectorDetail
              merchantId={merchantId}
              connector={selected}
              fee={selectedFee}
              clusters={selectedClusters}
              seedRows={selectedSeedRows}
              filters={filters}
              facets={facets}
              onFiltersChange={setFilters}
              editingKey={editingKey}
              setEditingKey={setEditingKey}
              onRefresh={refresh}
              onAddScenario={() => {
                setSeedSaveError(null)
                setSeedEditor({ connector: selected, index: null, row: blankRow(selected) })
              }}
              onEditScenario={(index) => {
                setSeedSaveError(null)
                setSeedEditor({ connector: selected, index, row: { ...seedRows[index] } })
              }}
            />
          )}
        </>
      )}

      <p className="px-1 text-xs text-slate-500 max-w-[57ch]">
        Rates are a percentage of the transaction amount; fit error stays in basis points (100 bps =
        1%) because it is a residual, not a rate. Contract-baseline rates are the last-resort
        fallback used when a connector has no learned or overridden fee.
      </p>

      <ErrorMessage
        error={
          seedSaveError ??
          (error instanceof Error ? error.message : error ? 'Failed to load connectors' : null) ??
          (seedError instanceof Error
            ? seedError.message
            : seedError
              ? 'Failed to load contract rates'
              : null)
        }
      />

      {feeModal && merchantId && (
        <FeeModal
          merchantId={merchantId}
          state={feeModal}
          onClose={() => setFeeModal(null)}
          onSaved={async () => {
            setFeeModal(null)
            await refresh()
          }}
        />
      )}

      {seedEditor && (
        <ScenarioModal
          initial={seedEditor.row}
          isNew={seedEditor.index === null}
          busy={seedBusy}
          saveError={seedSaveError}
          onSave={saveScenario}
          onRemove={removeScenario}
          onClose={() => {
            setSeedEditor(null)
            setSeedSaveError(null)
          }}
        />
      )}
    </div>
  )
}

/**
 * The decide-time cost ladder as an inline strip, so the precedence is visible without docs. The
 * chips are the very badges the tiles carry, so a tile's badge can be read straight off this strip.
 *
 * Written as an equation — "Active fee = A → B → C" — because the active fee is the *result* of the
 * ladder, not a rung of it: as a peer chip at the head of the same chevron chain it reads as the
 * first candidate, which inverts "the first one set wins".
 */
function PrecedenceLegend() {
  const ladder: FeeSource[] = ['override', 'reports', 'contract']
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-xl border border-slate-200 bg-slate-50/60 px-4 py-2.5 dark:border-[#232833] dark:bg-[#0b0e14]">
      <span className="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8d96aa]">
        Active fee
      </span>
      <span className="text-sm font-semibold text-slate-400 dark:text-[#5c6577]">=</span>
      {ladder.map((s, i) => (
        <Fragment key={s}>
          {i > 0 && (
            <ArrowRight
              size={13}
              aria-hidden="true"
              className="text-slate-300 dark:text-[#3a3a44]"
            />
          )}
          <Pill tone={SOURCE_TONES[s]}>{SOURCE_LABELS[s]}</Pill>
        </Fragment>
      ))}
      <span className="text-xs text-slate-500 dark:text-[#8d96aa]">— the first one set wins.</span>
    </div>
  )
}

/**
 * One connector's row: what it charges at decide time and which rung of the ladder set that number.
 * Clicking the row points the detail panel at this connector; the pencil sets a connector-wide
 * manual override. The volume backing the fee lives in the detail panel header, where there is room
 * to label it — a row this dense would only fit bare numbers.
 */
function ConnectorRow({
  connector,
  fee,
  clusters,
  seedRows,
  isSelected,
  onSelect,
  onEditFee,
}: {
  connector: string
  fee?: ConnectorFee
  clusters: ClusterFee[]
  seedRows: IndexedSeedRow[]
  isSelected: boolean
  onSelect: () => void
  onEditFee: () => void
}) {
  const baseline = baselineSeedRow(seedRows)
  const source = feeSource(fee, seedRows.length > 0)

  // The active fee: the API's effective (override/model) when present, else the contract baseline's
  // total rate — so a contract-only connector still shows a real number.
  const activePct = fee?.effective_pct_bps ?? (baseline ? totalRate(baseline) : null)
  const activeFixed = fee?.effective_fixed ?? baseline?.fixed ?? null

  // Which currency the fixed part is quoted in: a contract row states it outright; a learned fee is
  // fitted from settlement rows, which all price in the account's settlement currency.
  const currency = baseline?.transaction_currency || clusters[0]?.currency || null

  return (
    // A div rather than a button: the pencil inside it is itself a button, and nesting one button in
    // another is invalid HTML (the inner click would also fire the outer handler).
    <div
      role="button"
      tabIndex={0}
      aria-pressed={isSelected}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect()
        }
      }}
      className={`relative flex cursor-pointer items-center gap-3 px-5 py-3.5 transition-colors ${
        isSelected
          ? 'bg-brand-500/[0.06] dark:bg-brand-500/[0.09]'
          : 'hover:bg-slate-50 dark:hover:bg-[#0f131b]'
      }`}
    >
      {isSelected && <span className="absolute inset-y-0 left-0 w-1 bg-brand-500" aria-hidden />}

      <p className="w-28 shrink-0 truncate text-sm font-semibold text-slate-900 dark:text-white">
        {titleCase(connector)}
      </p>
      <Pill tone={SOURCE_TONES[source]}>{SOURCE_LABELS[source]}</Pill>
      {/* Only when there is one: a column of "No account" on every row is five rows of nothing. */}
      {fee?.account && (
        <p className="hidden min-w-0 truncate text-xs text-slate-500 dark:text-[#8d96aa] sm:block">
          Account {fee.account}
        </p>
      )}

      <span className="ml-auto shrink-0 tabular-nums text-sm font-semibold text-slate-900 dark:text-white">
        {formatFee(activePct, activeFixed, { currency, perTxn: true })}
      </span>

      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          onEditFee()
        }}
        title="Set a manual override for this connector"
        aria-label={`Set a manual override for ${titleCase(connector)}`}
        className="shrink-0 rounded-md border border-transparent p-1 text-slate-400 transition-colors hover:border-brand-500/40 hover:bg-brand-500/10 hover:text-brand-600 dark:text-[#78849a] dark:hover:text-brand-400"
      >
        <Pencil size={14} />
      </button>
      <ChevronRight
        size={16}
        aria-hidden="true"
        className={`shrink-0 ${isSelected ? 'text-brand-500' : 'text-slate-300 dark:text-[#3a3a44]'}`}
      />
    </div>
  )
}

type SubView = 'reports' | 'contract'

/**
 * The detail panel for the selected connector: its learned segments or its contract scenarios, one
 * layer at a time, paged.
 */
function ConnectorDetail({
  merchantId,
  connector,
  fee,
  clusters,
  seedRows,
  filters,
  facets,
  onFiltersChange,
  editingKey,
  setEditingKey,
  onRefresh,
  onAddScenario,
  onEditScenario,
}: {
  merchantId: string
  connector: string
  fee?: ConnectorFee
  clusters: ClusterFee[]
  seedRows: IndexedSeedRow[]
  filters: ClusterFilters
  facets: Record<string, ClusterFacet[]>
  onFiltersChange: (next: ClusterFilters) => void
  editingKey: string | null
  setEditingKey: (key: string | null) => void
  onRefresh: () => Promise<void>
  onAddScenario: () => void
  onEditScenario: (index: number) => void
}) {
  // Which layer is shown. Until the user picks a tab (`null`), derive it from the live data —
  // "From reports" when this connector has at least one learned segment, else "Contract baseline",
  // so a connector with nothing learned opens on the layer that actually has rates in it.
  // Deriving (rather than seeding useState once) matters because clusters load async: the panel can
  // render before its segments arrive, and a one-shot initial default would then strand a
  // report-backed connector on the empty Contract-baseline tab.
  const [pickedView, setPickedView] = useState<SubView | null>(null)
  const subView: SubView = pickedView ?? (clusters.length > 0 ? 'reports' : 'contract')

  const [page, setPage] = useState(0)
  // Both bits of view state belong to the selected connector, and the panel is reused rather than
  // remounted when the selection changes — so clear them by hand. Dropping the pick is what lets the
  // derivation above re-run per connector: without it, having chosen "From reports" on one connector
  // would land you on an empty Reports tab for the next one that has no learned segments.
  useEffect(() => {
    setPickedView(null)
    setPage(0)
  }, [connector])

  // Paging is per layer too, but switching layers must not clear an explicit tab pick.
  useEffect(() => {
    setPage(0)
  }, [subView])

  const source = feeSource(fee, seedRows.length > 0)
  const total = subView === 'reports' ? clusters.length : seedRows.length
  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const safePage = Math.min(page, pageCount - 1)
  const start = safePage * PAGE_SIZE
  const pageClusters = clusters.slice(start, start + PAGE_SIZE)
  const pageSeedRows = seedRows.slice(start, start + PAGE_SIZE)

  const volume = clusters.reduce((sum, c) => sum + c.gross_sum, 0)
  const txns = clusters.reduce((sum, c) => sum + c.n, 0)

  const shown = subView === 'reports' ? pageClusters.length : pageSeedRows.length
  const noun = subView === 'reports' ? 'active learned fees' : 'contract scenarios'
  const showing =
    total === 0
      ? `No ${noun}`
      : pageCount > 1
        ? `Showing ${start + 1}–${start + shown} of ${total} ${noun}`
        : `Showing ${total} ${noun}`

  return (
    <Card className="!rounded-2xl overflow-hidden">
      {/* Panel header */}
      <div className="flex flex-wrap items-center justify-between gap-3 px-5 py-4">
        <div className="flex items-center gap-2.5">
          <span className="h-4 w-1 rounded-full bg-brand-500" aria-hidden />
          <h3 className="text-base font-semibold text-slate-900 dark:text-white">
            {titleCase(connector)} Detail
          </h3>
          <Pill tone={source === 'none' ? 'slate' : 'green'}>
            {source === 'none' ? 'NO FEE' : 'ACTIVE'}
          </Pill>
          {/* What backs a learned fee — a rate fitted from 31 transactions is not the same claim as
              one fitted from 41k, and the number is meaningless without that. */}
          {clusters.length > 0 && (
            <span className="text-xs text-slate-500 dark:text-[#8d96aa]">
              {formatCompact(volume)} settled · {txns.toLocaleString()} txns
            </span>
          )}
        </div>

        <div className="flex items-center gap-1 rounded-lg border border-slate-200 bg-slate-50 p-1 dark:border-[#232833] dark:bg-[#0b0e14]">
          <LayerTab active={subView === 'reports'} onClick={() => setPickedView('reports')}>
            From reports ({clusters.length})
          </LayerTab>
          <LayerTab active={subView === 'contract'} onClick={() => setPickedView('contract')}>
            Contract baseline ({seedRows.length})
          </LayerTab>
        </div>
      </div>

      {subView === 'reports' ? (
        <>
          <div className="border-t border-slate-200 px-5 py-3 dark:border-[#232833]">
            <ClusterFilterBar filters={filters} facets={facets} onChange={onFiltersChange} />
          </div>
          {clusters.length > 0 ? (
          <div className="overflow-x-auto border-t border-slate-200 dark:border-[#232833]">
            <table className="w-full min-w-[920px] text-left text-sm">
              {/* Network is a COLUMN, not a band header splitting the rows into per-network
                  sub-tables. One flat, uniformly-sortable/filterable body means a Network filter
                  narrows rows the same way every other column does, and a page of results reads as
                  one list instead of a run of one-row sections. */}
              <thead>
                <tr className="border-b border-slate-200 bg-slate-50 text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:border-[#232833] dark:bg-[#0e131b] dark:text-[#8d96aa] leading-4">
                  <th className="w-16 py-2 pl-5 pr-3" />
                  <th className="w-28 py-2 pr-3 text-left font-semibold">Network</th>
                  <th className="w-24 py-2 pr-3 text-left font-semibold">Type</th>
                  <th className="w-16 py-2 pr-3 text-left font-semibold">Country</th>
                  <th className="w-20 py-2 pr-3 text-left font-semibold">Currency</th>
                  <th className="py-2 pr-3 text-left font-semibold">Category</th>
                  <th className="w-20 py-2 pr-3 text-right font-semibold">Txns</th>
                  <th className="py-2 pr-3 text-right font-semibold">Fee</th>
                  <th className="py-2 pr-5" />
                </tr>
              </thead>
              <tbody>
                  {pageClusters.map((c) => (
                    <SegmentRow
                      key={c.key}
                      merchantId={merchantId}
                      c={c}
                      isEditing={editingKey === c.key}
                      editingActive={editingKey !== null}
                      onEdit={() => setEditingKey(c.key)}
                      onCancel={() => setEditingKey(null)}
                      onSaved={async () => {
                        setEditingKey(null)
                        await onRefresh()
                      }}
                    />
                  ))}
              </tbody>
            </table>
          </div>
          ) : (
            <div className="border-t border-slate-200 px-5 py-8 text-sm text-slate-500 dark:border-[#232833]">
              {countActiveFilters(filters) > 0
                ? 'No segments match these filters — clear one to widen the search.'
                : `No settlement data yet for ${titleCase(connector)}. Ingest a report below to learn its fees, or set a contract baseline.`}
            </div>
          )}
        </>
      ) : (
        <ContractBaselineTable
          connector={connector}
          rows={pageSeedRows}
          onAdd={onAddScenario}
          onEdit={onEditScenario}
        />
      )}

      {/* Panel footer: what's on screen, and paging when there's more of it. */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-t border-slate-200 px-5 py-3 dark:border-[#232833]">
        <p className="text-[13px] text-slate-500 dark:text-[#8d96aa] leading-[18px]">{showing}</p>
        {pageCount > 1 && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-slate-500 dark:text-[#8d96aa]">
              Page {safePage + 1} of {pageCount}
            </span>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setPage(safePage - 1)}
              disabled={safePage === 0}
            >
              Previous
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setPage(safePage + 1)}
              disabled={safePage >= pageCount - 1}
            >
              Next
            </Button>
          </div>
        )}
      </div>
    </Card>
  )
}

/** A segmented-control tab used to switch the detail panel between its two layers. */
function LayerTab({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? 'true' : undefined}
      className={`rounded-md px-3 py-1.5 text-[13px] font-medium transition-colors ${
        active
          ? 'bg-white text-brand-600 shadow-sm dark:bg-[#161c26] dark:text-brand-400'
          : 'text-slate-500 hover:text-slate-700 dark:text-[#8d96aa] dark:hover:text-white'
      } leading-[18px]`}
    >
      {children}
    </button>
  )
}

/** The contract-baseline layer: that PSP's seed scenarios, editable per row. */
function ContractBaselineTable({
  connector,
  rows,
  onAdd,
  onEdit,
}: {
  connector: string
  rows: IndexedSeedRow[]
  onAdd: () => void
  onEdit: (index: number) => void
}) {
  return (
    <div className="border-t border-slate-200 dark:border-[#232833]">
      <div className="flex flex-wrap items-center justify-between gap-4 px-5 py-2.5">
        <p className="text-[12px] text-slate-500 dark:text-[#8d96aa] max-w-[57ch] leading-4">
          Your contract rate per card scenario — the last-resort fallback when {titleCase(connector)}{' '}
          has no learned or overridden fee. A blank dimension matches any card.
        </p>
        <Button variant="secondary" size="sm" onClick={onAdd}>
          <Plus size={13} /> Add scenario
        </Button>
      </div>
      {rows.length > 0 ? (
        <div className="overflow-x-auto border-t border-slate-200 dark:border-[#232833]">
          <table className="w-full min-w-[820px] text-left text-sm">
            <thead>
              <tr className="border-b border-slate-200 bg-slate-50 text-[12px] font-semibold text-slate-500 dark:border-[#232833] dark:bg-[#0b0e14] dark:text-[#8d96aa] leading-4">
                <th className="px-5 py-3">Scenario</th>
                <th className="py-3 pr-3">Network</th>
                <th className="py-3 pr-3">Funding</th>
                <th className="py-3 pr-3">Program</th>
                <th className="py-3 pr-3">Currency</th>
                <th className="py-3 pr-3">Region</th>
                <th className="py-3 pr-3 text-right">Interchange</th>
                <th className="py-3 pr-3 text-right">Scheme</th>
                <th className="py-3 pr-3 text-right">Markup</th>
                <th className="py-3 pr-3 text-right">Fixed</th>
                <th className="bg-brand-500/[0.06] py-3 pr-3 text-right dark:bg-brand-500/[0.1]">Total rate</th>
                <th className="bg-brand-500/[0.06] py-3 pr-5 dark:bg-brand-500/[0.1]" />
              </tr>
            </thead>
            <tbody>
              {rows.map(({ row, index }) => (
                <SeedRow key={index} row={row} busy={false} disabled={false} onEdit={() => onEdit(index)} />
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="border-t border-slate-200 px-5 py-8 text-sm text-slate-500 dark:border-[#232833]">
          No contract baseline for {titleCase(connector)} yet. Add a scenario to price it when no
          learned data is available.
        </div>
      )}
    </div>
  )
}

/**
 * One learned segment. Reads as a single line — card scenario on the left, the fee it resolves to on
 * the right — with three actions: edit the fee, inspect the evidence behind it, and (for a
 * capped/tiered cluster) open its recovered per-segment rates.
 */
function SegmentRow({
  merchantId,
  c,
  isEditing,
  editingActive,
  onEdit,
  onCancel,
  onSaved,
}: {
  merchantId: string
  c: ClusterFee
  isEditing: boolean
  editingActive: boolean
  onEdit: () => void
  onCancel: () => void
  onSaved: () => Promise<void>
}) {
  const isOverride = c.source === 'override'
  const hasSegments = (c.segments?.length ?? 0) > 0
  const seedPct = () => String(bpsToPct(c.effective_pct_bps ?? c.override_pct_bps ?? c.model_pct_bps ?? 0))
  const seedFixed = () => String(c.effective_fixed ?? c.override_fixed ?? c.model_fixed ?? 0)
  const [pct, setPct] = useState(seedPct)
  const [fixed, setFixed] = useState(seedFixed)
  const [busy, setBusy] = useState<'save' | 'clear' | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [showTiers, setShowTiers] = useState(false)
  const [showDetail, setShowDetail] = useState(false)

  // Re-seed inputs whenever the row (re)enters edit mode so a reopened editor never shows stale
  // typing; keyed only on the open/close transition so a background refresh can't clobber input.
  useEffect(() => {
    if (!isEditing) return
    setPct(seedPct())
    setFixed(seedFixed())
    setError(null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isEditing])

  async function save() {
    const p = parseFloat(pct)
    const f = parseFloat(fixed)
    if (!isFinite(p) || p < 0 || !isFinite(f) || f < 0) {
      setError('Enter non-negative numbers for the rate and fixed fee.')
      return
    }
    setBusy('save')
    setError(null)
    try {
      await setClusterOverride(merchantId, c.key, { pct_bps: pctToBps(p), fixed: f })
      await onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save fee')
    } finally {
      setBusy(null)
    }
  }

  async function clear() {
    setBusy('clear')
    setError(null)
    try {
      await deleteClusterOverride(merchantId, c.key)
      await onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to clear override')
    } finally {
      setBusy(null)
    }
  }

  return (
    <>
      <tr className="border-b border-slate-100 last:border-0 hover:bg-slate-50/70 dark:border-[#1c1c23] dark:hover:bg-[#0f131b]">
        <td className="w-16 py-3.5 pl-5 pr-3">
          <span className="inline-flex h-8 w-8 items-center justify-center rounded-lg bg-slate-100 text-slate-500 dark:bg-white/[0.06] dark:text-[#8d96aa]">
            <CreditCard size={15} />
          </span>
        </td>
        <td className="w-28 py-3.5 pr-3 font-medium text-slate-700 dark:text-[#c7cfdd]">
          {methodLabel(c)}
        </td>
        <td className="w-24 py-3.5 pr-3 font-semibold capitalize text-slate-800 dark:text-white">
          {c.funding || '—'}
        </td>
        <td className="w-16 py-3.5 pr-3 tabular-nums font-medium text-slate-700 dark:text-[#c7cfdd]">
          {c.issuer_country?.toUpperCase() || '—'}
        </td>
        <td className="w-20 py-3.5 pr-3 tabular-nums font-medium text-slate-700 dark:text-[#c7cfdd]">
          {c.currency?.toUpperCase() || '—'}
        </td>
        <td className="py-3.5 pr-3 text-slate-500 dark:text-[#9ca7ba]">{c.ic_category || '—'}</td>
        <td className="w-20 py-3.5 pr-3 text-right tabular-nums font-medium text-slate-600 dark:text-[#9ca7ba]">
          {c.n.toLocaleString()}
        </td>
        <td className="py-3.5 pr-3 text-right">
          {isEditing ? (
            <div className="flex items-center justify-end gap-1">
              <input
                className={`${feeInput} w-20`}
                type="number"
                step="0.01"
                min="0"
                value={pct}
                onChange={(e) => setPct(e.target.value)}
                title="Rate (%)"
                aria-label="Rate (%)"
              />
              <input
                className={`${feeInput} w-16`}
                type="number"
                step="0.01"
                min="0"
                value={fixed}
                onChange={(e) => setFixed(e.target.value)}
                title="Fixed per txn"
                aria-label="Fixed per txn"
              />
            </div>
          ) : (
            <div className="flex items-center justify-end gap-2">
              {hasSegments && (
                <span className="rounded bg-amber-100 px-1.5 py-0.5 text-[11px] font-semibold uppercase tracking-wide text-amber-700 dark:bg-amber-500/15 dark:text-amber-400 leading-4">
                  {c.segments!.length} tiers
                </span>
              )}
              {isOverride && <Pill tone="indigo">OVERRIDE</Pill>}
              <div>
                <span
                  className={`block tabular-nums font-semibold ${
                    isOverride ? 'text-brand-600 dark:text-brand-400' : 'text-slate-800 dark:text-[#c7cfdd]'
                  }`}
                >
                  {formatFee(c.effective_pct_bps, c.effective_fixed, { currency: c.currency })}
                </span>
                {isOverride && c.model_pct_bps != null && (
                  <span className="block text-[11px] tabular-nums text-slate-500 line-through leading-4">
                    {formatFee(c.model_pct_bps, c.model_fixed, { currency: c.currency })}
                  </span>
                )}
              </div>
            </div>
          )}
        </td>
        <td className="w-[120px] py-3.5 pr-5 text-right">
          {isEditing ? (
            <div className="flex items-center justify-end gap-1 whitespace-nowrap">
              <button
                type="button"
                onClick={save}
                disabled={busy !== null}
                title="Save fee"
                aria-label="Save fee"
                className="inline-flex h-7 w-7 items-center justify-center rounded-md bg-brand-600 text-white transition-colors hover:bg-brand-700 disabled:opacity-40 dark:bg-white dark:text-black dark:hover:bg-slate-200"
              >
                {busy === 'save' ? <Spinner size={13} /> : <Check size={14} />}
              </button>
              {isOverride && (
                <button
                  type="button"
                  onClick={clear}
                  disabled={busy !== null}
                  title="Remove override"
                  aria-label="Remove override"
                  className="inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-rose-50 hover:text-rose-600 disabled:opacity-40 dark:hover:bg-rose-500/10"
                >
                  {busy === 'clear' ? <Spinner size={13} /> : <X size={14} />}
                </button>
              )}
              {!isOverride && (
                <button
                  type="button"
                  onClick={onCancel}
                  disabled={busy !== null}
                  title="Cancel"
                  aria-label="Cancel"
                  className="inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-800 disabled:opacity-40 dark:text-[#a1a1aa] dark:hover:bg-[#121214] dark:hover:text-white"
                >
                  <X size={14} />
                </button>
              )}
            </div>
          ) : (
            <div className="flex items-center justify-end gap-0.5">
              <RowAction
                title="Edit segment fee"
                disabled={editingActive}
                onClick={onEdit}
                icon={<Pencil size={15} />}
              />
              <RowAction
                title="Show the evidence behind this fee"
                expanded={showDetail}
                onClick={() => setShowDetail((v) => !v)}
                icon={<Eye size={15} />}
              />
              {hasSegments && (
                <RowAction
                  title="Show per-segment rates"
                  expanded={showTiers}
                  onClick={() => setShowTiers((v) => !v)}
                  icon={
                    showTiers ? (
                      <ChevronDown size={16} className="text-brand-600 dark:text-brand-400" />
                    ) : (
                      <ChevronRight size={16} className="text-brand-600 dark:text-brand-400" />
                    )
                  }
                />
              )}
            </div>
          )}
        </td>
      </tr>

      {showDetail && (
        <tr className="bg-slate-50/70 dark:bg-[#0d1017]">
          <td colSpan={9} className="px-5 pb-4 pt-2">
            <div className="flex flex-wrap gap-x-10 gap-y-3 rounded-lg border border-slate-200 bg-white px-4 py-3 dark:border-[#232833] dark:bg-[#0b0e14]">
              <Fact label="Learned" value={formatFee(c.model_pct_bps, c.model_fixed, { currency: c.currency })} />
              <Fact
                label="Override"
                value={
                  c.override_pct_bps != null
                    ? formatFee(c.override_pct_bps, c.override_fixed, { currency: c.currency })
                    : '—'
                }
              />
              <Fact label="Txns" value={c.n.toLocaleString()} />
              <Fact label="Volume" value={formatCompact(c.gross_sum)} />
              <Fact label="Program" value={programRate(c.card_product)} />
              <div className="min-w-0">
                <p className="text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8d96aa] leading-4">
                  Fit
                </p>
                <div className="mt-1">
                  {c.verdict ? (
                    <Badge variant={verdictVariant(c.verdict)}>{c.verdict}</Badge>
                  ) : (
                    <span className="text-[13px] text-slate-500 leading-[18px]">no fitted rate</span>
                  )}
                </div>
              </div>
            </div>
          </td>
        </tr>
      )}

      {showTiers && hasSegments && (
        <tr className="bg-slate-50/70 dark:bg-[#0d1017]">
          <td colSpan={9} className="px-5 pb-4 pt-2">
            <p className="mb-0.5 text-sm font-semibold text-slate-700 dark:text-[#c7cfdd]">
              Per-segment rates
            </p>
            <p className="mb-2 text-[12px] text-slate-500 dark:text-[#8d96aa] max-w-[57ch] leading-4">
              Optimized tiers dynamically calculated based on historical card activity and fit-error
              thresholds.
            </p>
            <div className="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-[#232833] dark:bg-[#0b0e14]">
              <table className="w-full min-w-[560px] text-[12px] leading-4">
                <thead>
                  <tr className="border-b border-slate-200 text-left uppercase tracking-wide text-slate-500 dark:border-[#232833] dark:text-[#78849a]">
                    <th className="px-3 py-2 font-medium">Amount range</th>
                    <th className="py-2 pr-3 text-right font-medium">Rate</th>
                    <th className="py-2 pr-3 text-right font-medium">Fixed</th>
                    <th className="py-2 pr-3 text-right font-medium">Txns</th>
                    <th className="py-2 pr-3 text-right font-medium">Volume</th>
                    <th className="py-2 pr-3 text-right font-medium">Fit err (bps)</th>
                    <th className="py-2 pr-3 text-right font-medium">Quality</th>
                  </tr>
                </thead>
                <tbody>
                  {c.segments!.map((s) => (
                    <tr
                      key={s.seg_idx}
                      className="border-b border-slate-100 tabular-nums text-slate-600 last:border-0 dark:border-[#1c1c23] dark:text-[#c7cfdd]"
                    >
                      <td className="px-3 py-2 text-left">
                        {formatCompact(s.lo)} - {formatCompact(s.hi)}
                      </td>
                      <td className="py-2 pr-3 text-right font-medium">
                        {s.pct_bps != null ? pctText(s.pct_bps) : '—'}
                      </td>
                      <td className="py-2 pr-3 text-right">
                        {s.fixed != null ? `${ccySymbol(c.currency)}${s.fixed.toFixed(2)}` : '—'}
                      </td>
                      <td className="py-2 pr-3 text-right">{s.n.toLocaleString()}</td>
                      <td className="py-2 pr-3 text-right">{formatCompact(s.gross_sum)}</td>
                      <td className="py-2 pr-3 text-right">
                        {s.bps_rmse != null ? s.bps_rmse.toFixed(1) : '—'}
                      </td>
                      <td className="py-2 pr-3 text-right">
                        <Badge variant={verdictVariant(s.verdict)}>{s.verdict}</Badge>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </td>
        </tr>
      )}

      {isEditing && error && (
        <tr>
          <td colSpan={9} className="px-5 pb-2">
            <ErrorMessage error={error} />
          </td>
        </tr>
      )}
    </>
  )
}

/** An icon button in a segment row's action cluster. */
function RowAction({
  title,
  icon,
  onClick,
  disabled = false,
  expanded,
}: {
  title: string
  icon: ReactNode
  onClick: () => void
  disabled?: boolean
  expanded?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={title}
      aria-expanded={expanded}
      className="inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-700 disabled:opacity-40 dark:hover:bg-[#121214] dark:hover:text-white"
    >
      {icon}
    </button>
  )
}

/** A label-over-value pair inside a segment's evidence strip. */
function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <p className="text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8d96aa] leading-4">
        {label}
      </p>
      <p className="mt-1 truncate text-[13px] tabular-nums font-medium text-slate-700 dark:text-[#c7cfdd] leading-[18px]">
        {value}
      </p>
    </div>
  )
}

// Compact numeric input for the inline segment-fee editor.
const feeInput =
  'rounded-lg border border-slate-200 bg-white px-2 py-1 text-sm text-right text-slate-900 ' +
  'focus:border-brand-500 focus:outline-none dark:border-[#232833] dark:bg-[#0b1017] dark:text-white'

interface FeeModalState {
  connector: string
  /** Provided only when adding — the selectable connectors. */
  connectors?: string[]
  isNew: boolean
  initialPctBps: number
  initialFixed: number
  canClear: boolean
}

/** Set / clear a connector-wide blended fee override (also the "Add connector" flow). */
function FeeModal({
  merchantId,
  state,
  onClose,
  onSaved,
}: {
  merchantId: string
  state: FeeModalState
  onClose: () => void
  onSaved: () => Promise<void>
}) {
  const [connector, setConnector] = useState(state.connector)
  const [pctBps, setPctBps] = useState(state.initialPctBps)
  const [fixed, setFixed] = useState(String(state.initialFixed))
  const [busy, setBusy] = useState<'save' | 'clear' | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape' && !busy) onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [busy, onClose])

  async function save() {
    const f = parseFloat(fixed)
    if (pctBps < 0 || !isFinite(f) || f < 0) {
      setError('Enter non-negative numbers for the rate and fixed fee.')
      return
    }
    setBusy('save')
    setError(null)
    try {
      await setFeeOverride(merchantId, connector, { pct_bps: pctBps, fixed: f })
      await onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save fee')
    } finally {
      setBusy(null)
    }
  }

  async function clear() {
    setBusy('clear')
    setError(null)
    try {
      await deleteFeeOverride(merchantId, connector)
      await onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to clear override')
    } finally {
      setBusy(null)
    }
  }

  const fieldLabel = 'block text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]'
  const numField =
    'mt-1 w-full rounded-md border border-slate-200 bg-white px-2 py-1.5 text-sm text-slate-900 ' +
    'focus:border-brand-500 focus:outline-none dark:border-[#232833] dark:bg-[#0b1017] dark:text-white'

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/40 backdrop-blur-[2px]" onClick={() => !busy && onClose()} />
      <div className="relative w-full max-w-md rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl dark:border-[#2a303a] dark:bg-[#0d1118]">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-slate-900 dark:text-white">
            {state.isNew ? 'Add connector fee' : `${titleCase(connector)} manual override`}
          </h3>
          <button
            type="button"
            onClick={onClose}
            disabled={busy !== null}
            aria-label="Close"
            className="text-slate-500 transition-colors hover:text-slate-700 disabled:opacity-40 dark:hover:text-white"
          >
            <X size={16} />
          </button>
        </div>

        <div className="mt-4 space-y-4">
          {state.isNew && state.connectors && (
            <label className="block">
              <span className={fieldLabel}>Connector</span>
              <select
                className={`${inputClass} mt-1`}
                value={connector}
                onChange={(e) => setConnector(e.target.value)}
              >
                {state.connectors.map((c) => (
                  <option key={c} value={c}>
                    {titleCase(c)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className={fieldLabel}>Rate</span>
              <div className="mt-1">
                <PctInput bps={pctBps} onChange={setPctBps} />
              </div>
            </label>
            <label className="block">
              <span className={fieldLabel}>Fixed per txn</span>
              <input
                className={numField}
                type="number"
                step="0.01"
                min="0"
                value={fixed}
                onChange={(e) => setFixed(e.target.value)}
              />
            </label>
          </div>
          <p className="text-xs text-slate-500 max-w-[57ch]">
            Applied to every economic-value calculation for {titleCase(connector)} from now on,
            overriding the learned model and contract baseline for this connector.
          </p>
          <ErrorMessage error={error} />
        </div>

        <div className="mt-6 flex items-center justify-between">
          {state.canClear ? (
            <Button variant="danger" size="sm" onClick={clear} disabled={busy !== null}>
              {busy === 'clear' ? <Spinner size={13} /> : null} Remove override
            </Button>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button variant="secondary" size="sm" onClick={onClose} disabled={busy !== null}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" onClick={save} disabled={busy !== null}>
              {busy === 'save' ? <Spinner size={13} /> : null} Save fee
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
