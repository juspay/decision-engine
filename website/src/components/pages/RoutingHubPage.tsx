import type { ElementType } from 'react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import useSWR, { useSWRConfig } from 'swr'
import {
  BookOpen,
  ChevronRight,
  FlaskConical,
  Network,
  PieChart,
  PowerOff,
  TrendingUp,
} from 'lucide-react'
import { Card, CardBody, SurfaceLabel } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiPost, fetcher } from '../../lib/api'
import { AnalyticsOverviewResponse, RoutingAlgorithm, RuleConfig, SRConfigData } from '../../types/api'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'

type StrategyId = 'auth-rate' | 'rules' | 'volume' | 'debit' | 'ab-test'
type StrategyState = 'configured' | 'enabled' | 'not_set'

interface StrategyRow {
  id: StrategyId
  title: string
  description: string
  useCase: string
  icon: ElementType
  state: StrategyState
  canDeactivate: boolean
  href: string
}

const strategyLabels: Record<StrategyId, string> = {
  'auth-rate': 'Auth-Rate Configuration',
  rules: 'Rule-Based Routing',
  volume: 'Volume Split Routing',
  debit: 'Debit Routing',
  'ab-test': 'A/B Testing',
}

function formatCompactNumber(value: number) {
  return new Intl.NumberFormat(undefined, { notation: 'compact', maximumFractionDigits: 1 }).format(value)
}

function algorithmType(algorithm?: RoutingAlgorithm) {
  return (algorithm?.algorithm_data || algorithm?.algorithm)?.type || ''
}

function isRuleBasedAlgorithmType(type: string) {
  return type === 'advanced' || type === 'priority' || type === 'single'
}

export function RoutingHubPage() {
  const { mutate: mutateCache } = useSWRConfig()
  const selectedMerchantId = useMerchantStore((state) => state.merchantId)
  const authMerchantId = useAuthStore((state) => state.user?.merchantId || '')
  const merchantId = selectedMerchantId || authMerchantId
  const debitRoutingFlag = useDebitRoutingFlag(merchantId)
  const [deactivatingStrategy, setDeactivatingStrategy] = useState<StrategyId | null>(null)
  const [pendingDeactivateStrategy, setPendingDeactivateStrategy] = useState<StrategyId | null>(null)
  const [actionMessage, setActionMessage] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)

  const { data: activeAlgorithms, isLoading: activeLoading, mutate: mutateActiveAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`),
  )

  const { data: srConfig, isLoading: srLoading, mutate: mutateSrConfig } = useSWR<RuleConfig>(
    merchantId ? ['/rule/get', 'successRate', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'successRate' }),
  )

  const { data: analyticsData } = useSWR<AnalyticsOverviewResponse>(
    merchantId ? '/analytics/overview?range=1d' : null,
    fetcher,
    { shouldRetryOnError: false },
  )

  const activeAlgorithm = activeAlgorithms?.[0]
  const srData = ((srConfig as any)?.config?.data ?? srConfig?.data) as SRConfigData | undefined
  const hasAuthRateConfig = Boolean(srData)
  const hasRuleBasedRouting = (activeAlgorithms || []).some((a) => isRuleBasedAlgorithmType(algorithmType(a)))
  const hasVolumeSplit = (activeAlgorithms || []).some((a) => algorithmType(a) === 'volume_split')
  const hasDebitRouting = debitRoutingFlag.isEnabled
  const hasAbTest = (activeAlgorithms || []).some((a) => algorithmType(a) === 'ab_test')
  const loading = activeLoading || srLoading || debitRoutingFlag.isLoading
  const activeRuleAlgorithm = (activeAlgorithms || []).find((a) => isRuleBasedAlgorithmType(algorithmType(a)))
  const activeVolumeAlgorithm = (activeAlgorithms || []).find((a) => algorithmType(a) === 'volume_split')


  async function deactivateStrategy(strategyId: StrategyId) {
    if (!merchantId) return
    setPendingDeactivateStrategy(strategyId)
  }

  async function doDeactivateStrategy(strategyId: StrategyId) {
    setDeactivatingStrategy(strategyId)
    setActionMessage(null)
    setActionError(null)
    try {
      if (strategyId === 'auth-rate') {
        await apiPost('/rule/delete', { merchant_id: merchantId, algorithm: 'successRate' })
        await mutateSrConfig(undefined, { revalidate: false })
      } else if (strategyId === 'rules' && activeRuleAlgorithm) {
        await apiPost('/routing/deactivate', { created_by: merchantId, routing_algorithm_id: activeRuleAlgorithm.id })
        await Promise.all([
          mutateActiveAlgorithms(),
          mutateCache(['active-routing', merchantId]),
          mutateCache(['routing-list', merchantId]),
          mutateCache(`/routing/list/${merchantId}`),
        ])
      } else if (strategyId === 'volume' && activeVolumeAlgorithm) {
        await apiPost('/routing/deactivate', { created_by: merchantId, routing_algorithm_id: activeVolumeAlgorithm.id })
        await Promise.all([
          mutateActiveAlgorithms(),
          mutateCache(['active-routing', merchantId]),
          mutateCache(['routing-list', merchantId]),
          mutateCache(`/routing/list/${merchantId}`),
        ])
      } else if (strategyId === 'debit') {
        await debitRoutingFlag.setDebitRoutingEnabled(false)
      }
      setActionMessage(`${strategyLabels[strategyId]} deactivated.`)
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err))
    } finally {
      setDeactivatingStrategy(null)
    }
  }

  const strategies: StrategyRow[] = [
    {
      id: 'auth-rate',
      title: 'Auth-rate based',
      description: 'Steers traffic toward the best authorization rate using live connector score signals.',
      useCase: 'Use when you want automatic gateway selection driven by real-time success-rate data.',
      icon: TrendingUp,
      state: hasAuthRateConfig ? 'configured' : 'not_set',
      canDeactivate: hasAuthRateConfig,
      href: 'sr',
    },
    {
      id: 'rules',
      title: 'Rule based',
      description: 'Evaluates explicit business conditions before traffic reaches connector selection.',
      useCase: 'Use for BIN, network, country, amount, metadata, or merchant policy overrides.',
      icon: BookOpen,
      state: hasRuleBasedRouting ? 'enabled' : 'not_set',
      canDeactivate: Boolean(activeRuleAlgorithm),
      href: 'rules',
    },
    {
      id: 'volume',
      title: 'Volume split',
      description: 'Distributes payments by configured percentages across gateways.',
      useCase: 'Use for ramp-ups, new gateway rollout, traffic balancing, or controlled migrations.',
      icon: PieChart,
      state: hasVolumeSplit ? 'enabled' : 'not_set',
      canDeactivate: Boolean(activeVolumeAlgorithm),
      href: 'volume',
    },
    {
      id: 'debit',
      title: 'Debit routing',
      description: 'Enables debit-network decisions for co-badged card payment flows.',
      useCase: 'Use when debit network cost, issuer country, or regulated-card behavior matters.',
      icon: Network,
      state: hasDebitRouting ? 'enabled' : 'not_set',
      canDeactivate: hasDebitRouting,
      href: 'debit',
    },
    {
      id: 'ab-test',
      title: 'A/B Testing',
      description: 'Tests two routing strategies against each other with statistical significance reporting.',
      useCase: 'Use when validating a new algorithm, rule, or gateway before full rollout.',
      icon: FlaskConical,
      state: hasAbTest ? 'enabled' : 'not_set',
      canDeactivate: false,
      href: 'ab-testing',
    },
  ]

  const activeStrategies = strategies.filter((s) => s.state !== 'not_set')

  const topRules = analyticsData?.top_rules || []
  const activeNamedAlgorithm = activeAlgorithm?.name
    ? activeAlgorithm
    : null

  return (
    <div className="space-y-6 px-5 sm:px-6 lg:px-8 xl:px-10">
      <ConfirmDialog
        open={pendingDeactivateStrategy !== null}
        title={`Deactivate ${pendingDeactivateStrategy ? strategyLabels[pendingDeactivateStrategy] : ''}?`}
        description="This will stop routing decisions from using this strategy. You can reactivate it at any time."
        confirmLabel="Deactivate"
        variant="danger"
        onConfirm={() => { const s = pendingDeactivateStrategy!; setPendingDeactivateStrategy(null); doDeactivateStrategy(s) }}
        onCancel={() => setPendingDeactivateStrategy(null)}
      />

      <header>
        <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Routing Hub</h1>
      </header>

      {actionError && (
        <div className="rounded-lg border border-red-500/20 bg-red-500/8 px-3 py-2 text-sm text-red-600 dark:text-red-400">
          {actionError}
        </div>
      )}
      {actionMessage && (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-600 dark:text-emerald-400">
          {actionMessage}
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[1.3fr_0.7fr]">

        {/* ── strategy list ──────────────────────────────────── */}
        <Card>
          <CardBody className="p-0">
            <div className="px-6 pt-5">
              <SurfaceLabel>Routing strategies</SurfaceLabel>
            </div>
            <div className="mt-3 divide-y divide-slate-100 dark:divide-[#1e2535]">
              {strategies.map((strategy) => {
                const Icon = strategy.icon
                const active = strategy.state !== 'not_set'
                return (
                  <div
                    key={strategy.id}
                    className={`flex items-center gap-4 px-6 py-4 ${active ? 'bg-emerald-500/[0.04] dark:bg-emerald-500/[0.06]' : ''}`}
                  >
                    <div className={`rounded-xl border p-2.5 flex-shrink-0 ${
                      active
                        ? 'border-emerald-200/60 bg-emerald-50 text-emerald-600 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-400'
                        : 'border-slate-200 bg-slate-50 text-slate-400 dark:border-[#273141] dark:bg-[#0c1119] dark:text-[#6d778a]'
                    }`}>
                      <Icon size={17} />
                    </div>

                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-semibold text-slate-950 dark:text-white">{strategy.title}</p>
                        {strategy.state === 'enabled' || strategy.state === 'configured'
                          ? <Badge variant="green">{strategy.state === 'configured' ? 'Configured' : 'Enabled'}</Badge>
                          : <Badge variant="gray">Not set</Badge>}
                      </div>
                      <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#8390a7]">
                        {strategy.description}
                      </p>
                      <p className="mt-0.5 text-xs leading-5 text-slate-400 dark:text-[#5a6a82]">
                        {strategy.useCase}
                      </p>
                    </div>

                    <div className="flex flex-shrink-0 items-center gap-2">
                      {strategy.canDeactivate && (
                        <Button
                          size="sm"
                          variant="danger"
                          onClick={() => deactivateStrategy(strategy.id)}
                          disabled={deactivatingStrategy === strategy.id}
                        >
                          <PowerOff size={13} />
                          {deactivatingStrategy === strategy.id ? 'Deactivating' : 'Deactivate'}
                        </Button>
                      )}
                      <Link
                        to={strategy.href}
                        className="inline-flex items-center gap-1 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 transition-colors hover:bg-slate-50 dark:border-[#2a303a] dark:bg-[#11151d] dark:text-[#c4cfdf] dark:hover:bg-[#161b26]"
                      >
                        Configure
                        <ChevronRight size={13} className="text-slate-400 dark:text-[#5a6a82]" />
                      </Link>
                    </div>
                  </div>
                )
              })}
            </div>
          </CardBody>
        </Card>

        {/* ── status panel ───────────────────────────────────── */}
        <Card>
          <CardBody className="p-6 space-y-6">

            {/* Active strategies */}
            <div>
              <SurfaceLabel>Active strategies</SurfaceLabel>
              <div className="mt-3 divide-y divide-slate-100 dark:divide-[#1e2535]">
                {activeStrategies.length > 0 ? activeStrategies.map((s) => {
                  const Icon = s.icon
                  return (
                    <div key={s.id} className="flex items-center justify-between gap-3 py-2.5">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <Icon size={14} className="flex-shrink-0 text-emerald-500" />
                        <span className="truncate text-sm font-medium text-slate-900 dark:text-white">
                          {s.title}
                        </span>
                      </div>
                      <Badge variant="green">
                        {s.state === 'configured' ? 'Configured' : 'Enabled'}
                      </Badge>
                    </div>
                  )
                }) : (
                  <p className="py-2 text-sm text-slate-500 dark:text-[#8390a7]">
                    {loading ? 'Checking…' : 'No active strategies yet'}
                  </p>
                )}
              </div>
            </div>

            {/* Running algorithm */}
            {activeNamedAlgorithm && (
              <div className="border-t border-slate-100 pt-6 dark:border-[#1e2535]">
                <SurfaceLabel>Running algorithm</SurfaceLabel>
                <p className="mt-3 text-sm font-semibold text-slate-950 dark:text-white">
                  {activeNamedAlgorithm.name}
                </p>
                {activeNamedAlgorithm.description && (
                  <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#8390a7]">
                    {activeNamedAlgorithm.description}
                  </p>
                )}
                <div className="mt-2">
                  <Badge variant="blue">
                    {algorithmType(activeNamedAlgorithm).replace(/_/g, ' ')}
                  </Badge>
                </div>
              </div>
            )}

            {/* Top rules triggered */}
            <div className="border-t border-slate-100 pt-6 dark:border-[#1e2535]">
              <div className="flex items-center justify-between gap-2">
                <SurfaceLabel>Rules triggered</SurfaceLabel>
                <Badge variant="blue">Last 24h</Badge>
              </div>
              {topRules.length > 0 ? (
                <div className="mt-3 space-y-2">
                  {topRules.slice(0, 5).map((rule) => (
                    <div key={rule.rule_name} className="flex items-center justify-between gap-3">
                      <span className="truncate text-xs text-slate-700 dark:text-[#c4cfdf]">
                        {rule.rule_name}
                      </span>
                      <span className="flex-shrink-0 text-xs font-semibold tabular-nums text-slate-950 dark:text-white">
                        {formatCompactNumber(rule.count)}
                      </span>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="mt-2 text-xs text-slate-500 dark:text-[#8390a7]">
                  No rule hits in the last 24 hours.
                </p>
              )}
            </div>

            {/* Auth-rate config */}
            {hasAuthRateConfig && srData && (
              <div className="border-t border-slate-100 pt-6 dark:border-[#1e2535]">
                <div className="flex items-center justify-between gap-2">
                  <SurfaceLabel>Auth-rate config</SurfaceLabel>
                  <Badge variant="green">Configured</Badge>
                </div>
                <div className="mt-3 divide-y divide-slate-100 dark:divide-[#1e2535]">
                  <SRRow label="Score window" value={`${srData.defaultBucketSize} requests`} />
                  {srData.defaultHedgingPercent != null && (
                    <SRRow label="Hedging" value={`${srData.defaultHedgingPercent}%`} />
                  )}
                  {srData.defaultLatencyThreshold != null && (
                    <SRRow label="Latency threshold" value={`${srData.defaultLatencyThreshold} ms`} />
                  )}
                  {srData.defaultLowerResetFactor != null && (
                    <SRRow label="Lower reset" value={String(srData.defaultLowerResetFactor)} />
                  )}
                  {srData.defaultUpperResetFactor != null && (
                    <SRRow label="Upper reset" value={String(srData.defaultUpperResetFactor)} />
                  )}
                  {(srData.subLevelInputConfig?.length ?? 0) > 0 && (
                    <SRRow label="Payment method overrides" value={`${srData.subLevelInputConfig!.length}`} />
                  )}
                </div>
              </div>
            )}

          </CardBody>
        </Card>
      </div>
    </div>
  )
}

function SRRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3 py-2">
      <span className="text-xs text-slate-500 dark:text-[#8390a7]">{label}</span>
      <span className="text-xs font-semibold tabular-nums text-slate-950 dark:text-white">{value}</span>
    </div>
  )
}
