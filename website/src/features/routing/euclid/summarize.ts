import { RoutingAlgorithm, EuclidAlgorithmData } from '../../../types/api'
import { describeRuleConditions } from './describe'

type PriorityGateway = { gateway_name: string; gateway_id: string | null }
type VolumeSplit = { split: number; output: { gateway_name: string } }
type VolumeSplitPriority = { split: number; output: PriorityGateway[] }

export type NormalizedRuleOutput = {
  priorityGateways: PriorityGateway[]
  volumeSplits: VolumeSplit[]
  volumeSplitPriorityEntries: VolumeSplitPriority[]
  isVolumeSplit: boolean
  isVolumeSplitPriority: boolean
}

/**
 * The backend returns a rule's output in one of four shapes — `{ priority: [...] }`,
 * `{ type: 'volume_split', data: [...] }`, `{ volume_split: [...] }`, or
 * `{ volume_split_priority: [...] }`. Every consumer needs the same flattening, so it lives here
 * once and both the breakdown panel and the list summaries read from it.
 */
export function normalizeRuleOutput(rule: {
  output?: unknown
  routing_type?: string
}): NormalizedRuleOutput {
  const output = rule.output as Record<string, unknown> | undefined
  const rawPriority = output?.priority ?? output?.data
  const rawVolume = output?.volume_split ?? (output?.type === 'volume_split' ? output?.data : undefined)
  const rawVolumeSplitPriority = output?.volume_split_priority

  const priorityGateways = (Array.isArray(rawPriority) ? rawPriority : []) as PriorityGateway[]
  const volumeSplits = (Array.isArray(rawVolume) ? rawVolume : []) as VolumeSplit[]
  const volumeSplitPriorityEntries = (
    Array.isArray(rawVolumeSplitPriority) ? rawVolumeSplitPriority : []
  ) as VolumeSplitPriority[]

  return {
    priorityGateways,
    volumeSplits,
    volumeSplitPriorityEntries,
    isVolumeSplit: rule.routing_type === 'volume_split' || volumeSplits.length > 0,
    isVolumeSplitPriority:
      rule.routing_type === 'volume_split_priority' || volumeSplitPriorityEntries.length > 0,
  }
}

function algorithmData(algo: RoutingAlgorithm): EuclidAlgorithmData | undefined {
  const algorithm = algo.algorithm_data || algo.algorithm
  return algorithm?.data as EuclidAlgorithmData | undefined
}

/**
 * One-line summary of a rule's conditions for the list table. Delegates to the shared formatter so
 * the row, the expanded breakdown and the builder all describe a rule the same way.
 */
export function summarizeConditions(algo: RoutingAlgorithm): string {
  const rules = algorithmData(algo)?.rules ?? []
  if (rules.length === 0) return 'No conditions — always matches'

  const text = describeRuleConditions(rules[0])
  return rules.length > 1 ? `${text} (+${rules.length - 1} more)` : text
}

/**
 * Where a rule sends traffic — "zift (Priority 100%)" or "Stripe (50%) / Adyen (50%)" — plus the
 * default fallback when one is configured.
 */
export function summarizeDestination(algo: RoutingAlgorithm): string {
  const data = algorithmData(algo)
  const first = data?.rules?.[0]

  let primary = ''
  if (first) {
    const out = normalizeRuleOutput(first)
    if (out.isVolumeSplitPriority) {
      primary = out.volumeSplitPriorityEntries
        .map((e) => `${e.output?.[0]?.gateway_name ?? '?'} (${e.split}%)`)
        .join(' / ')
    } else if (out.isVolumeSplit) {
      primary = out.volumeSplits
        .map((e) => `${e.output?.gateway_name ?? '?'} (${e.split}%)`)
        .join(' / ')
    } else if (out.priorityGateways.length > 0) {
      primary = `${out.priorityGateways[0].gateway_name} (Priority 100%)`
    }
  }

  const defaultSel = data?.default_selection || data?.defaultSelection
  const fallbackRaw = (defaultSel as { priority?: unknown; data?: unknown } | undefined)
  const fallbackArr = (Array.isArray(fallbackRaw?.priority)
    ? fallbackRaw!.priority
    : Array.isArray(fallbackRaw?.data)
    ? fallbackRaw!.data
    : []) as PriorityGateway[]
  const fallback = fallbackArr[0]?.gateway_name

  if (!primary) return fallback ? `${fallback} (fallback only)` : 'No destination configured'
  return fallback ? `${primary} — fallback ${fallback}` : primary
}

/** Every distinct destination gateway a rule mentions — powers the list's Gateway filter. */
export function destinationGateways(algo: RoutingAlgorithm): string[] {
  const data = algorithmData(algo)
  const names = new Set<string>()

  ;(data?.rules ?? []).forEach((rule) => {
    const out = normalizeRuleOutput(rule)
    out.priorityGateways.forEach((g) => g.gateway_name && names.add(g.gateway_name))
    out.volumeSplits.forEach((e) => e.output?.gateway_name && names.add(e.output.gateway_name))
    out.volumeSplitPriorityEntries.forEach((e) =>
      (e.output ?? []).forEach((g) => g.gateway_name && names.add(g.gateway_name))
    )
  })

  const defaultSel = data?.default_selection || data?.defaultSelection
  const fallbackRaw = defaultSel as { priority?: unknown; data?: unknown } | undefined
  const fallbackArr = (Array.isArray(fallbackRaw?.priority)
    ? fallbackRaw!.priority
    : Array.isArray(fallbackRaw?.data)
    ? fallbackRaw!.data
    : []) as PriorityGateway[]
  fallbackArr.forEach((g) => g.gateway_name && names.add(g.gateway_name))

  return Array.from(names)
}
