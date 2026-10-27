import { RoutingAlgorithm } from '../../types/api'
import { toVolumeSplitRuleDetailsState } from '../../features/routing/volumeSplit/state'
import { SplitBreakdown } from '../routing/volumeSplit/SplitBreakdown'
import { RoutingRuleListConfig, RoutingRuleListPage, algorithmType } from '../routing/RoutingRuleListPage'
import { Notice } from '../ui/Notice'

/** "stripe 50% / adyen 50%" — the row's one-line view of where traffic goes. */
function describeSplit(algo: RoutingAlgorithm): string {
  const gateways = toVolumeSplitRuleDetailsState(algo)?.gateways ?? []
  const named = gateways.filter((g) => g.gatewayName.trim())
  if (named.length === 0) return 'No gateways configured'
  return named.map((g) => `${g.gatewayName} ${g.split}%`).join(' / ')
}

function splitGateways(algo: RoutingAlgorithm): string[] {
  return (toVolumeSplitRuleDetailsState(algo)?.gateways ?? [])
    .map((g) => g.gatewayName.trim())
    .filter(Boolean)
}

const activeRuleBased = (active: RoutingAlgorithm[]) =>
  active.find((r) => {
    const t = algorithmType(r)
    return t && t !== 'volume_split'
  })

const config: RoutingRuleListConfig = {
  title: 'Volume Split Routing',
  description: 'Divide payment traffic across gateways by fixed percentages, independent of transaction attributes.',
  cellAlign: 'middle',
  basePath: '/routing/volume',
  emptyText: 'No volume split rules yet.',
  includes: (algo) => algorithmType(algo) === 'volume_split',
  gateways: splitGateways,
  columns: [
    {
      header: 'Distribution',
      gatewayFilter: true,
      cellClassName: 'max-w-[320px]',
      render: (algo) => {
        const distribution = describeSplit(algo)
        return (
          <p className="truncate text-sm font-medium text-slate-800 dark:text-slate-200" title={distribution}>
            {distribution}
          </p>
        )
      },
    },
  ],
  breakdown: (algo) => <SplitBreakdown gateways={toVolumeSplitRuleDetailsState(algo)?.gateways ?? []} />,
  replaces: activeRuleBased,
  switchDialog: {
    title: 'Switch to Volume Split Routing?',
    description: (replaced) => `"${replaced.name}" is currently active. Activating this rule will replace it.`,
  },
  messages: { activated: 'Rule activated.', deactivated: 'Rule deactivated.' },
  notices: (active) => {
    const replaced = activeRuleBased(active)
    return replaced ? (
      <Notice tone="warning">
        <strong>“{replaced.name}” is active</strong> — activating a volume split rule will
        automatically deactivate it.
      </Notice>
    ) : null
  },
}

export function VolumeSplitPage() {
  return <RoutingRuleListPage config={config} />
}
