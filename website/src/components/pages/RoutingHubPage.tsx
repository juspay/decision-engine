import type { ElementType } from 'react'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import useSWR, { useSWRConfig } from 'swr'
import {
  Activity,
  ArrowRight,
  BarChart3,
  BookOpen,
  CheckCircle2,
  FlaskConical,
  GitBranch,
  Network,
  PieChart,
  PowerOff,
  ShieldCheck,
  TrendingUp,
} from 'lucide-react'
import { Card, CardBody, SurfaceLabel } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm, RuleConfig } from '../../types/api'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'

type StrategyId = 'auth-rate' | 'rules' | 'volume' | 'debit'
type StrategyState = 'configured' | 'enabled' | 'not_set'

interface StrategyRow {
  id: StrategyId
  title: string
  eyebrow: string
  description: string
  useCase: string
  icon: ElementType
  route: string
  state: StrategyState
  evidence: string
  canDeactivate: boolean
}

function algorithmType(algorithm?: RoutingAlgorithm) {
  return (algorithm?.algorithm_data || algorithm?.algorithm)?.type || ''
}

function isRuleBasedAlgorithmType(type: string) {
  return type === 'advanced' || type === 'priority' || type === 'single'
}

function stateBadge(state: StrategyState) {
  if (state === 'enabled') return <Badge variant="green">Enabled</Badge>
  if (state === 'configured') return <Badge variant="green">Configured</Badge>
  return <Badge variant="gray">Not set</Badge>
}

function routeForActiveType(type: string) {
  if (type === 'advanced') return '/routing/rules'
  if (type === 'volume_split') return '/routing/volume'
  if (type === 'priority' || type === 'single') return '/routing/rules'
  return '/routing'
}

export function RoutingHubPage() {
  const navigate = useNavigate()
  const { mutate: mutateCache } = useSWRConfig()
  const selectedMerchantId = useMerchantStore((state) => state.merchantId)
  const authMerchantId = useAuthStore((state) => state.user?.merchantId || '')
  const merchantId = selectedMerchantId || authMerchantId
  const debitRoutingFlag = useDebitRoutingFlag(merchantId)
  const [deactivatingStrategy, setDeactivatingStrategy] = useState<StrategyId | null>(null)
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

  const activeAlgorithm = activeAlgorithms?.[0]
  const activeType = algorithmType(activeAlgorithm)
  const hasAuthRateConfig = Boolean((srConfig as any)?.config?.data || srConfig?.data)
  const hasRuleBasedRouting = (activeAlgorithms || []).some((algorithm) => isRuleBasedAlgorithmType(algorithmType(algorithm)))
  const hasVolumeSplit = (activeAlgorithms || []).some((algorithm) => algorithmType(algorithm) === 'volume_split')
  const hasDebitRouting = debitRoutingFlag.isEnabled
  const readiness = [hasAuthRateConfig, hasRuleBasedRouting || hasVolumeSplit, hasDebitRouting].filter(Boolean).length
  const loading = activeLoading || srLoading || debitRoutingFlag.isLoading
  const activeRuleAlgorithm = (activeAlgorithms || []).find((algorithm) => isRuleBasedAlgorithmType(algorithmType(algorithm)))
  const activeVolumeAlgorithm = (activeAlgorithms || []).find((algorithm) => algorithmType(algorithm) === 'volume_split')
  const hasRuntimeStrategy = Boolean(activeAlgorithm || hasAuthRateConfig || hasDebitRouting)
  const activeRoute = activeAlgorithm
    ? routeForActiveType(activeType)
    : hasAuthRateConfig
      ? '/routing/sr'
      : hasDebitRouting
        ? '/routing/debit'
        : '/routing/rules'
  const activeStrategyId: StrategyId | null = isRuleBasedAlgorithmType(activeType)
    ? 'rules'
    : activeType === 'volume_split'
      ? 'volume'
      : hasAuthRateConfig
        ? 'auth-rate'
        : hasDebitRouting
          ? 'debit'
          : null
  const runtimeName = activeAlgorithm?.name
    || (hasAuthRateConfig ? 'Auth-rate based routing' : hasDebitRouting ? 'Debit routing' : 'No active strategy')
  const runtimeDescription = activeAlgorithm
    ? `${activeAlgorithm.description || 'Active routing strategy'} is currently selected for payment routing.`
    : hasAuthRateConfig && hasDebitRouting
      ? 'Success-rate routing is configured and debit routing is enabled for this merchant.'
      : hasAuthRateConfig
        ? 'Success-rate routing is configured and available for runtime routing decisions.'
        : hasDebitRouting
          ? 'Debit network routing is enabled for this merchant.'
          : 'Configure and activate a strategy before expecting runtime routing decisions to follow a custom policy.'
  const runtimeType = activeType
    ? activeType.replace('_', ' ')
    : hasAuthRateConfig
      ? 'success rate'
      : hasDebitRouting
        ? 'debit'
        : '--'

  async function deactivateStrategy(strategyId: StrategyId) {
    if (!merchantId) return
    const labels: Record<StrategyId, string> = {
      'auth-rate': 'auth-rate configuration',
      rules: 'rule-based routing',
      volume: 'volume split routing',
      debit: 'debit routing',
    }

    if (!window.confirm(`Deactivate ${labels[strategyId]} for ${merchantId}?`)) {
      return
    }

    setDeactivatingStrategy(strategyId)
    setActionMessage(null)
    setActionError(null)

    try {
      if (strategyId === 'auth-rate') {
        await apiPost('/rule/delete', { merchant_id: merchantId, algorithm: 'successRate' })
        await mutateSrConfig(undefined, { revalidate: false })
      } else if (strategyId === 'rules' && activeRuleAlgorithm) {
        await apiPost('/routing/deactivate', {
          created_by: merchantId,
          routing_algorithm_id: activeRuleAlgorithm.id,
        })
        await Promise.all([
          mutateActiveAlgorithms(),
          mutateCache(['active-routing', merchantId]),
          mutateCache(['routing-list', merchantId]),
          mutateCache(`/routing/list/${merchantId}`),
        ])
      } else if (strategyId === 'volume' && activeVolumeAlgorithm) {
        await apiPost('/routing/deactivate', {
          created_by: merchantId,
          routing_algorithm_id: activeVolumeAlgorithm.id,
        })
        await Promise.all([
          mutateActiveAlgorithms(),
          mutateCache(['active-routing', merchantId]),
          mutateCache(['routing-list', merchantId]),
          mutateCache(`/routing/list/${merchantId}`),
        ])
      } else if (strategyId === 'debit') {
        await debitRoutingFlag.setDebitRoutingEnabled(false)
      }
      setActionMessage(`${labels[strategyId]} deactivated.`)
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
      eyebrow: 'Performance routing',
      description: 'Use connector score updates to steer traffic toward the best authorization rate.',
      useCase: 'Best when you want automatic gateway choice based on live success-rate signals.',
      icon: TrendingUp,
      route: '/routing/sr',
      state: hasAuthRateConfig ? 'configured' : 'not_set',
      evidence: hasAuthRateConfig ? 'Score config is available for runtime decisions.' : 'Set score defaults before relying on auth-rate routing.',
      canDeactivate: hasAuthRateConfig,
    },
    {
      id: 'rules',
      title: 'Rule based',
      eyebrow: 'Policy routing',
      description: 'Enforce explicit business conditions before traffic reaches connector selection.',
      useCase: 'Best for BIN, network, country, amount, metadata, or merchant policy overrides.',
      icon: BookOpen,
      route: '/routing/rules',
      state: hasRuleBasedRouting ? 'enabled' : 'not_set',
      evidence: hasRuleBasedRouting ? 'An advanced rule algorithm is active.' : 'No active advanced rule algorithm found.',
      canDeactivate: Boolean(activeRuleAlgorithm),
    },
    {
      id: 'volume',
      title: 'Volume split',
      eyebrow: 'Controlled rollout',
      description: 'Distribute payments by configured percentages and verify actual traffic share.',
      useCase: 'Best for ramp-ups, A/B routing, new connector rollout, and traffic balancing.',
      icon: PieChart,
      route: '/routing/volume',
      state: hasVolumeSplit ? 'enabled' : 'not_set',
      evidence: hasVolumeSplit ? 'A volume split algorithm is active.' : 'No active volume split algorithm found.',
      canDeactivate: Boolean(activeVolumeAlgorithm),
    },
    {
      id: 'debit',
      title: 'Debit routing',
      eyebrow: 'Network cost routing',
      description: 'Enable debit-network decisions for co-badged card payment flows.',
      useCase: 'Best when debit network cost, issuer country, and regulated-card behavior matter.',
      icon: Network,
      route: '/routing/debit',
      state: hasDebitRouting ? 'enabled' : 'not_set',
      evidence: hasDebitRouting ? 'Merchant debit-routing flag is enabled.' : 'Enable the merchant flag before running debit network decisions.',
      canDeactivate: hasDebitRouting,
    },
  ]

  const nextAction = !merchantId
    ? {
        title: 'Select or create a merchant first',
        body: 'Routing setup, audit, and explorer traffic are all merchant scoped.',
        cta: 'Create merchant',
        route: '/onboarding',
        icon: ShieldCheck,
      }
    : !hasAuthRateConfig
      ? {
          title: 'Start with auth-rate configuration',
          body: 'Score defaults make the runtime decision surface useful even before custom rules are active.',
          cta: 'Configure auth-rate',
          route: '/routing/sr',
          icon: TrendingUp,
        }
      : !hasRuntimeStrategy
        ? {
            title: 'Activate one routing strategy',
            body: 'Create a rule-based or volume-split strategy, activate it, then test it in Decision Explorer.',
            cta: 'Create rule',
            route: '/routing/rules',
            icon: GitBranch,
          }
        : {
            title: 'Test the active strategy now',
            body: 'Run real decision calls, then open audit to inspect request, response, gateway, and status.',
            cta: 'Open explorer',
            route: '/decisions',
            icon: FlaskConical,
          }

  const NextActionIcon = nextAction.icon

  return (
    <div className="mx-auto max-w-[1380px] space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-3xl font-semibold tracking-tight text-slate-950 dark:text-white">Routing Hub</h1>
            <Badge variant={merchantId ? 'blue' : 'orange'}>{merchantId || 'No merchant selected'}</Badge>
          </div>
          <p className="mt-2 max-w-3xl text-sm leading-7 text-slate-500 dark:text-[#8a8a93]">
            Decide what to configure next, test the active strategy, and jump into audit without duplicating the overview dashboard.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="secondary" onClick={() => navigate('/analytics')}>
            <BarChart3 size={16} />
            Analytics
          </Button>
          <Button variant="secondary" onClick={() => navigate('/audit')}>
            <Activity size={16} />
            Audit
          </Button>
          <Button onClick={() => navigate('/decisions')}>
            <FlaskConical size={16} />
            Test routing
          </Button>
        </div>
      </div>

      <section className="grid gap-5 xl:grid-cols-[1.2fr_0.8fr]">
        <Card>
          <CardBody className="p-6 md:p-7">
            <div className="flex h-full flex-col justify-between gap-8">
              <div className="flex flex-wrap items-start justify-between gap-4">
                <div>
                  <SurfaceLabel>Runtime posture</SurfaceLabel>
                  <h2 className="mt-4 text-4xl font-semibold tracking-[-0.04em] text-slate-950 dark:text-white">
                    {runtimeName}
                  </h2>
                  <p className="mt-3 max-w-2xl text-sm leading-7 text-slate-500 dark:text-[#9aa6bb]">
                    {runtimeDescription}
                  </p>
                </div>
                <Badge variant={hasRuntimeStrategy ? 'green' : 'gray'}>
                  {hasRuntimeStrategy ? 'Active' : loading ? 'Checking' : 'Inactive'}
                </Badge>
              </div>

              <div className="grid gap-3 sm:grid-cols-3">
                <PostureStat label="Ready surfaces" value={`${readiness}/3`} detail="Auth-rate, traffic policy, debit" />
                <PostureStat label="Runtime type" value={runtimeType} detail="Current routing surface" />
                <PostureStat label="Debit gate" value={hasDebitRouting ? 'Enabled' : 'Off'} detail="Merchant feature flag" />
              </div>

              <div className="flex flex-wrap gap-2">
                <Button disabled={!hasRuntimeStrategy} onClick={() => navigate(activeRoute)}>
                  Manage active strategy
                  <ArrowRight size={16} />
                </Button>
                <Button variant="secondary" onClick={() => navigate('/decisions')}>
                  Run decision
                </Button>
                {activeStrategyId && (
                  <Button
                    variant="danger"
                    onClick={() => deactivateStrategy(activeStrategyId)}
                    disabled={deactivatingStrategy === activeStrategyId}
                  >
                    <PowerOff size={16} />
                    {deactivatingStrategy === activeStrategyId ? 'Deactivating' : 'Deactivate'}
                  </Button>
                )}
              </div>
            </div>
          </CardBody>
        </Card>

        <Card>
          <CardBody className="p-6 md:p-7">
            <div className="flex items-start gap-4">
              <div className="rounded-2xl border border-brand-500/20 bg-brand-500/10 p-3 text-brand-600 dark:text-sky-300">
                <NextActionIcon size={22} />
              </div>
              <div>
                <SurfaceLabel>Recommended next action</SurfaceLabel>
                <h3 className="mt-3 text-2xl font-semibold text-slate-950 dark:text-white">{nextAction.title}</h3>
                <p className="mt-3 text-sm leading-7 text-slate-500 dark:text-[#9aa6bb]">{nextAction.body}</p>
                <Button className="mt-5" onClick={() => navigate(nextAction.route)}>
                  {nextAction.cta}
                  <ArrowRight size={16} />
                </Button>
              </div>
            </div>

            <div className="mt-7 space-y-3 border-t border-slate-200 pt-5 dark:border-[#242b36]">
              <RunbookStep done={hasAuthRateConfig} label="Configure" detail="Define score defaults or routing policy." />
              <RunbookStep done={hasRuntimeStrategy} label="Activate" detail="Make one strategy live for the merchant." />
              <RunbookStep done={false} label="Verify" detail="Run Explorer, then inspect Audit." />
            </div>
          </CardBody>
        </Card>
      </section>

      <section className="space-y-3">
        <div>
          <SurfaceLabel>Routing surfaces</SurfaceLabel>
          <h2 className="mt-2 text-xl font-semibold text-slate-950 dark:text-white">Pick the job, then configure the route</h2>
        </div>

        {actionError && (
          <div className="rounded-lg border border-red-500/20 bg-red-500/8 px-3 py-2 text-sm text-red-500">
            {actionError}
          </div>
        )}
        {actionMessage && (
          <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-500">
            {actionMessage}
          </div>
        )}

        <div className="overflow-hidden rounded-[30px] border border-slate-200 bg-white dark:border-[#2a303a] dark:bg-[#11151d]">
          {strategies.map((strategy, index) => {
            const Icon = strategy.icon
            return (
              <div
                key={strategy.id}
                className={`group grid gap-5 px-5 py-5 transition-colors hover:bg-slate-50 dark:hover:bg-[#151b25] lg:grid-cols-[minmax(320px,520px)_minmax(280px,1fr)] lg:items-center xl:grid-cols-[minmax(360px,560px)_minmax(320px,1fr)_max-content] ${
                  index === 0 ? '' : 'border-t border-slate-200 dark:border-[#252d3a]'
                }`}
              >
                <div className="flex items-start gap-4">
                  <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3 text-slate-500 transition-colors group-hover:border-brand-500/25 group-hover:text-brand-600 dark:border-[#273141] dark:bg-[#0c1119] dark:text-[#8ea0bb] dark:group-hover:text-sky-300">
                    <Icon size={22} />
                  </div>
                  <div className="min-w-0 max-w-[470px]">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="text-lg font-semibold text-slate-950 dark:text-white">{strategy.title}</h3>
                      {stateBadge(strategy.state)}
                    </div>
                    <p className="mt-1 text-xs font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-[#6d768a]">
                      {strategy.eyebrow}
                    </p>
                    <p className="mt-3 text-sm leading-6 text-slate-500 dark:text-[#9aa6bb]">
                      {strategy.description}
                    </p>
                  </div>
                </div>

                <div className="max-w-[520px]">
                  <p className="text-sm font-medium text-slate-700 dark:text-[#d8deea]">{strategy.useCase}</p>
                  <p className="mt-2 text-xs leading-5 text-slate-500 dark:text-[#7d879b]">{strategy.evidence}</p>
                </div>

                <div className="flex flex-wrap justify-start gap-2 lg:col-start-2 xl:col-start-auto xl:justify-end xl:whitespace-nowrap">
                  <Button size="sm" variant="secondary" onClick={() => navigate(strategy.route)}>
                    Configure
                  </Button>
                  <Button size="sm" variant="secondary" onClick={() => navigate('/decisions')}>
                    Test
                  </Button>
                  {strategy.canDeactivate && (
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => deactivateStrategy(strategy.id)}
                      disabled={deactivatingStrategy === strategy.id}
                    >
                      <PowerOff size={14} />
                      {deactivatingStrategy === strategy.id ? 'Deactivating' : 'Deactivate'}
                    </Button>
                  )}
                </div>
              </div>
            )
          })}
        </div>
      </section>
    </div>
  )
}

function PostureStat({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="rounded-[22px] border border-slate-200 bg-slate-50 px-4 py-4 dark:border-[#273141] dark:bg-[#0c1119]">
      <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-[#6d768a]">{label}</p>
      <p className="mt-3 text-2xl font-semibold text-slate-950 dark:text-white">{value}</p>
      <p className="mt-1 text-xs text-slate-500 dark:text-[#7d879b]">{detail}</p>
    </div>
  )
}

function RunbookStep({ done, label, detail }: { done: boolean; label: string; detail: string }) {
  return (
    <div className="flex items-start gap-3">
      <div className={`mt-0.5 rounded-full ${done ? 'text-emerald-500' : 'text-slate-400 dark:text-[#6d768a]'}`}>
        <CheckCircle2 size={17} />
      </div>
      <div>
        <p className="text-sm font-semibold text-slate-900 dark:text-white">{label}</p>
        <p className="text-xs leading-5 text-slate-500 dark:text-[#7d879b]">{detail}</p>
      </div>
    </div>
  )
}
