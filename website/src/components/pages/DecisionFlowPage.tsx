import type { ElementType, ReactNode } from 'react'
import { useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import useSWR from 'swr'
import {
  Activity,
  BookOpen,
  ChevronRight,
  Coins,
  Filter,
  FlaskConical,
  Network,
  RefreshCcw,
  Flag,
  Scale,
  SlidersHorizontal,
  Star,
  TrendingUp,
  Zap,
} from 'lucide-react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import * as type from '../ui/typography'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiErrorStatus, apiPost } from '../../lib/api'
import { EliminationData, RoutingAlgorithm, RuleConfig, SRConfigData } from '../../types/api'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'
import { useMerchantFeatures } from '../../hooks/useMerchantFeatures'
import { describeRuleConditions } from '../../features/routing/euclid/describe'
import { gatewayLabel, normalizeRuleOutput } from '../../features/routing/euclid/summarize'
import {
  StackState,
  abTestData,
  deriveLanes,
  deriveStack,
  euclidData,
  exampleEvaluateParameters,
  slotAlgorithmSummary,
  volumeSplits,
} from '../../features/routing/decisionFlow/model'
import { LaneCanvas } from '../../features/routing/decisionFlow/LaneCanvas'

type StageId =
  | 'arrive'
  | 'ab'
  | 'debit'
  | 'slot'
  | 'eligibility'
  | 'preferred'
  | 'priority'
  | 'sr'
  | 'health'
  | 'cost'
  | 'commitment'
  | 'decide'
  | 'learn'

type BadgeVariant = 'green' | 'gray' | 'blue' | 'purple' | 'orange'

interface StageView {
  badge: string
  variant: BadgeVariant
  /**
   * Not configured for this merchant: the stage is omitted from the rail entirely. The stack
   * board above stays the discovery surface — it lists every layer with a Configure link, and
   * the stage joins the flow as soon as its configuration lands.
   */
  dim: boolean
  /** A short config fact shown next to the badge when the stage is live, e.g. "3 rules". */
  detail?: string
  /** Tiny top-right tag naming the operation kind: "hard filter", "re-rank", "branch"… */
  kind?: string
  kindTone?: 'filter' | 'rank' | 'override' | 'neutral'
}

/** Both halves of the hybrid integration are live: static routing picks, then SR ranks. */
function isHybrid(stack: StackState) {
  return (stack.slot === 'rule' || stack.slot === 'volume') && stack.srConfigured
}

interface StageDef {
  id: StageId
  group: string
  icon: ElementType
  name: string
  /** Computed from the live configuration, so the prose matches what actually happens here. */
  what: (stack: StackState) => string
  runsWhen: string
  configuredBy: string
  configureTo?: string
  api: string
  view: (stack: StackState, connectorCount: number) => StageView
}

/**
 * The execution order below mirrors the decide-gateway pipeline: A/B interception first, the
 * debit branch, then candidate selection, the scoring ladder, and the post-decision overrides.
 */
const STAGES: StageDef[] = [
  {
    id: 'arrive',
    group: 'Entry',
    icon: Zap,
    name: 'Payment arrives',
    what: (stack) =>
      'Your integration asks for a gateway, offering the connectors it considers eligible. Missing card attributes (network, type, issuer country) are filled in from the BIN before anything else runs.' +
      (stack.slot === 'none'
        ? ' With no strategy active, that list continues through the pipeline as-is — activate one to shape it.'
        : ''),
    runsWhen: 'Every decision starts here.',
    configuredBy: 'Nothing to configure — but the fields your integration sends decide which later stages can run.',
    api: 'POST /decide-gateway',
    view: (_stack, connectorCount) => ({
      badge: 'Always runs',
      variant: 'blue',
      dim: false,
      kind: 'input',
      detail:
        connectorCount > 0 ? `${connectorCount} connector${connectorCount === 1 ? '' : 's'} in play` : undefined,
    }),
  },
  {
    id: 'ab',
    group: 'Experiment layer',
    icon: FlaskConical,
    name: 'A/B experiment gate',
    what: (stack) => {
      const experiment = abTestData(stack)
      const split = experiment ? `${experiment.variant_split_pct}% of payments land in the variant, the rest in control — ` : ''
      return `${split}payments are hashed into their arm by payment ID. An arm pointing at a fixed strategy answers immediately — nothing below runs for that payment. An arm pointing at live routing continues with its own overrides (hedging, elimination threshold).`
    },
    runsWhen: 'While an experiment occupies the strategy slot. /routing/evaluate always follows its arms; /decide-gateway interception additionally needs the real-payments flag.',
    configuredBy: 'The A/B Testing page. An active experiment occupies the same slot as rules and volume splits.',
    configureTo: '/routing/ab-testing',
    api: 'POST /routing/list/active/{merchant_id}',
    view: (stack) => {
      if (stack.slot !== 'ab') return { badge: 'No experiment', variant: 'gray', dim: true }
      if (!stack.abRealPaymentsOn)
        return { badge: 'Live · interceptor flag off', variant: 'orange', dim: false, kind: 'traffic split' }
      const split = abTestData(stack)?.variant_split_pct
      return {
        badge: 'Experiment live',
        variant: 'purple',
        dim: false,
        kind: 'traffic split',
        detail: split != null ? `${split}% variant` : undefined,
      }
    },
  },
  {
    id: 'debit',
    group: 'Network branch',
    icon: Network,
    name: 'Debit network routing',
    what: () =>
      'For co-badged debit cards, eligible networks are ranked by processing cost — answering by itself in network-only mode, or riding along with the gateway decision in hybrid mode.',
    runsWhen: 'Only when your integration asks for it on the request (rankingAlgorithm = NTW_BASED_ROUTING or NTW_SR_HYBRID_ROUTING) — and the account flag is on.',
    configuredBy: 'The Debit Routing page (merchant category code, acquirer country, enable flag).',
    configureTo: '/routing/debit',
    api: 'GET /merchant-account/{merchant_id}/debit-routing',
    view: (stack) =>
      stack.debitOn
        ? { badge: 'Enabled · per request', variant: 'blue', dim: false, kind: 'branch' }
        : { badge: 'Not enabled', variant: 'gray', dim: true },
  },
  {
    id: 'slot',
    group: 'Candidates — who can process it',
    icon: BookOpen,
    name: 'Your routing strategy',
    what: (stack) => {
      const name = slotAlgorithmSummary(stack)?.name
      if (stack.slot === 'rule')
        return `Rules in “${name}” evaluate top-to-bottom — the first match wins and emits its connectors; when nothing matches, the default selection answers. Only one strategy can hold this slot at a time.`
      if (stack.slot === 'volume')
        return `“${name}” splits traffic by weighted draw — lane thickness above mirrors each connector’s share. Only one strategy can hold this slot at a time.`
      if (stack.slot === 'ab')
        return `The experiment “${name}” holds the slot, so rules and volume splits are paused while it runs — they all share this one activation slot.`
      return 'One strategy holds this slot at a time: a rule set (first match wins), a volume split (weighted draw), or a running experiment. Its output is the ordered candidate list.'
    },
    runsWhen: 'Whenever a strategy is activated. Activating one replaces whatever held the slot.',
    configuredBy: 'The Rule-Based or Volume Split pages.',
    configureTo: '/routing/rules',
    api: 'POST /routing/list/active/{merchant_id}',
    view: (stack) => {
      const summary = slotAlgorithmSummary(stack)
      if (!summary) return { badge: 'Not set', variant: 'gray', dim: true }
      if (stack.slot === 'ab')
        return { badge: 'Occupied by experiment', variant: 'purple', dim: false, kind: 'traffic split' }
      const euclid = euclidData(stack)
      const detail = euclid
        ? `${euclid.rules?.length ?? 0} rule${(euclid.rules?.length ?? 0) === 1 ? '' : 's'} · first match wins`
        : stack.slot === 'volume'
          ? volumeSplits(stack).map((s) => `${s.output.gateway_name} ${s.split}%`).join(' / ')
          : undefined
      return {
        badge: `“${summary.name}” active`,
        variant: 'green',
        dim: false,
        detail,
        kind:
          (isHybrid(stack) ? 'step 1 · ' : '') +
          (stack.slot === 'volume' ? 'weighted pick' : 'hard filter'),
        kindTone: 'filter',
      }
    },
  },
  {
    id: 'eligibility',
    group: 'Candidates — who can process it',
    icon: Filter,
    name: 'Eligibility check',
    what: (stack) =>
      `Connectors that can’t process the payment’s method are removed from ${
        stack.slot === 'none' ? 'your integration’s list' : 'the strategy’s output'
      }. Fails open: if the filter graph can’t be built, nothing is removed.`,
    runsWhen: 'Whenever the payment’s method type is known.',
    configuredBy: 'Platform payment-method filters (not merchant-editable today).',
    api: 'pm_filters graph inside POST /routing/evaluate',
    view: () => ({ badge: 'Always runs', variant: 'blue', dim: false, kind: 'hard filter', kindTone: 'filter' }),
  },
  {
    id: 'preferred',
    group: 'Ordering — who should get it',
    icon: Star,
    name: 'Preferred gateway',
    what: () =>
      'A payment that pins a gateway (with dynamic switching off) is answered on the spot — or fails if the pinned gateway isn’t eligible. With dynamic switching on, the preference only boosts it to the front.',
    runsWhen: 'Only when a preferred gateway is present on the payment itself.',
    configuredBy: 'Set per order by your integration — the dashboard cannot know it in advance.',
    api: 'payment_info.preferred_gateway on POST /decide-gateway',
    // Always request-supplied, never merchant-configured, so it never joins the rail; the stack
    // board's Overrides row is where it stays discoverable.
    view: () => ({ badge: 'Per payment', variant: 'blue', dim: true }),
  },
  {
    id: 'priority',
    group: 'Ordering — who should get it',
    icon: SlidersHorizontal,
    name: 'Baseline priority',
    what: (stack) =>
      stack.srConfigured
        ? 'The static gateway priority (or priority script) sets the starting order and can enforce a hard allow-list; success-rate scores take over from here.'
        : 'With success-rate scoring off, these positions become the scores themselves (1.0, 0.9, 0.8…) — in effect, the priority order decides the ranking.',
    runsWhen: 'Every gateway decision, unless the request forces pure success-rate ranking.',
    configuredBy: 'Gateway priority on the merchant account.',
    api: 'merchant_account.gateway_priority',
    view: (stack) => ({
      badge: 'Always runs',
      variant: 'blue',
      dim: false,
      kind: 'ordering',
      kindTone: 'rank',
      detail: stack.srConfigured ? undefined : 'positions become scores',
    }),
  },
  {
    id: 'sr',
    group: 'Ordering — who should get it',
    icon: TrendingUp,
    name: 'Success-rate scoring',
    what: (stack) =>
      'Live rolling success rates for this payment’s dimensions re-rank the candidates. A hedging slice of traffic explores uniformly so the scores never go stale.' +
      (stack.autopilotOn ? ' Autopilot is retuning the score window and hedging automatically.' : ''),
    runsWhen: 'When success-rate routing is configured for your account (or forced by the request).',
    configuredBy: 'The Multi Objective page — score window, hedging %, per-method overrides, Autopilot.',
    configureTo: '/routing/sr',
    api: 'POST /rule/get {algorithm: "successRate"}',
    view: (stack) => {
      if (!stack.srConfigured) return { badge: 'Not set', variant: 'gray', dim: true }
      const parts: string[] = []
      if (stack.srData?.defaultBucketSize) parts.push(`window ${stack.srData.defaultBucketSize}`)
      if (stack.srData?.defaultHedgingPercent != null) parts.push(`hedging ${stack.srData.defaultHedgingPercent}%`)
      return {
        badge: stack.autopilotOn && !stack.srData ? 'Auto-pilot' : 'Configured',
        variant: 'green',
        dim: false,
        kind: (isHybrid(stack) ? 'step 2 · ' : '') + 're-rank',
        kindTone: 'rank',
        detail: parts.join(' · ') || (stack.autopilotOn ? 'tuned automatically' : undefined),
      }
    },
  },
  {
    id: 'health',
    group: 'Ordering — who should get it',
    icon: Activity,
    name: 'Health penalties',
    what: (stack) =>
      `Scheduled outages divide a gateway’s score by 10; elimination divides below-threshold gateways by 5 — on top of ${
        stack.srConfigured ? 'their live success-rate scores' : 'the priority-based scores'
      } — and relabels the decision as downtime routing. Penalised gateways are demoted, never removed.`,
    runsWhen: 'Outage checks always run; elimination needs its configuration (or the request flag).',
    configuredBy: 'Elimination thresholds on the Multi Objective page.',
    configureTo: '/routing/sr',
    api: 'POST /rule/get {algorithm: "elimination"}',
    view: (stack) =>
      stack.eliminationConfigured
        ? {
            badge: 'Elimination on',
            variant: 'green',
            dim: false,
            kind: 'demote',
            kindTone: 'filter',
            detail:
              stack.eliminationData?.threshold != null
                ? // The threshold is stored as a 0..1 fraction; tolerate percent-scale values too.
                  `threshold ${Math.round(
                    stack.eliminationData.threshold > 1
                      ? stack.eliminationData.threshold
                      : stack.eliminationData.threshold * 100,
                  )}%`
                : undefined,
          }
        : { badge: 'Outages only', variant: 'gray', dim: true },
  },
  {
    id: 'cost',
    group: 'Ordering — who should get it',
    icon: Coins,
    name: 'Cost optimization',
    what: (stack) =>
      'Expected value = auth score × (margin − cost). A gateway that trails slightly on auth but processes cheaper takes the lead. Steps aside without cost data, and for exploration traffic.' +
      (stack.srConfigured
        ? ''
        : ' Note: with success-rate scoring off, the auth side of the trade-off comes from priority positions, not live data.'),
    runsWhen: 'When multi-objective routing is enabled.',
    configuredBy: 'The Multi Objective page — the margin dial sets how much success rate you’ll trade for cost.',
    configureTo: '/routing/sr',
    api: 'features: multi_objective_routing_enabled',
    view: (stack) =>
      stack.costOn
        ? {
            badge: 'Enabled',
            variant: 'green',
            dim: false,
            kind: 're-rank',
            kindTone: 'rank',
            detail: stack.srData?.margin != null ? `margin ${stack.srData.margin * 100}%` : undefined,
          }
        : { badge: 'Not set', variant: 'gray', dim: true },
  },
  {
    id: 'commitment',
    group: 'Ordering — who should get it',
    icon: Scale,
    name: 'Volume commitment',
    what: () =>
      'A contracted gateway that is behind on its committed volume and within quality tolerance of the leader can win a dice-roll and take the payment. The last stage that can change the answer; fails open on a stale plan.',
    runsWhen: 'When volume-commitment routing is enabled and a current steering plan exists.',
    configuredBy: 'Volume contracts on the Multi Objective page.',
    configureTo: '/routing/sr?tab=volume',
    api: 'GET /merchant-account/{merchant_id}/volume-commitment',
    view: (stack) =>
      stack.volumeCommitmentOn
        ? { badge: 'Enabled', variant: 'green', dim: false, kind: 'override', kindTone: 'override' }
        : { badge: 'Not set', variant: 'gray', dim: true },
  },
  {
    id: 'decide',
    group: 'Decision & learning',
    icon: Flag,
    name: 'Decision',
    what: (stack) => {
      const suffix =
        ' The response’s routing_approach names the deciding stage; if no candidate survived, the decision fails with GATEWAY_NOT_FOUND — there is no hidden default.'
      if (stack.srConfigured && stack.costOn)
        return `The best live success-rate score leads, and cost optimization may hand the payment to a near-tied cheaper gateway; the rest queue as ordered fallbacks.${suffix}`
      if (stack.srConfigured)
        return `The best live success-rate score wins; the rest queue as ordered fallbacks.${suffix}`
      if (stack.slot === 'rule' || stack.slot === 'volume')
        return `With success-rate scoring off, your strategy’s order decides — the first eligible connector it emits wins, and the rest queue in its order.${suffix}`
      return `With no strategy and no scoring, nothing filters or reorders — the payment simply goes to your integration’s fallback: the eligibleGatewayList sent on /decide-gateway. Every candidate in it ties, so the pick within the list is effectively arbitrary; activate a strategy or configure scoring to make it deterministic.${suffix}`
    },
    runsWhen: 'Every decision ends here, unless answered early above.',
    configuredBy: '—',
    api: 'DecidedGateway response of POST /decide-gateway',
    view: (stack) => ({
      badge: 'Always runs',
      variant: 'blue',
      dim: false,
      kind: 'pick winner',
      detail:
        stack.srConfigured && stack.costOn
          ? 'best score + cost wins'
          : stack.srConfigured
            ? 'best score wins'
            : stack.slot === 'rule' || stack.slot === 'volume'
              ? 'strategy order wins'
              : 'integration fallback wins',
    }),
  },
  {
    id: 'learn',
    group: 'Decision & learning',
    icon: RefreshCcw,
    name: 'Learning loop',
    what: (stack) => {
      if (stack.srConfigured && stack.eliminationConfigured)
        return 'Your integration reports each payment’s outcome; success-rate and elimination scores update, and tomorrow’s ranking shifts. No outcome reported means no learning.'
      if (stack.srConfigured)
        return 'Your integration reports each payment’s outcome; success-rate scores update, and tomorrow’s ranking shifts. No outcome reported means no learning.'
      if (stack.eliminationConfigured)
        return 'Your integration reports each payment’s outcome; elimination health scores update. Success-rate scoring is off, so the ranking itself doesn’t learn yet — only health penalties do.'
      return 'Your integration can report outcomes here, but nothing reads them yet — success-rate scoring and elimination are both off, so reported results don’t change routing today.'
    },
    runsWhen: 'Whenever the score-update endpoint is called after the payment settles or fails.',
    configuredBy: 'An integration responsibility.',
    api: 'POST /update-gateway-score',
    view: () => ({ badge: 'Integration-controlled', variant: 'blue', dim: false, kind: 'feedback' }),
  },
]

const GROUP_ORDER = [
  'Entry',
  'Experiment layer',
  'Network branch',
  'Candidates — who can process it',
  'Ordering — who should get it',
  'Decision & learning',
]

/**
 * The Decision Flow view, embedded as a tab of the Routing Hub (?tab=flow). It owns its own data
 * reads (the SWR keys are shared with the hub's, so nothing is fetched twice).
 */
export function DecisionFlowView() {
  const selectedMerchantId = useMerchantStore((state) => state.merchantId)
  const authMerchantId = useAuthStore((state) => state.user?.merchantId || '')
  const merchantId = selectedMerchantId || authMerchantId
  const debitRoutingFlag = useDebitRoutingFlag(merchantId)
  const merchantFeatures = useMerchantFeatures(merchantId || undefined)
  const [openStage, setOpenStage] = useState<StageId | null>(null)
  const flowRef = useRef<HTMLDivElement>(null)

  const { data: activeAlgorithms, isLoading: activeLoading, error: activeError } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`),
  )
  // Both /rule/get fetches 404 until their config is created; that is the normal "not configured"
  // state, so never retry it and don't refetch on focus (matching SRRoutingPage on the same keys).
  const { data: srConfig, isLoading: srLoading, error: srError } = useSWR<RuleConfig>(
    merchantId ? ['/rule/get', 'successRate', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'successRate' }),
    { shouldRetryOnError: false, revalidateOnFocus: false },
  )
  const { data: elimConfig, isLoading: elimLoading, error: elimError } = useSWR<RuleConfig>(
    merchantId ? ['/rule/get', 'elimination', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'elimination' }),
    { shouldRetryOnError: false, revalidateOnFocus: false },
  )

  const srData = (((srConfig as any)?.config?.data ?? srConfig?.data) as SRConfigData | undefined) ?? null
  const eliminationData =
    (((elimConfig as any)?.config?.data ?? elimConfig?.data) as EliminationData | undefined) ?? null

  const stack = deriveStack({
    activeAlgorithms,
    srData,
    eliminationData,
    isFeatureEnabled: (feature) => merchantFeatures.isEnabled(feature),
    debitEnabled: debitRoutingFlag.isEnabled,
  })
  const laneModel = deriveLanes(stack)
  const loading =
    activeLoading || srLoading || elimLoading || debitRoutingFlag.isLoading || merchantFeatures.isLoading
  // A 404 on any of these reads means "nothing configured for this merchant" — the legitimate
  // starting state. Anything else means the page doesn't actually know the merchant's state and
  // must not claim it does.
  const isRealError = (error: unknown) => Boolean(error) && apiErrorStatus(error) !== 404
  const loadFailed =
    isRealError(activeError) ||
    isRealError(srError) ||
    isRealError(elimError) ||
    isRealError(merchantFeatures.error) ||
    isRealError(debitRoutingFlag.error)

  return (
    <div className="max-w-[1100px] space-y-6">
      <p className={type.subheading}>
        Follow your connectors through every stage — read live from your configuration.
      </p>

      <StackBoard stack={stack} loading={loading} loadFailed={loadFailed} hasMerchant={Boolean(merchantId)} />

      <Card>
        <CardHeader className="flex flex-wrap items-baseline justify-between gap-2">
          <h2 className={type.heading}>The journey of one payment</h2>
          <p className={type.bodySmall}>
            {laneModel.ghost
              ? 'Showing an example connector set — activate a strategy to see yours.'
              : 'Lanes are your connectors. Click any stage to see what it does.'}
          </p>
        </CardHeader>
        <CardBody className="overflow-x-auto">
          <div ref={flowRef} className="relative min-w-[600px]">
            <LaneCanvas
              containerRef={flowRef}
              lanes={laneModel.lanes}
              ghost={laneModel.ghost}
              deterministicHead={laneModel.deterministicHead}
              overflow={laneModel.overflow}
            />
            <FlowRail
              stack={stack}
              connectorCount={laneModel.ghost ? 0 : laneModel.lanes.length + laneModel.overflow}
              overflow={laneModel.overflow}
              loadFailed={loadFailed}
              merchantId={merchantId}
              laneNames={laneModel.lanes.map((lane) => lane.name)}
              openStage={openStage}
              onToggle={(id) => setOpenStage((current) => (current === id ? null : id))}
            />
          </div>
        </CardBody>
      </Card>
    </div>
  )
}

/* ── the read-only routing-stack status board ─────────────────────────────── */

function StackBoard({
  stack,
  loading,
  loadFailed,
  hasMerchant,
}: {
  stack: StackState
  loading: boolean
  loadFailed: boolean
  hasMerchant: boolean
}) {
  const summary = slotAlgorithmSummary(stack)
  const warnings: Array<{ tone: 'warn' | 'info'; text: string }> = []
  if (!loading && !loadFailed && hasMerchant) {
    if (stack.slot === 'none')
      warnings.push({
        tone: 'warn',
        text: 'No strategy is active — your integration’s connector list goes to scoring with only the eligibility check applied.',
      })
    if (stack.slot === 'ab' && !stack.abRealPaymentsOn)
      warnings.push({
        tone: 'warn',
        text: 'An experiment holds the slot but the real-payments flag is off — /decide-gateway traffic is not intercepted, while /routing/evaluate calls still follow the experiment’s arms.',
      })
    if (stack.slot === 'ab' && stack.abRealPaymentsOn)
      warnings.push({
        tone: 'info',
        text: 'While the experiment runs, rules and volume splits are paused — they share one activation slot.',
      })
    if (stack.srConfigured && !stack.eliminationConfigured)
      warnings.push({
        tone: 'warn',
        text: 'Success-rate scoring is on but elimination is off — an unhealthy gateway keeps its rank until scores catch up.',
      })
    if (stack.debitOn)
      warnings.push({
        tone: 'info',
        text: 'Debit routing runs only when your integration asks for it on the request.',
      })
  }

  return (
    <Card>
      <CardHeader className="flex flex-wrap items-baseline gap-2">
        <h2 className={type.heading}>Your routing stack</h2>
        <p className={type.bodySmall}>
          {hasMerchant ? 'read from your saved configuration' : 'select a merchant to load configuration'}
        </p>
        {loading ? <Badge variant="blue">Loading…</Badge> : null}
      </CardHeader>
      <CardBody className="space-y-3 py-4">
        {loadFailed ? (
          <div className="rounded-lg border border-red-500/20 bg-red-500/8 px-3 py-2 text-sm text-red-600 dark:text-red-400">
            Couldn’t load part of your configuration — the states below may be incomplete. Retry by reloading the page.
          </div>
        ) : null}
        <BoardRow label="Active strategy">
          {summary ? (
            <span
              className={`inline-flex items-center gap-2.5 rounded-xl border px-3.5 py-2 ${
                stack.slot === 'ab'
                  ? 'border-purple-300/60 bg-purple-500/5 dark:border-purple-500/40 dark:bg-purple-500/10'
                  : 'border-emerald-300/60 bg-emerald-500/5 dark:border-emerald-500/40 dark:bg-emerald-500/10'
              }`}
            >
              <StateDot on={stack.slot !== 'ab' ? 'on' : 'vio'} />
              <span className={type.labelSmall}>{summary.kicker}</span>
              <span className="text-[13px] font-semibold text-slate-900 dark:text-white">{summary.name}</span>
            </span>
          ) : (
            <span className="inline-flex items-center gap-2.5 rounded-xl border border-dashed border-slate-300 px-3.5 py-2 dark:border-[#3a4150]">
              <StateDot on="off" />
              <span className={type.body}>No strategy active</span>
              <ConfigureLink to="/routing/rules" />
            </span>
          )}
        </BoardRow>
        <BoardRow label="Optimization">
          <StatusChip on={stack.srConfigured} label="Success-rate scoring" to="/routing/sr" />
          <StatusChip on={stack.eliminationConfigured} label="Elimination" to="/routing/sr" />
          <StatusChip on={stack.costOn} label="Cost optimization" to="/routing/sr" />
          <StatusChip on={stack.volumeCommitmentOn} label="Volume commitment" to="/routing/sr?tab=volume" />
        </BoardRow>
        <BoardRow label="Overrides">
          <StatusChip on={stack.debitOn ? 'req' : false} label="Debit routing" to="/routing/debit" />
          <StatusChip on="req" label="Preferred gateway" note="per payment" />
        </BoardRow>
        {warnings.length > 0 ? (
          <div className="space-y-2 border-t border-slate-100 pt-3 dark:border-[#1e2535]">
            {warnings.map((warning) => (
              <div key={warning.text} className="flex items-start gap-2.5">
                <span
                  className={`mt-[7px] h-1.5 w-1.5 flex-shrink-0 rounded-full ${
                    warning.tone === 'warn' ? 'bg-amber-400' : 'bg-sky-400'
                  }`}
                />
                <p className={type.bodySmall}>{warning.text}</p>
              </div>
            ))}
          </div>
        ) : null}
      </CardBody>
    </Card>
  )
}

function BoardRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <span className={`${type.labelSmall} w-32 flex-shrink-0`}>{label}</span>
      <div className="flex flex-wrap items-center gap-x-5 gap-y-2">{children}</div>
    </div>
  )
}

function StateDot({ on }: { on: 'on' | 'off' | 'req' | 'vio' }) {
  if (on === 'off')
    return <span className="h-[7px] w-[7px] flex-shrink-0 rounded-full border-[1.5px] border-slate-400 dark:border-[#6d778a]" />
  const color =
    on === 'req'
      ? 'bg-sky-400 shadow-[0_0_7px_rgba(56,189,248,0.5)]'
      : on === 'vio'
        ? 'bg-violet-400 shadow-[0_0_7px_rgba(167,139,250,0.5)]'
        : 'bg-emerald-400 shadow-[0_0_7px_rgba(52,211,153,0.5)]'
  return <span className={`h-[7px] w-[7px] flex-shrink-0 rounded-full ${color}`} />
}

function StatusChip({
  on,
  label,
  to,
  note,
}: {
  on: boolean | 'req'
  label: string
  to?: string
  note?: string
}) {
  const off = on === false
  return (
    <span className={`inline-flex items-center gap-2 text-[13px] ${off ? 'text-slate-500 dark:text-[#78849a]' : 'text-slate-800 dark:text-[#c4cfdf]'}`}>
      <StateDot on={on === 'req' ? 'req' : on ? 'on' : 'off'} />
      {label}
      {note ? <span className={type.bodySmall}>· {note}</span> : null}
      {off && to ? <ConfigureLink to={to} /> : null}
    </span>
  )
}

function ConfigureLink({ to }: { to: string }) {
  return (
    <Link
      to={to}
      className="inline-flex items-center gap-0.5 text-[11px] font-semibold text-brand-600 hover:underline dark:text-[#93c5fd]"
    >
      Configure
      <ChevronRight size={11} />
    </Link>
  )
}

/* ── example requests: the smallest call that exercises each stage ────────── */

function curlFor(path: string, body: unknown) {
  return `curl -X POST '$DE_HOST${path}' \\\n  -H 'content-type: application/json' \\\n  -d '${JSON.stringify(body, null, 2)}'`
}

/**
 * A minimal, copy-pasteable request per stage — required fields only, rebuilt from the live
 * configuration (real connector names and rule parameters when a strategy provides them, a
 * sensible default set otherwise).
 */
function exampleRequest(
  stageId: StageId,
  stack: StackState,
  merchantId: string,
  laneNames: string[],
): { code: string; note?: string; label?: string } | null {
  const mid = merchantId || 'your_merchant_id'
  const decideBody = {
    merchantId: mid,
    eligibleGatewayList: laneNames,
    paymentInfo: { paymentId: 'PAY_001', amount: 1500, currency: 'INR', paymentMethodType: 'card' },
  }
  switch (stageId) {
    case 'arrive':
    case 'decide': {
      // A plain /decide-gateway call never consults the static strategy — its candidate list is
      // caller-supplied. With a rule set or volume split active, the right integration is
      // /routing/hybrid: the strategy's output becomes the eligible list for scoring.
      if (stack.slot === 'rule' || stack.slot === 'volume') {
        const strategyName = slotAlgorithmSummary(stack)?.name ?? 'your strategy'
        const { eligibleGatewayList: _omitted, ...dynamicBody } = decideBody
        return {
          code: curlFor('/routing/hybrid', {
            static_routing_request: {
              created_by: mid,
              payment_id: 'PAY_001',
              parameters: exampleEvaluateParameters(stack),
              fallback_output: laneNames.map((name) => ({ gateway_name: name, gateway_id: null })),
            },
            dynamic_routing_request: dynamicBody,
          }),
          note: `With “${strategyName}” active, call /routing/hybrid — the strategy picks the candidates and its output feeds scoring as the eligible list automatically. Sending your own eligibleGatewayList in the dynamic half would override it.`,
        }
      }
      return {
        code: curlFor('/decide-gateway', decideBody),
        note:
          stack.slot === 'none' && !stack.srConfigured
            ? 'With nothing configured, the answer comes straight from this eligibleGatewayList.'
            : undefined,
      }
    }
    case 'slot':
      if (stack.slot === 'ab') return null
      return {
        code: curlFor('/routing/evaluate', {
          created_by: mid,
          payment_id: 'PAY_001',
          fallback_output: laneNames.map((name) => ({ gateway_name: name, gateway_id: null })),
          parameters: exampleEvaluateParameters(stack),
        }),
        note:
          stack.slot === 'rule'
            ? 'These parameters satisfy your first rule, so the response shows it matching.'
            : undefined,
      }
    case 'sr':
      return {
        code: curlFor('/decide-gateway', { ...decideBody, rankingAlgorithm: 'SR_BASED_ROUTING' }),
        note: 'rankingAlgorithm forces pure success-rate ranking for this request.',
      }
    case 'debit':
      return {
        code: curlFor('/decide-gateway', { ...decideBody, rankingAlgorithm: 'NTW_BASED_ROUTING' }),
        note: 'The co-badged card details must ride along in paymentInfo.metadata.',
      }
    case 'eligibility':
      // The filter graph is built from the server's TOML, which no API exposes — this excerpt
      // shows its real shape (values from config/development.toml) so the mechanism is concrete.
      return {
        label: 'How it’s configured — pm_filters in the server’s config/<env>.toml',
        code: [
          '[pm_filters.razorpay]',
          'upi_collect = { country = "IN", currency = "INR" }',
          '',
          '[pm_filters.default]',
          'affirm  = { country = "US", currency = "USD" }',
          'giropay = { country = "DE", currency = "EUR" }',
          'klarna  = { country = "AT,BE,DK,…,US,CA", currency = "USD,GBP,EUR,…" }',
        ].join('\n'),
        note: 'Per connector and payment method, on country and currency: a payment outside a connector’s lists drops that connector here. Server-owned config — not editable from the dashboard yet.',
      }
    case 'learn':
      return {
        code: curlFor('/update-gateway-score', {
          merchantId: mid,
          gateway: laneNames[0] ?? 'razorpay',
          gatewayReferenceId: null,
          status: 'CHARGED',
          paymentId: 'PAY_001',
        }),
        note: 'Report the same paymentId you routed — the decision context is kept for 30 minutes.',
      }
    default:
      return null
  }
}

function CodeSnippet({ code, note, label }: { code: string; note?: string; label?: string }) {
  return (
    <div className="mt-3">
      <p className={`${type.labelSmall} mb-1.5`}>{label ?? 'Example request — required fields only'}</p>
      <pre className="overflow-x-auto rounded-lg border border-slate-200 bg-white px-3 py-2.5 font-mono text-[10.5px] leading-[16px] text-slate-700 dark:border-[#1e2535] dark:bg-[#0d1118] dark:text-[#a8b4c8]">
        {code}
      </pre>
      {note ? <p className={`${type.bodySmall} mt-1.5`}>{note}</p> : null}
    </div>
  )
}

/* ── the stage rail ───────────────────────────────────────────────────────── */

function FlowRail({
  stack,
  connectorCount,
  overflow,
  loadFailed,
  merchantId,
  laneNames,
  openStage,
  onToggle,
}: {
  stack: StackState
  connectorCount: number
  overflow: number
  loadFailed: boolean
  merchantId: string
  laneNames: string[]
  openStage: StageId | null
  onToggle: (id: StageId) => void
}) {
  let firstGapRendered = false
  return (
    <div>
      {GROUP_ORDER.map((group) => {
        // Only stages that actually run for this merchant are drawn; a whole group can vanish.
        const stages = STAGES.filter(
          (stage) => stage.group === group && !stage.view(stack, connectorCount).dim,
        )
        if (stages.length === 0) return null
        return (
          <div key={group}>
            <p className={`${type.labelSmall} relative z-[2] pb-2 pt-3`}>{group}</p>
            {stages.map((stage) => {
              const gapKind = !firstGapRendered
                ? 'fan'
                : stage.id === 'decide'
                  ? 'converge'
                  : stage.id === 'slot' && stack.slot === 'volume'
                    ? 'split'
                    : stage.id === 'eligibility'
                      ? 'filter'
                      : stage.id === 'sr'
                        ? 'sort'
                        : 'straight'
              const renderGap = stage.id !== 'learn'
              if (renderGap) firstGapRendered = true
              return (
                <div key={stage.id}>
                  <StageRow
                    stage={stage}
                    stack={stack}
                    connectorCount={connectorCount}
                    overflow={overflow}
                    loadFailed={loadFailed}
                    merchantId={merchantId}
                    laneNames={laneNames}
                    open={openStage === stage.id}
                    onToggle={() => onToggle(stage.id)}
                  />
                  {renderGap ? (
                    <div
                      data-lane-gap={gapKind}
                      className={
                        gapKind === 'fan'
                          ? 'h-[58px]'
                          : gapKind === 'converge'
                            ? 'h-[56px]'
                            : gapKind === 'filter'
                              ? 'h-[62px]'
                              : gapKind === 'split'
                                ? 'h-[46px]'
                                : gapKind === 'sort'
                                  ? 'h-[64px]'
                                  : 'h-[34px]'
                      }
                    />
                  ) : null}
                  {stage.id === 'decide' ? (
                    <div className="relative z-[2] flex items-center gap-2 pb-1 pl-1">
                      <RefreshCcw size={12} className="text-slate-400 dark:text-[#78849a]" />
                      <span className={type.bodySmall}>outcomes feed tomorrow’s scores</span>
                    </div>
                  ) : null}
                </div>
              )
            })}
          </div>
        )
      })}
    </div>
  )
}

function StageRow({
  stage,
  stack,
  connectorCount,
  overflow,
  loadFailed,
  merchantId,
  laneNames,
  open,
  onToggle,
}: {
  stage: StageDef
  stack: StackState
  connectorCount: number
  overflow: number
  loadFailed: boolean
  merchantId: string
  laneNames: string[]
  open: boolean
  onToggle: () => void
}) {
  const view = stage.view(stack, connectorCount)
  const Icon = stage.icon
  const kindTone =
    view.kindTone === 'filter'
      ? 'text-amber-600 dark:text-amber-400/90'
      : view.kindTone === 'rank'
        ? 'text-sky-600 dark:text-sky-300/90'
        : view.kindTone === 'override'
          ? 'text-violet-600 dark:text-violet-300/90'
          : 'text-slate-400 dark:text-[#78849a]'
  return (
    <div className="relative z-[2]">
      {view.kind ? (
        // Sits across the card's top-right border like a ribbon; the Card-background fill masks
        // the border segment beneath it.
        <span
          className={`absolute -top-[7px] right-4 z-[1] rounded bg-white px-1.5 text-[9px] font-semibold uppercase tracking-wider dark:bg-[#11151d] ${kindTone}`}
        >
          {view.kind}
        </span>
      ) : null}
      <div
        className={`overflow-hidden rounded-2xl border transition-colors ${
          open
            ? 'border-brand-500/50 dark:border-[#3b82f6]/50'
            : 'border-slate-200 hover:border-slate-300 dark:border-[#1e2535] dark:hover:border-[#3a4150]'
        } bg-white dark:bg-[#161b24]`}
      >
        <button
          type="button"
          onClick={onToggle}
          aria-expanded={open}
          className="flex w-full items-center gap-2.5 px-4 py-2.5 text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-[#3b82f6]/40"
        >
          <span
            className={`flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg border ${
              view.variant === 'green'
                ? 'border-emerald-200/60 bg-emerald-50 text-emerald-700 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-400'
                : view.variant === 'purple'
                  ? 'border-purple-200/60 bg-purple-50 text-purple-700 dark:border-purple-500/25 dark:bg-purple-500/10 dark:text-purple-400'
                  : 'border-slate-200 bg-slate-50 text-slate-500 dark:border-[#273141] dark:bg-[#0c1119] dark:text-[#8d96aa]'
            }`}
          >
            <Icon size={14} />
          </span>
          <span className="text-[13px] font-semibold text-slate-900 dark:text-white">{stage.name}</span>
          <span className="ml-auto flex flex-shrink-0 items-center gap-2">
            {view.detail ? (
              <span className="hidden font-mono text-[11px] text-brand-700 dark:text-[#93c5fd] sm:inline">
                {view.detail}
              </span>
            ) : null}
            <Badge variant={view.variant}>{view.badge}</Badge>
            <ChevronRight
              size={12}
              className={`text-slate-400 transition-transform dark:text-[#6d778a] ${open ? 'rotate-90' : ''}`}
            />
          </span>
        </button>
        {open ? (
          <div className="border-t border-slate-100 bg-slate-50/60 px-4 py-3 dark:border-[#1e2535] dark:bg-black/15">
            <p className={type.body}>{stage.what(stack)}</p>
            <StageLiveDetail stage={stage} stack={stack} overflow={overflow} loadFailed={loadFailed} />
            {(() => {
              const example = exampleRequest(stage.id, stack, merchantId, laneNames)
              return example ? <CodeSnippet code={example.code} note={example.note} label={example.label} /> : null
            })()}
            <dl className="mt-3 space-y-1.5">
              <ExpansionFact label="Runs when" value={stage.runsWhen} />
              <ExpansionFact label="Configured by" value={stage.configuredBy} />
              <ExpansionFact label="Powered by" value={stage.api} mono />
            </dl>
            {stage.configureTo ? (
              <div className="mt-3">
                <ConfigureLink to={stage.configureTo} />
              </div>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  )
}

function ExpansionFact({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex gap-2">
      <dt className={`${type.bodySmall} w-24 flex-shrink-0`}>{label}</dt>
      <dd className={mono ? 'font-mono text-[11px] leading-4 text-brand-700 dark:text-[#93c5fd]' : type.bodySmall}>
        {value}
      </dd>
    </div>
  )
}

/** The live, merchant-specific facts inside a stage expansion — real rules, real splits. */
function StageLiveDetail({
  stage,
  stack,
  overflow,
  loadFailed,
}: {
  stage: StageDef
  stack: StackState
  overflow: number
  loadFailed: boolean
}) {
  if (stage.id === 'slot') {
    const euclid = euclidData(stack)
    if (euclid?.rules?.length) {
      return (
        <div className="mt-3 space-y-1.5">
          {euclid.rules.slice(0, 6).map((rule, i) => {
            const output = normalizeRuleOutput(rule)
            const destinations = [
              ...output.priorityGateways.map((g) => gatewayLabel(g)),
              ...output.volumeSplits.map((s) => gatewayLabel(s.output)),
              ...output.volumeSplitPriorityEntries.flatMap((entry) => (entry.output ?? []).map((g) => gatewayLabel(g))),
            ]
              .filter(Boolean)
              .slice(0, 4)
            return (
              <div key={`${rule.name}-${i}`} className="flex flex-wrap items-baseline gap-x-2">
                <span className="text-[12px] font-semibold text-slate-800 dark:text-[#c4cfdf]">{rule.name}</span>
                <span className={type.bodySmall}>{describeRuleConditions(rule)}</span>
                {destinations.length ? (
                  <span className="font-mono text-[11px] text-brand-700 dark:text-[#93c5fd]">
                    → {destinations.join(', ')}
                  </span>
                ) : null}
              </div>
            )
          })}
          {euclid.rules.length > 6 ? (
            <p className={type.bodySmall}>…and {euclid.rules.length - 6} more rules.</p>
          ) : null}
          {overflow > 0 ? (
            <p className={type.bodySmall}>{overflow} more connectors referenced than lanes drawn above.</p>
          ) : null}
        </div>
      )
    }
    const splits = volumeSplits(stack)
    if (splits.length) {
      return (
        <div className="mt-3 space-y-1">
          {splits.map((split, i) => (
            <div key={i} className="flex items-baseline gap-2">
              <span className="font-mono text-[12px] tabular-nums text-slate-800 dark:text-[#c4cfdf]">
                {split.split}%
              </span>
              <span className={type.bodySmall}>{gatewayLabel(split.output)}</span>
            </div>
          ))}
          <p className={type.bodySmall}>Lane thickness above mirrors each connector’s share.</p>
        </div>
      )
    }
    return null
  }
  if (stage.id === 'ab') {
    const experiment = abTestData(stack)
    if (!experiment) return null
    return (
      <div className="mt-3 space-y-1">
        <p className={type.bodySmall}>
          Variant takes {experiment.variant_split_pct}% of payments · minimum sample{' '}
          {experiment.min_sample_size.toLocaleString()}.
        </p>
        {!stack.abRealPaymentsOn ? (
          <p className="text-[12px] leading-4 text-amber-600 dark:text-amber-400">
            The real-payments flag is off: /decide-gateway traffic is not intercepted, but
            /routing/evaluate calls still follow the experiment’s arms while it holds the slot.
          </p>
        ) : null}
      </div>
    )
  }
  // The decide stage's arbitrary-tie warning lives in its dynamic `what` copy; only flag when
  // that copy may be wrong because the configuration never fully loaded.
  if (stage.id === 'decide' && loadFailed) {
    return (
      <p className="mt-3 text-[12px] leading-4 text-amber-600 dark:text-amber-400">
        Part of your configuration didn’t load, so this description may not reflect your real setup.
      </p>
    )
  }
  return null
}
