import {
  ABTestAlgorithmData,
  EliminationData,
  EuclidAlgorithmData,
  GatewayConnector,
  RoutingAlgorithm,
  SRConfigData,
  VolumeSplitItem,
} from '../../../types/api'
import { normalizeRuleOutput } from '../euclid/summarize'

export type SlotKind = 'rule' | 'volume' | 'ab' | 'none'

/** Everything the flow page derives from the merchant's saved configuration. */
export interface StackState {
  slot: SlotKind
  slotAlgorithm: RoutingAlgorithm | null
  srConfigured: boolean
  autopilotOn: boolean
  srData: SRConfigData | null
  eliminationConfigured: boolean
  eliminationData: EliminationData | null
  costOn: boolean
  volumeCommitmentOn: boolean
  debitOn: boolean
  abRealPaymentsOn: boolean
}

export interface LaneDef {
  name: string
  color: string
  /** Traffic share (0..1) when the active strategy is a volume split; drives lane thickness. */
  share?: number
}

function algorithmType(algorithm?: RoutingAlgorithm | null): string {
  return (algorithm?.algorithm_data || algorithm?.algorithm)?.type || ''
}

function algorithmData(algorithm?: RoutingAlgorithm | null): unknown {
  return (algorithm?.algorithm_data || algorithm?.algorithm)?.data
}

export function isRuleBasedAlgorithmType(type: string) {
  return type === 'advanced' || type === 'priority' || type === 'single'
}

export type StackFeature =
  | 'autopilot'
  | 'elimination'
  | 'multi-objective-routing'
  | 'volume-contracts'
  | 'ab-test-real-payments'

export function deriveStack(input: {
  activeAlgorithms: RoutingAlgorithm[] | undefined
  srData: SRConfigData | null
  eliminationData: EliminationData | null
  isFeatureEnabled: (feature: StackFeature) => boolean
  debitEnabled: boolean
}): StackState {
  const paymentAlgorithms = (input.activeAlgorithms || []).filter(
    (a) => (a.algorithm_for || 'payment') === 'payment',
  )
  const ruleAlgorithm = paymentAlgorithms.find((a) => isRuleBasedAlgorithmType(algorithmType(a)))
  const volumeAlgorithm = paymentAlgorithms.find((a) => algorithmType(a) === 'volume_split')
  const abAlgorithm = paymentAlgorithms.find((a) => algorithmType(a) === 'ab_test')

  // The payment slot holds one algorithm; when several rows exist the experiment wins the
  // billing as "the live strategy" because the interceptor consults it first.
  const slotAlgorithm = abAlgorithm ?? ruleAlgorithm ?? volumeAlgorithm ?? null
  const slot: SlotKind = abAlgorithm ? 'ab' : ruleAlgorithm ? 'rule' : volumeAlgorithm ? 'volume' : 'none'

  return {
    slot,
    slotAlgorithm,
    srConfigured: Boolean(input.srData) || input.isFeatureEnabled('autopilot'),
    autopilotOn: input.isFeatureEnabled('autopilot'),
    srData: input.srData,
    eliminationConfigured: Boolean(input.eliminationData) || input.isFeatureEnabled('elimination'),
    eliminationData: input.eliminationData,
    costOn: input.isFeatureEnabled('multi-objective-routing'),
    volumeCommitmentOn: input.isFeatureEnabled('volume-contracts'),
    debitOn: input.debitEnabled,
    abRealPaymentsOn: input.isFeatureEnabled('ab-test-real-payments'),
  }
}

/** Fixed hue rotation so a connector keeps its color as config changes reorder the list. */
const LANE_PALETTE = ['#60a5fa', '#2dd4bf', '#c084fc', '#fb923c', '#f472b6', '#facc15', '#4ade80', '#38bdf8']

function laneColor(index: number) {
  return LANE_PALETTE[index % LANE_PALETTE.length]
}

export const MAX_LANES = 6

export interface LaneModel {
  lanes: LaneDef[]
  /** Connectors referenced by the strategy beyond MAX_LANES — shown as a count, not drawn. */
  overflow: number
  /** True when the strategy names no connectors (empty slot / experiment) and the lanes are illustrative. */
  ghost: boolean
  /** Set when the configured strategy fully determines the first choice (single / priority head). */
  deterministicHead: string | null
}

/**
 * When no strategy names connectors we can't know the merchant's real set — but a blank grey
 * diagram teaches nothing. Show a plausible example set instead, in full color, dashed and
 * explicitly labeled as an example by the canvas.
 */
const EXAMPLE_LANES: LaneDef[] = [
  { name: 'razorpay', color: LANE_PALETTE[0] },
  { name: 'payu', color: LANE_PALETTE[1] },
  { name: 'stripe', color: LANE_PALETTE[2] },
]

/**
 * The lanes are the connectors the active strategy can route to, in the order the strategy
 * declares them. Which connectors exist for a merchant has no read API (the gateway-account
 * table is seeded out of band), so the strategy's own output list is the only truthful source.
 */
export function deriveLanes(stack: StackState): LaneModel {
  const seen: string[] = []
  const shares = new Map<string, number>()
  const push = (connector?: { gateway_name?: string } | null) => {
    const name = connector?.gateway_name
    if (name && !seen.includes(name)) seen.push(name)
  }

  const type = algorithmType(stack.slotAlgorithm)
  const data = algorithmData(stack.slotAlgorithm)

  if (type === 'single') {
    push(data as GatewayConnector)
  } else if (type === 'priority') {
    ;((data as GatewayConnector[]) || []).forEach(push)
  } else if (type === 'volume_split') {
    const splits = (data as VolumeSplitItem[]) || []
    const total = splits.reduce((sum, s) => sum + (s.split || 0), 0)
    splits.forEach((s) => {
      push(s.output)
      if (s.output?.gateway_name && total > 0) {
        shares.set(s.output.gateway_name, (shares.get(s.output.gateway_name) || 0) + s.split / total)
      }
    })
  } else if (type === 'advanced') {
    const euclid = data as EuclidAlgorithmData | undefined
    for (const rule of euclid?.rules || []) {
      const output = normalizeRuleOutput(rule)
      output.priorityGateways.forEach(push)
      output.volumeSplits.forEach((s) => push(s.output))
      output.volumeSplitPriorityEntries.forEach((entry) => entry.output.forEach(push))
    }
    // default_selection is the externally-tagged Output enum on the wire: {"priority": [...]},
    // {"volume_split": [...]}, {"volume_split_priority": [...]} or {"single": {...}} — never the
    // {type, data} envelope the stale EuclidOutput type suggests. Mirror summarize.ts and keep
    // `.data` as a defensive legacy fallback.
    const fallback = (euclid?.default_selection ?? euclid?.defaultSelection) as
      | Record<string, unknown>
      | undefined
    if (fallback?.single) push(fallback.single as GatewayConnector)
    const fallbackItems = [fallback?.priority, fallback?.volume_split, fallback?.volume_split_priority, fallback?.data]
      .find(Array.isArray) as Array<GatewayConnector | VolumeSplitItem | { output?: GatewayConnector[] }> | undefined
    for (const item of fallbackItems || []) {
      const output = (item as { output?: GatewayConnector | GatewayConnector[] }).output
      if (Array.isArray(output)) output.forEach(push)
      else if (output) push(output)
      else push(item as GatewayConnector)
    }
  }

  if (seen.length === 0) {
    return { lanes: EXAMPLE_LANES, overflow: 0, ghost: true, deterministicHead: null }
  }

  const lanes = seen.slice(0, MAX_LANES).map((name, i) => ({
    name,
    color: laneColor(i),
    share: shares.get(name),
  }))
  const deterministicHead =
    type === 'single' || type === 'priority' ? seen[0] : null
  return { lanes, overflow: Math.max(0, seen.length - MAX_LANES), ghost: false, deterministicHead }
}

export function slotAlgorithmSummary(stack: StackState): { kicker: string; name: string } | null {
  if (!stack.slotAlgorithm) return null
  const kicker = {
    ab: 'A/B test',
    rule: 'Rule-based',
    volume: 'Volume split',
    none: '',
  }[stack.slot]
  return { kicker, name: stack.slotAlgorithm.name || 'Unnamed strategy' }
}

export function abTestData(stack: StackState): ABTestAlgorithmData | null {
  if (stack.slot !== 'ab') return null
  return (algorithmData(stack.slotAlgorithm) as ABTestAlgorithmData) ?? null
}

export function euclidData(stack: StackState): EuclidAlgorithmData | null {
  if (algorithmType(stack.slotAlgorithm) !== 'advanced') return null
  return (algorithmData(stack.slotAlgorithm) as EuclidAlgorithmData) ?? null
}

export function volumeSplits(stack: StackState): VolumeSplitItem[] {
  if (algorithmType(stack.slotAlgorithm) !== 'volume_split') return []
  return (algorithmData(stack.slotAlgorithm) as VolumeSplitItem[]) ?? []
}

export function priorityList(stack: StackState): GatewayConnector[] {
  const type = algorithmType(stack.slotAlgorithm)
  const data = algorithmData(stack.slotAlgorithm)
  if (type === 'priority') return (data as GatewayConnector[]) || []
  if (type === 'single' && data) return [data as GatewayConnector]
  return []
}
