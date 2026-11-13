import { RoutingAlgorithm, EuclidAlgorithmData } from '../../../types/api'
import { RoutingKeyConfig } from '../../../hooks/useDynamicRoutingConfig'
import type {
  RuleBlock, StatementGroup, ConditionRow, GatewayEntry,
} from '../../../components/ui/RuleCodeEditor'

export type DefaultOutput = {
  priorityGateways: GatewayEntry[]
}

export const OPERATOR_TO_API: Record<string, string> = {
  '==': 'equal',
  '!=': 'not_equal',
  '>': 'greater_than',
  '<': 'less_than',
  '>=': 'greater_than_equal',
  '<=': 'less_than_equal',
  'in': 'equal',
  'not_in': 'not_equal',
}

export const OPERATOR_LABELS: Record<string, string> = {
  '==': 'equals',
  '!=': 'not equals',
  '>': 'greater than',
  '<': 'less than',
  '>=': 'greater than or equal',
  '<=': 'less than or equal',
  'in': 'is one of',
  'not_in': 'is not one of',
}


export function toLabel(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
}



export function createCondition(routingKeys: Record<string, RoutingKeyConfig>): ConditionRow {
  const firstKey = Object.keys(routingKeys)[0] || 'payment_method'
  const firstKeyValues = routingKeys[firstKey]?.values || []
  return {
    id: crypto.randomUUID(),
    lhs: firstKey,
    operator: '==',
    value: firstKeyValues[0] || '',
    metadataKey: '',
  }
}

export function createStatementGroup(routingKeys: Record<string, RoutingKeyConfig>): StatementGroup {
  return {
    id: crypto.randomUUID(),
    conditions: [createCondition(routingKeys)],
    nested: [],
  }
}

export function formatOp(comparison: string): string {
  const map: Record<string, string> = {
    equal: '=', not_equal: '≠',
    greater_than: '>', less_than: '<',
    greater_than_equal: '≥', less_than_equal: '≤',
  }
  return map[comparison] ?? comparison.replace(/_/g, ' ')
}


// ---- Build Euclid payload ----
export function buildAlgorithmData(rules: RuleBlock[], defaultOutput: DefaultOutput, routingKeys: Record<string, RoutingKeyConfig>) {
  function buildPriorityOutput(gateways: GatewayEntry[]): Record<string, unknown> {
    return {
      priority: gateways.map((g) => ({ gateway_name: g.gatewayName, gateway_id: g.gatewayId || null })),
    }
  }

  function buildOutput(block: RuleBlock): Record<string, unknown> {
    if (block.outputType === 'volume_split') {
      return {
        volume_split: block.volumeSplitEntries.map((e) => ({
          split: e.split,
          output: { gateway_name: e.gatewayName, gateway_id: e.gatewayId || null },
        })),
      }
    }
    if (block.outputType === 'volume_split_priority') {
      return {
        volume_split_priority: block.volumeSplitPriorityEntries.map((e) => ({
          split: e.split,
          output: e.gateways.map((g) => ({ gateway_name: g.gatewayName, gateway_id: g.gatewayId || null })),
        })),
      }
    }
    return buildPriorityOutput(block.priorityGateways)
  }

  function buildCondition(c: ConditionRow) {
    const keyType = routingKeys[c.lhs]?.type
    const isMulti = c.operator === 'in' || c.operator === 'not_in'

    if (isMulti && Array.isArray(c.value)) {
      return {
        lhs: c.lhs,
        comparison: OPERATOR_TO_API[c.operator],
        value: { type: 'enum_variant_array', value: c.value },
        metadata: {},
      }
    }

    if (keyType === 'udf' || keyType === 'global_ref') {
      return {
        lhs: c.lhs,
        comparison: OPERATOR_TO_API[c.operator] || c.operator,
        value: {
          type: 'metadata_variant',
          value: { key: c.metadataKey || c.lhs, value: c.value },
        },
        metadata: {},
      }
    }

    const apiValueType =
      keyType === 'integer' ? 'number' :
      keyType === 'str_value' ? 'str_value' :
      'enum_variant'
    return {
      lhs: c.lhs,
      comparison: OPERATOR_TO_API[c.operator] || c.operator,
      value: {
        type: apiValueType,
        value: keyType === 'integer' ? Number(c.value) : c.value,
      },
      metadata: {},
    }
  }

  function buildStatement(group: StatementGroup): Record<string, unknown> {
    const statement: Record<string, unknown> = {
      condition: group.conditions.map(buildCondition),
      nested: group.nested.length > 0 ? group.nested.map(buildStatement) : null,
    }
    return statement
  }

  return {
    globals: {},
    default_selection: buildPriorityOutput(defaultOutput.priorityGateways),
    rules: rules.map((r) => ({
      name: r.name,
      routing_type: r.outputType,
      output: buildOutput(r),
      statements: r.statements.map(buildStatement),
    })),
  }
}

// ---- Reverse-parse API → RuleBlocks ----
const API_OPERATOR_TO_UI: Record<string, string> = {
  equal: '==', not_equal: '!=',
  greater_than: '>', less_than: '<',
  greater_than_equal: '>=', less_than_equal: '<=',
}

export function parseAlgorithmToRuleBlocks(algo: RoutingAlgorithm): { ruleBlocks: RuleBlock[]; defaultOutput: DefaultOutput } {
  const algorithm = algo.algorithm_data || algo.algorithm
  const data = algorithm?.data as EuclidAlgorithmData | undefined
  if (!data) return { ruleBlocks: [], defaultOutput: { priorityGateways: [] } }

  function parseGateways(arr: { gateway_name: string; gateway_id?: string | null }[]): GatewayEntry[] {
    return arr.map((g) => ({ id: crypto.randomUUID(), gatewayName: g.gateway_name, gatewayId: g.gateway_id ?? '' }))
  }

  function parseCondition(cond: { lhs: string; comparison: string; value: { type: string; value: unknown } }): ConditionRow {
    const isArray = cond.value?.type === 'enum_variant_array'
    const isMetadataVariant = cond.value?.type === 'metadata_variant'
    let operator = API_OPERATOR_TO_UI[cond.comparison] ?? cond.comparison
    if (isArray && cond.comparison === 'equal') operator = 'in'
    if (isArray && cond.comparison === 'not_equal') operator = 'not_in'

    if (isMetadataVariant) {
      const metaVal = cond.value?.value as { key?: string; value?: unknown } | undefined
      return {
        id: crypto.randomUUID(),
        lhs: String(cond.lhs),
        operator,
        value: String(metaVal?.value ?? ''),
        metadataKey: String(metaVal?.key ?? cond.lhs),
      }
    }

    const value = isArray
      ? (Array.isArray(cond.value?.value) ? (cond.value.value as string[]) : [String(cond.value?.value)])
      : String(cond.value?.value ?? '')
    return { id: crypto.randomUUID(), lhs: String(cond.lhs), operator, value, metadataKey: '' }
  }

  function parseStatement(stmt: { condition: Parameters<typeof parseCondition>[0][]; nested?: typeof stmt[] }): StatementGroup {
    return {
      id: crypto.randomUUID(),
      conditions: (stmt.condition ?? []).map(parseCondition),
      nested: (stmt.nested ?? []).map(parseStatement),
    }
  }

  const ruleBlocks: RuleBlock[] = (data.rules ?? []).map((rule) => {
    const output = rule.output as Record<string, unknown> | undefined
    const outputType: RuleBlock['outputType'] = rule.routing_type ?? 'priority'
    const rawPriority = output?.priority ?? output?.data
    const rawVolume = output?.volume_split ?? (output?.type === 'volume_split' ? output?.data : undefined)
    const rawVolumeSplitPriority = output?.volume_split_priority

    return {
      id: crypto.randomUUID(),
      name: rule.name || 'Cloned Rule',
      statements: (rule.statements ?? []).map(parseStatement),
      outputType,
      priorityGateways: outputType === 'priority'
        ? parseGateways((Array.isArray(rawPriority) ? rawPriority : []) as { gateway_name: string; gateway_id?: string | null }[])
        : [],
      volumeSplitEntries: outputType === 'volume_split'
        ? (Array.isArray(rawVolume) ? rawVolume : []).map((e: { split: number; output: { gateway_name: string; gateway_id?: string | null } }) => ({
            id: crypto.randomUUID(), split: e.split, gatewayName: e.output.gateway_name, gatewayId: e.output.gateway_id ?? '',
          }))
        : [],
      volumeSplitPriorityEntries: outputType === 'volume_split_priority'
        ? (Array.isArray(rawVolumeSplitPriority) ? rawVolumeSplitPriority : []).map((e: { split: number; output: { gateway_name: string; gateway_id?: string | null }[] }) => ({
            id: crypto.randomUUID(), split: e.split, gateways: parseGateways(e.output),
          }))
        : [],
    }
  })

  const defRaw = (data.default_selection || data.defaultSelection) as Record<string, unknown> | undefined
  const defArr = (Array.isArray(defRaw?.priority) ? defRaw!.priority : Array.isArray(defRaw?.data) ? defRaw!.data : []) as { gateway_name: string; gateway_id?: string | null }[]

  return { ruleBlocks, defaultOutput: { priorityGateways: parseGateways(defArr) } }
}
