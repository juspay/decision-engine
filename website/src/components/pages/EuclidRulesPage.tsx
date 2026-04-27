import { useState } from 'react'
import useSWR from 'swr'
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
} from '@dnd-kit/core'
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { useDynamicRoutingConfig, RoutingKeyConfig } from '../../hooks/useDynamicRoutingConfig'
import { Plus, Trash2, GripVertical, ChevronDown, ChevronUp, Eye, PowerOff, CornerDownRight } from 'lucide-react'

const OPERATOR_TO_API: Record<string, string> = {
  '==': 'equal',
  '!=': 'not_equal',
  '>': 'greater_than',
  '<': 'less_than',
  '>=': 'greater_than_equal',
  '<=': 'less_than_equal',
}

const OPERATOR_LABELS: Record<string, string> = {
  '==': 'equals',
  '!=': 'not equals',
  '>': 'greater than',
  '<': 'less than',
  '>=': 'greater than or equal',
  '<=': 'less than or equal',
}

// ---- Types for builder ----
interface GatewayEntry {
  id: string
  gatewayName: string
  gatewayId: string
}

interface ConditionRow {
  id: string
  lhs: string
  operator: string
  value: string
}

interface StatementGroup {
  id: string
  conditions: ConditionRow[]
  nested: StatementGroup[]
}

interface RuleBlock {
  id: string
  name: string
  statements: StatementGroup[]
  priorityGateways: GatewayEntry[]
}

type DefaultOutput = {
  priorityGateways: GatewayEntry[]
}

function createCondition(routingKeys: Record<string, RoutingKeyConfig>): ConditionRow {
  const firstKey = Object.keys(routingKeys)[0] || 'payment_method'
  const firstKeyValues = routingKeys[firstKey]?.values || []

  return {
    id: crypto.randomUUID(),
    lhs: firstKey,
    operator: '==',
    value: firstKeyValues[0] || '',
  }
}

function createStatementGroup(routingKeys: Record<string, RoutingKeyConfig>): StatementGroup {
  return {
    id: crypto.randomUUID(),
    conditions: [createCondition(routingKeys)],
    nested: [],
  }
}

// ---- Sortable gateway item ----
function SortableGatewayItem({
  id,
  name,
  onRemove,
}: {
  id: string
  name: string
  onRemove: () => void
}) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id })
  const style = { transform: CSS.Transform.toString(transform), transition }
  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex items-center gap-2 bg-slate-100 dark:bg-[#111118] border border-slate-200 dark:border-[#1c1c24] rounded-lg px-2 py-1.5"
    >
      <span {...attributes} {...listeners} className="cursor-grab text-slate-400">
        <GripVertical size={14} />
      </span>
      <span className="text-sm flex-1 font-mono">{name}</span>
      <button type="button" onClick={onRemove} className="text-red-400 hover:text-red-600">
        <Trash2 size={12} />
      </button>
    </div>
  )
}

// ---- Priority output editor ----
function PriorityEditor({
  gateways,
  onChange,
}: {
  gateways: GatewayEntry[]
  onChange: (gws: GatewayEntry[]) => void
}) {
  const [newGatewayName, setNewGatewayName] = useState('')
  const [newGatewayId, setNewGatewayId] = useState('')
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  )

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event
    if (over && active.id !== over.id) {
      const oldIndex = gateways.findIndex((g) => g.id === active.id)
      const newIndex = gateways.findIndex((g) => g.id === over.id)
      onChange(arrayMove(gateways, oldIndex, newIndex))
    }
  }

  function add() {
    if (!newGatewayName.trim()) return
    onChange([
      ...gateways,
      {
        id: crypto.randomUUID(),
        gatewayName: newGatewayName.trim(),
        gatewayId: newGatewayId.trim(),
      },
    ])
    setNewGatewayName('')
    setNewGatewayId('')
  }

  return (
    <div className="space-y-2">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={gateways.map((g) => g.id)} strategy={verticalListSortingStrategy}>
          {gateways.map((gw, idx) => (
            <SortableGatewayItem
              key={gw.id}
              id={gw.id}
              name={`${idx + 1}. ${gw.gatewayName}${gw.gatewayId ? ` (${gw.gatewayId})` : ''}`}
              onRemove={() => onChange(gateways.filter((g) => g.id !== gw.id))}
            />
          ))}
        </SortableContext>
      </DndContext>
      <div className="flex gap-2">
        <input
          value={newGatewayName}
          onChange={(e) => setNewGatewayName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="gateway_name"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <input
          value={newGatewayId}
          onChange={(e) => setNewGatewayId(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="gateway_id"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <Button type="button" size="sm" variant="secondary" onClick={add}>
          <Plus size={13} /> Add
        </Button>
      </div>
    </div>
  )
}

// ---- Condition row ----
function ConditionRowEditor({
  row,
  onChange,
  onRemove,
  routingKeys,
}: {
  row: ConditionRow
  onChange: (r: ConditionRow) => void
  onRemove: () => void
  routingKeys: Record<string, RoutingKeyConfig>
}) {
  const keyInfo = routingKeys[row.lhs]
  const isEnum = keyInfo?.type === 'enum'
  const isInt = keyInfo?.type === 'integer'

  const operators = isInt
    ? ['>', '<', '>=', '<=', '==', '!=']
    : ['==', '!=']

  return (
    <div className="flex items-center gap-2 flex-wrap">
      <select
        value={row.lhs}
        onChange={(e) => onChange({ ...row, lhs: e.target.value, value: '', operator: '==' })}
        className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs focus:outline-none"
      >
        {Object.keys(routingKeys).map((k) => (
          <option key={k} value={k}>
            {k}
          </option>
        ))}
      </select>
      <select
        value={row.operator}
        onChange={(e) => onChange({ ...row, operator: e.target.value })}
        aria-label="Condition operator"
        className="min-w-[9.5rem] border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2.5 py-1 text-xs focus:outline-none"
      >
        {operators.map((op) => (
          <option key={op} value={op}>
            {OPERATOR_LABELS[op] || op}
          </option>
        ))}
      </select>
      {isEnum ? (
        <select
          value={row.value}
          onChange={(e) => onChange({ ...row, value: e.target.value })}
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs focus:outline-none"
        >
          <option value="">select...</option>
          {(routingKeys[row.lhs]?.values || []).map((v: string) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      ) : (
        <input
          type="number"
          value={row.value}
          onChange={(e) => onChange({ ...row, value: e.target.value })}
          placeholder="value"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs w-24 focus:outline-none"
        />
      )}
      <button type="button" onClick={onRemove} className="text-red-400 hover:text-red-600">
        <Trash2 size={12} />
      </button>
    </div>
  )
}

// ---- Statement group ----
function StatementGroupEditor({
  group,
  onChange,
  onRemove,
  routingKeys,
  depth = 0,
}: {
  group: StatementGroup
  onChange: (g: StatementGroup) => void
  onRemove: () => void
  routingKeys: Record<string, RoutingKeyConfig>
  depth?: number
}) {
  const canRemove = depth > 0 || group.conditions.length > 1 || group.nested.length > 0
  const label = depth === 0 ? 'IF group' : 'Nested IF group'

  function addCondition() {
    onChange({
      ...group,
      conditions: [...group.conditions, createCondition(routingKeys)],
    })
  }

  function addNestedGroup() {
    onChange({
      ...group,
      nested: [...group.nested, createStatementGroup(routingKeys)],
    })
  }

  return (
    <div className={`rounded-xl border border-slate-200 bg-slate-50/40 dark:border-[#222733] dark:bg-[#0f141d] ${depth > 0 ? 'ml-4' : ''}`}>
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-slate-200 px-3 py-2 dark:border-[#222733]">
        <div className="flex items-center gap-2">
          {depth > 0 && <CornerDownRight size={14} className="text-slate-400" />}
          <span className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8490a5]">{label}</span>
          <Badge variant="gray">{group.conditions.length} AND</Badge>
          {group.nested.length > 0 && <Badge variant="blue">{group.nested.length} nested OR</Badge>}
        </div>
        <button
          type="button"
          onClick={onRemove}
          disabled={!canRemove}
          className="text-red-400 transition hover:text-red-600 disabled:cursor-not-allowed disabled:opacity-30"
          aria-label="Remove condition group"
        >
          <Trash2 size={12} />
        </button>
      </div>

      <div className="space-y-3 px-3 py-3">
        <div>
          <p className="mb-2 text-[11px] font-medium uppercase tracking-[0.14em] text-slate-500">
            All conditions in this group must match
          </p>
          <div className="space-y-2">
            {group.conditions.map((cond, idx) => (
              <div key={cond.id} className="space-y-2">
                {idx > 0 && (
                  <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-emerald-500">
                    <span className="h-px flex-1 bg-emerald-500/20" />
                    AND
                    <span className="h-px flex-1 bg-emerald-500/20" />
                  </div>
                )}
                <ConditionRowEditor
                  row={cond}
                  routingKeys={routingKeys}
                  onChange={(updated) =>
                    onChange({
                      ...group,
                      conditions: group.conditions.map((c) => (c.id === cond.id ? updated : c)),
                    })
                  }
                  onRemove={() =>
                    onChange({
                      ...group,
                      conditions: group.conditions.length > 1
                        ? group.conditions.filter((c) => c.id !== cond.id)
                        : group.conditions,
                    })
                  }
                />
              </div>
            ))}
            <Button type="button" variant="ghost" size="sm" onClick={addCondition}>
              <Plus size={12} /> Add AND condition
            </Button>
          </div>
        </div>

        <div className="space-y-2">
          {group.nested.length > 0 && (
            <p className="text-[11px] font-medium uppercase tracking-[0.14em] text-slate-500">
              Nested branches. Any nested branch can satisfy this group after the parent conditions match.
            </p>
          )}
          {group.nested.map((nested, idx) => (
            <div key={nested.id} className="space-y-2">
              {idx > 0 && (
                <div className="flex items-center gap-2 pl-4 text-[11px] font-semibold uppercase tracking-[0.18em] text-sky-400">
                  <span className="h-px flex-1 bg-sky-400/20" />
                  OR
                  <span className="h-px flex-1 bg-sky-400/20" />
                </div>
              )}
              <StatementGroupEditor
                group={nested}
                routingKeys={routingKeys}
                depth={depth + 1}
                onChange={(updated) =>
                  onChange({
                    ...group,
                    nested: group.nested.map((item) => (item.id === nested.id ? updated : item)),
                  })
                }
                onRemove={() =>
                  onChange({
                    ...group,
                    nested: group.nested.filter((item) => item.id !== nested.id),
                  })
                }
              />
            </div>
          ))}
          <Button type="button" variant="secondary" size="sm" onClick={addNestedGroup}>
            <Plus size={12} /> Add nested OR group
          </Button>
        </div>
      </div>
    </div>
  )
}

// ---- Rule block ----
function RuleBlockEditor({
  block,
  onChange,
  onRemove,
  routingKeys,
}: {
  block: RuleBlock
  onChange: (b: RuleBlock) => void
  onRemove: () => void
  routingKeys: Record<string, RoutingKeyConfig>
}) {
  const [collapsed, setCollapsed] = useState(false)

  return (
    <div className="border border-slate-200 dark:border-[#1c1c24] rounded-xl">
      <div
        className="flex items-center justify-between px-4 py-2.5 bg-[#0d0d12] rounded-t-xl cursor-pointer"
        onClick={() => setCollapsed(!collapsed)}
      >
        <input
          value={block.name}
          onChange={(e) => {
            e.stopPropagation()
            onChange({ ...block, name: e.target.value })
          }}
          onClick={(e) => e.stopPropagation()}
          placeholder="Rule name"
          className="bg-transparent text-sm font-medium focus:outline-none border-b border-transparent focus:border-[#28282f] text-slate-900"
        />
        <div className="flex items-center gap-2">
          <button type="button" onClick={(e) => { e.stopPropagation(); onRemove() }} className="text-red-400 hover:text-red-600">
            <Trash2 size={14} />
          </button>
          {collapsed ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
        </div>
      </div>
      {!collapsed && (
        <div className="px-4 py-3 space-y-3">
          {/* Conditions */}
          <div>
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
              <div>
                <p className="text-xs font-medium text-slate-500">CONDITION LOGIC</p>
                <p className="mt-1 text-xs text-slate-500 dark:text-[#8d96a8]">
                  Rule groups are evaluated top-to-bottom. Sibling groups are OR; conditions inside a group are AND.
                </p>
              </div>
              <Badge variant="blue">Nested supported</Badge>
            </div>
            <div className="space-y-2">
              {block.statements.map((group, idx) => (
                <div key={group.id} className="space-y-2">
                  {idx > 0 && (
                    <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-sky-400">
                      <span className="h-px flex-1 bg-sky-400/20" />
                      OR
                      <span className="h-px flex-1 bg-sky-400/20" />
                    </div>
                  )}
                  <StatementGroupEditor
                    group={group}
                    routingKeys={routingKeys}
                    onChange={(updated) =>
                      onChange({
                        ...block,
                        statements: block.statements.map((statement) =>
                          statement.id === group.id ? updated : statement
                        ),
                      })
                    }
                    onRemove={() =>
                      onChange({
                        ...block,
                        statements: block.statements.length > 1
                          ? block.statements.filter((statement) => statement.id !== group.id)
                          : block.statements,
                      })
                    }
                  />
                </div>
              ))}
              <Button
                type="button"
                variant="secondary"
                size="sm"
                onClick={() =>
                  onChange({
                    ...block,
                    statements: [...block.statements, createStatementGroup(routingKeys)],
                  })
                }
              >
                <Plus size={12} /> Add OR group
              </Button>
            </div>
          </div>

          <div>
            <p className="text-xs font-medium text-slate-500 mb-2">PRIORITY OUTPUT</p>
            <PriorityEditor
              gateways={block.priorityGateways}
              onChange={(gws) => onChange({ ...block, priorityGateways: gws })}
            />
          </div>
        </div>
      )}
    </div>
  )
}

// ---- Build Euclid payload ----
function buildAlgorithmData(rules: RuleBlock[], defaultOutput: DefaultOutput, routingKeys: Record<string, RoutingKeyConfig>) {
  function buildPriorityOutput(gateways: GatewayEntry[]): Record<string, unknown> {
    return {
      priority: gateways.map((g) => ({ gateway_name: g.gatewayName, gateway_id: g.gatewayId || null })),
    }
  }

  function buildCondition(c: ConditionRow) {
    return {
      lhs: c.lhs,
      comparison: OPERATOR_TO_API[c.operator] || c.operator,
      value: {
        type: routingKeys[c.lhs]?.type === 'integer' ? 'number' : 'enum_variant',
        value: routingKeys[c.lhs]?.type === 'integer' ? Number(c.value) : c.value,
      },
      metadata: {},
    }
  }

  function buildStatement(group: StatementGroup): Record<string, unknown> {
    const statement: Record<string, unknown> = {
      condition: group.conditions.map(buildCondition),
    }

    if (group.nested.length > 0) {
      statement.nested = group.nested.map(buildStatement)
    }

    return statement
  }

  return {
    globals: {},
    default_selection: buildPriorityOutput(defaultOutput.priorityGateways),
    rules: rules.map((r) => ({
      name: r.name,
      routing_type: 'priority',
      output: buildPriorityOutput(r.priorityGateways),
      statements: r.statements.map(buildStatement),
    })),
  }
}

// ---- Main Page ----
export function EuclidRulesPage() {
  const { merchantId } = useMerchantStore()
  const { routingKeysConfig, isLoading: routingKeysLoading, error: routingKeysError } = useDynamicRoutingConfig()
  const routingKeys = routingKeysConfig
  const hasRoutingKeys = Object.keys(routingKeys).length > 0
  const routingKeysUnavailable = !routingKeysLoading && (!hasRoutingKeys || Boolean(routingKeysError))
  const [ruleName, setRuleName] = useState('')
  const [ruleDesc, setRuleDesc] = useState('')
  const [ruleBlocks, setRuleBlocks] = useState<RuleBlock[]>([])
  const [defaultOutput, setDefaultOutput] = useState<DefaultOutput>({
    priorityGateways: [],
  })
  const [showJson, setShowJson] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [createdId, setCreatedId] = useState<string | null>(null)
  const [activating, setActivating] = useState(false)
  const [activateError, setActivateError] = useState<string | null>(null)
  const [activateSuccess, setActivateSuccess] = useState(false)
  const [deactivatingId, setDeactivatingId] = useState<string | null>(null)
  const [deactivateError, setDeactivateError] = useState<string | null>(null)
  const [deactivateSuccess, setDeactivateSuccess] = useState(false)
  const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set())

  const { data: allAlgorithms, mutate: mutateAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )

  const { data: activeAlgorithms, mutate: mutateActiveAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const activeIds = new Set((activeAlgorithms || []).map((a) => a.id))
  const ruleAlgorithms = (allAlgorithms || []).filter((algo) => {
    const algorithm = algo.algorithm_data || algo.algorithm
    return algorithm?.type !== 'volume_split'
  })

  const algorithmData = buildAlgorithmData(ruleBlocks, defaultOutput, routingKeys)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!merchantId) { setSubmitError('Set a Merchant ID first.'); return }
    if (routingKeysUnavailable) {
      setSubmitError('Routing key config is unavailable. Ensure backend /config/routing-keys is reachable and valid.')
      return
    }
    if (!ruleName.trim()) { setSubmitError('Rule name is required.'); return }
    setSubmitting(true)
    setSubmitError(null)
    setCreatedId(null)
    try {
      const result = await apiPost<RoutingAlgorithm>('/routing/create', {
        name: ruleName.trim(),
        description: ruleDesc,
        created_by: merchantId,
        algorithm_for: 'payment',
        algorithm: { type: 'advanced', data: algorithmData },
      })
      setCreatedId(result.id)
      mutateAlgorithms()
    } catch (err) {
      setSubmitError(String(err))
    } finally {
      setSubmitting(false)
    }
  }

  async function handleActivate(id: string) {
    if (!merchantId) return
    setActivating(true)
    setActivateError(null)
    setActivateSuccess(false)
    setDeactivateError(null)
    setDeactivateSuccess(false)
    try {
      await apiPost('/routing/activate', {
        created_by: merchantId,
        routing_algorithm_id: id,
      })
      setActivateSuccess(true)
      await Promise.all([
        mutateAlgorithms(),
        mutateActiveAlgorithms(),
      ])
    } catch (err) {
      setActivateError(String(err))
    } finally {
      setActivating(false)
    }
  }

  async function handleDeactivate(id: string) {
    if (!merchantId) return
    if (!window.confirm('Deactivate this routing rule for the selected merchant? The saved rule will remain available.')) {
      return
    }

    setDeactivatingId(id)
    setDeactivateError(null)
    setDeactivateSuccess(false)
    setActivateError(null)
    setActivateSuccess(false)
    try {
      await apiPost('/routing/deactivate', {
        created_by: merchantId,
        routing_algorithm_id: id,
      })
      setDeactivateSuccess(true)
      await Promise.all([
        mutateAlgorithms(),
        mutateActiveAlgorithms(),
      ])
    } catch (err) {
      setDeactivateError(String(err))
    } finally {
      setDeactivatingId(null)
    }
  }

  function toggleRuleExpand(id: string) {
    setExpandedRuleIds(prev => {
      const newSet = new Set(prev)
      if (newSet.has(id)) {
        newSet.delete(id)
      } else {
        newSet.add(id)
      }
      return newSet
    })
  }

  function addRuleBlock() {
    setRuleBlocks((prev) => [
      ...prev,
        {
          id: crypto.randomUUID(),
          name: `Rule ${prev.length + 1}`,
          statements: [createStatementGroup(routingKeys)],
          priorityGateways: [],
      },
    ])
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-slate-900">Rule-Based Routing</h1>
        <p className="text-sm text-slate-500 mt-1">Create declarative routing rules</p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Rule list */}
        <div className="lg:col-span-1 space-y-3">
          <Card>
            <CardHeader>
              <h2 className="text-sm font-semibold text-slate-800">Existing Rules</h2>
            </CardHeader>
            <CardBody className="p-0">
              {!merchantId ? (
                <p className="px-4 py-3 text-sm text-slate-400">Set merchant ID to load rules.</p>
              ) : !allAlgorithms ? (
                <p className="px-4 py-3 text-sm text-slate-400">Loading...</p>
              ) : ruleAlgorithms.length === 0 ? (
                <p className="px-4 py-3 text-sm text-slate-400">No rule-based rules yet.</p>
              ) : (
                <div className="divide-y divide-slate-100 dark:divide-[#222226]">
                  {ruleAlgorithms.map((algo) => {
                    const isActive = activeIds.has(algo.id)
                    const isExpanded = expandedRuleIds.has(algo.id)
                    const algorithm = algo.algorithm_data || algo.algorithm

                    return (
                      <div key={algo.id}>
                        <div className="flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-start sm:justify-between">
                          <div className="min-w-0 flex-1">
                            <p className="truncate font-medium">{algo.name}</p>
                            <p className="text-xs text-slate-400 capitalize">{algorithm?.type}</p>
                          </div>

                          <div className="flex shrink-0 flex-wrap items-center gap-2 sm:justify-end">
                            <Badge variant={isActive ? 'green' : 'gray'}>
                              {isActive ? 'Active' : 'Inactive'}
                            </Badge>
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() => toggleRuleExpand(algo.id)}
                            >
                              <Eye size={14} className="mr-1" />
                              {isExpanded ? 'Hide' : 'View'}
                            </Button>
                            {!isActive && (
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => handleActivate(algo.id)}
                                disabled={activating}
                              >
                                Activate
                              </Button>
                            )}
                            {isActive && (
                              <Button
                                size="sm"
                                variant="danger"
                                onClick={() => handleDeactivate(algo.id)}
                                disabled={deactivatingId === algo.id}
                              >
                                <PowerOff size={14} />
                                {deactivatingId === algo.id ? 'Deactivating' : 'Deactivate'}
                              </Button>
                            )}
                          </div>
                        </div>

                        {isExpanded && (
                          <div className="bg-slate-50 px-4 py-3 dark:bg-[#151518]">
                            <div className="space-y-2 text-xs text-slate-600">
                              <p><strong>ID:</strong> {algo.id}</p>
                              <p><strong>Description:</strong> {algo.description || 'N/A'}</p>
                              <p><strong>Algorithm For:</strong> {algo.algorithm_for}</p>
                              {algo.created_at && (
                                <p><strong>Created:</strong> {new Date(algo.created_at).toLocaleString()}</p>
                              )}
                              <div>
                                <strong>Configuration:</strong>
                                <pre className="mt-1 max-h-48 overflow-auto rounded border border-transparent bg-slate-100 p-2 text-xs dark:border-[#222226] dark:bg-[#0f0f11]">
                                  {JSON.stringify(algorithm, null, 2)}
                                </pre>
                              </div>
                            </div>
                          </div>
                        )}
                      </div>
                    )
                  })}
                </div>
              )}
            </CardBody>
          </Card>
          {activateError && <ErrorMessage error={activateError} />}
          {activateSuccess && (
            <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-400">
              Rule activated successfully.
            </div>
          )}
          {deactivateError && <ErrorMessage error={deactivateError} />}
          {deactivateSuccess && (
            <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-400">
              Rule deactivated successfully.
            </div>
          )}
        </div>

        {/* Rule builder */}
        <div className="lg:col-span-2 space-y-4">
          <form onSubmit={handleSubmit} className="space-y-4">
            <Card>
              <CardHeader>
                <h2 className="text-sm font-semibold text-slate-800">Rule Builder</h2>
              </CardHeader>
              <CardBody className="space-y-4">
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs text-slate-500 mb-1">Rule Name *</label>
                    <input
                      value={ruleName}
                      onChange={(e) => setRuleName(e.target.value)}
                      placeholder="my-rule"
                      className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-500 mb-1">Description</label>
                    <input
                      value={ruleDesc}
                      onChange={(e) => setRuleDesc(e.target.value)}
                      placeholder="Optional description"
                      className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                </div>

                {/* Rule blocks */}
                <div className="space-y-3">
                  <p className="text-xs font-medium text-slate-500 uppercase tracking-wide">Rules</p>
                  {routingKeysLoading && (
                    <p className="text-sm text-slate-500">Loading routing keys from backend...</p>
                  )}
                  {routingKeysUnavailable && (
                    <ErrorMessage error="Routing keys are unavailable from backend (/config/routing-keys). Rule Builder is disabled until this is fixed." />
                  )}
                  {ruleBlocks.map((block) => (
                    <RuleBlockEditor
                      key={block.id}
                      block={block}
                      routingKeys={routingKeys}
                      onChange={(updated) =>
                        setRuleBlocks((prev) =>
                          prev.map((b) => (b.id === block.id ? updated : b))
                        )
                      }
                      onRemove={() =>
                        setRuleBlocks((prev) => prev.filter((b) => b.id !== block.id))
                      }
                    />
                  ))}
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    onClick={addRuleBlock}
                    disabled={routingKeysUnavailable}
                  >
                    <Plus size={14} /> Add Rule Block
                  </Button>
                </div>

                {/* Default selection */}
                <div className="border border-slate-200 dark:border-[#1c1c24] rounded-xl px-4 py-3">
                  <p className="text-xs font-medium text-slate-500 mb-2">DEFAULT PRIORITY SELECTION (Stored no-match output)</p>
                  <p className="mb-3 text-xs leading-5 text-slate-500 dark:text-[#8d96a8]">
                    Backend uses this configured default when no rule matches. If an evaluate request sends a non-empty
                    <code className="mx-1 font-mono">fallback_output</code>, that request fallback overrides this default for that evaluation.
                    Configure percentage-based split behavior from its dedicated routing page.
                  </p>
                  <PriorityEditor
                    gateways={defaultOutput.priorityGateways}
                    onChange={(gws) =>
                      setDefaultOutput({ ...defaultOutput, priorityGateways: gws })
                    }
                  />
                </div>

                <ErrorMessage error={submitError} />
                {createdId && (
                  <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-400 flex items-center justify-between">
                    <span>Rule created (ID: {createdId})</span>
                    <Button
                      type="button"
                      size="sm"
                      onClick={() => handleActivate(createdId)}
                      disabled={activating}
                    >
                      Activate Now
                    </Button>
                  </div>
                )}
                <div className="flex gap-3">
                  <Button type="submit" disabled={submitting || routingKeysUnavailable}>
                    {submitting ? 'Creating...' : 'Create Rule'}
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    onClick={() => setShowJson(!showJson)}
                  >
                    {showJson ? 'Hide JSON' : 'Preview JSON'}
                  </Button>
                </div>
              </CardBody>
            </Card>
          </form>

          {/* JSON preview */}
          {showJson && (
            <Card>
              <CardHeader>
                <h2 className="text-sm font-semibold text-slate-800">JSON Preview</h2>
              </CardHeader>
              <CardBody>
                <pre className="text-xs text-slate-600 overflow-auto max-h-64 bg-[#07070b] rounded-lg p-4 font-mono border border-slate-200 dark:border-[#1c1c24]">
                  {JSON.stringify(
                    {
                      name: ruleName,
                      description: ruleDesc,
                      created_by: merchantId,
                      algorithm_for: 'payment',
                      algorithm: { type: 'advanced', data: algorithmData },
                    },
                    null,
                    2
                  )}
                </pre>
              </CardBody>
            </Card>
          )}
        </div>
      </div>
    </div>
  )
}
