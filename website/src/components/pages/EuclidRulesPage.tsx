import { ABTestAlgorithmData, RoutingAlgorithm } from '../../types/api'
import { resolvedArm } from '../../features/routing/abTesting/arms'
import { RuleBreakdown } from '../routing/euclid/RuleBreakdown'
import {
  summarizeConditions, summarizeDestination, destinationGateways,
} from '../../features/routing/euclid/summarize'
import { RoutingRuleListConfig, RoutingRuleListPage, algorithmType } from '../routing/RoutingRuleListPage'
import { Notice } from '../ui/Notice'

const NOT_RULE_BASED = new Set(['volume_split', 'ab_test', 'volume_contract'])

const activeVolumeSplit = (active: RoutingAlgorithm[]) =>
  active.find((a) => algorithmType(a) === 'volume_split')
const activeAbTest = (active: RoutingAlgorithm[]) =>
  active.find((a) => algorithmType(a) === 'ab_test')

/** The rule ids an active A/B experiment routes through. */
function abTestArmIds(active: RoutingAlgorithm[]): Set<string> {
  const abTest = activeAbTest(active)
  const d = abTest && ((abTest.algorithm_data || abTest.algorithm)?.data as ABTestAlgorithmData | undefined)
  if (!d) return new Set()
  return new Set(
    [resolvedArm(d, 'control').rule_algorithm_id, resolvedArm(d, 'variant').rule_algorithm_id].filter(Boolean) as string[],
  )
}

const config: RoutingRuleListConfig = {
  title: 'Rule-Based Routing',
  description: 'Create conditions to route card and alternative payment transactions dynamically across multiple active gateways.',
  cellAlign: 'top',
  basePath: '/routing/rules',
  emptyText: 'No rule-based rules yet.',
  includes: (algo) => !NOT_RULE_BASED.has(algorithmType(algo) ?? ''),
  gateways: destinationGateways,
  columns: [
    {
      header: 'Conditions Overview',
      cellClassName: 'max-w-[420px]',
      render: (algo) => {
        const conditions = summarizeConditions(algo)
        return (
          <p className="break-words font-mono text-xs leading-[18px] text-slate-600 dark:text-[#8d96a8]" title={conditions}>
            {conditions}
          </p>
        )
      },
    },
    {
      header: 'Destination Gateway',
      gatewayFilter: true,
      cellClassName: 'max-w-[300px]',
      render: (algo) => {
        const destination = summarizeDestination(algo)
        return (
          <p className="break-words text-sm font-medium leading-5 text-slate-800 dark:text-slate-200" title={destination}>
            {destination}
          </p>
        )
      },
    },
  ],
  breakdown: (algo) => <RuleBreakdown algo={algo} />,
  replaces: activeVolumeSplit,
  switchDialog: {
    title: 'Switch to Rule-Based Routing?',
    description: (replaced) => `"${replaced.name}" (Volume Split) is currently active. Activating this rule will replace it.`,
  },
  messages: { activated: 'Rule activated successfully.', deactivated: 'Rule deactivated successfully.' },
  markRow: (active) => {
    const arms = abTestArmIds(active)
    return (algo) =>
      arms.has(algo.id)
        ? { rowClassName: 'bg-purple-50/40 dark:bg-purple-900/10', label: 'In A/B test' }
        : undefined
  },
  notices: (active) => {
    const volumeSplit = activeVolumeSplit(active)
    const abTest = activeAbTest(active)
    return (
      <>
        {volumeSplit && (
          <Notice tone="warning">
            <strong>Volume Split is active</strong> — activating a rule-based rule will automatically deactivate it.
          </Notice>
        )}
        {abTest && (
          <Notice tone="info">
            <strong>A/B experiment "{abTest.name}" is active</strong> — rules marked "In A/B test" are its arms. Payments the experiment covers are routed by its arms; all other payments use the active rule. Activating a different rule doesn't stop the experiment.
          </Notice>
        )}
      </>
    )
  },
}

export function EuclidRulesPage() {
  return <RoutingRuleListPage config={config} />
}
