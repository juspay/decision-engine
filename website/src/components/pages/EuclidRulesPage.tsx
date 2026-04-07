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
import { ROUTING_KEYS, RoutingKey } from '../../lib/constants'
import { Plus, Trash2, GripVertical, ChevronDown, ChevronUp, Eye } from 'lucide-react'

const OPERATOR_TO_API: Record<string, string> = {
  '==': 'equal',
  '!=': 'not_equal',
  '>': 'greater_than',
  '<': 'less_than',
  '>=': 'greater_than_equal',
  '<=': 'less_than_equal',
}

// ---- Types for builder ----
interface GatewayEntry {
  id: string
  name: string
}

interface VolSplitEntry {
  id: string
  name: string
  split: number
}

interface ConditionRow {
  id: string
  lhs: RoutingKey
  operator: string
  value: string
}

interface RuleBlock {
  id: string
  name: string
  conditions: ConditionRow[]
  outputType: 'priority' | 'volume_split'
  priorityGateways: GatewayEntry[]
  volumeGateways: VolSplitEntry[]
}

type DefaultOutput = {
  type: 'priority' | 'volume_split'
  priorityGateways: GatewayEntry[]
  volumeGateways: VolSplitEntry[]
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
  const [newName, setNewName] = useState('')
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
    if (!newName.trim()) return
    onChange([...gateways, { id: crypto.randomUUID(), name: newName.trim() }])
    setNewName('')
  }

  return (
    <div className="space-y-2">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={gateways.map((g) => g.id)} strategy={verticalListSortingStrategy}>
          {gateways.map((gw, idx) => (
            <SortableGatewayItem
              key={gw.id}
              id={gw.id}
              name={`${idx + 1}. ${gw.name}`}
              onRemove={() => onChange(gateways.filter((g) => g.id !== gw.id))}
            />
          ))}
        </SortableContext>
      </DndContext>
      <div className="flex gap-2">
        <input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="gateway name"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <Button type="button" size="sm" variant="secondary" onClick={add}>
          <Plus size={13} /> Add
        </Button>
      </div>
    </div>
  )
}

// ---- Volume split output editor ----
function VolumeSplitEditor({
  gateways,
  onChange,
}: {
  gateways: VolSplitEntry[]
  onChange: (gws: VolSplitEntry[]) => void
}) {
  const [newName, setNewName] = useState('')
  const total = gateways.reduce((s, g) => s + g.split, 0)

  function add() {
    if (!newName.trim()) return
    onChange([...gateways, { id: crypto.randomUUID(), name: newName.trim(), split: 0 }])
    setNewName('')
  }

  return (
    <div className="space-y-2">
      {gateways.map((gw) => (
        <div key={gw.id} className="flex items-center gap-2">
          <span className="text-sm font-mono w-24 truncate">{gw.name}</span>
          <input
            type="range"
            min={0}
            max={100}
            value={gw.split}
            onChange={(e) =>
              onChange(
                gateways.map((g) =>
                  g.id === gw.id ? { ...g, split: Number(e.target.value) } : g
                )
              )
            }
            className="flex-1 accent-brand-500"
          />
          <span className="text-sm w-10 text-right">{gw.split}%</span>
          <button
            type="button"
            onClick={() => onChange(gateways.filter((g) => g.id !== gw.id))}
            className="text-red-400 hover:text-red-600"
          >
            <Trash2 size={12} />
          </button>
        </div>
      ))}
      <div className={`text-xs font-medium ${total === 100 ? 'text-emerald-400' : 'text-red-400'}`}>
        Total: {total}% {total !== 100 && '(must equal 100)'}
      </div>
      <div className="flex gap-2">
        <input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="gateway name"
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
}: {
  row: ConditionRow
  onChange: (r: ConditionRow) => void
  onRemove: () => void
}) {
  const keyInfo = ROUTING_KEYS[row.lhs]
  const isEnum = keyInfo?.type === 'enum'
  const isInt = keyInfo?.type === 'integer'

  const operators = isInt
    ? ['>', '<', '>=', '<=', '==', '!=']
    : ['==', '!=']

  return (
    <div className="flex items-center gap-2 flex-wrap">
      <select
        value={row.lhs}
        onChange={(e) => onChange({ ...row, lhs: e.target.value as RoutingKey, value: '', operator: '==' })}
        className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs focus:outline-none"
      >
        {Object.keys(ROUTING_KEYS).map((k) => (
          <option key={k} value={k}>
            {k}
          </option>
        ))}
      </select>
      <select
        value={row.operator}
        onChange={(e) => onChange({ ...row, operator: e.target.value })}
        className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs focus:outline-none"
      >
        {operators.map((op) => (
          <option key={op} value={op}>
            {op}
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
          {ROUTING_KEYS[row.lhs].values.map((v: string) => (
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

// ---- Rule block ----
function RuleBlockEditor({
  block,
  onChange,
  onRemove,
}: {
  block: RuleBlock
  onChange: (b: RuleBlock) => void
  onRemove: () => void
}) {
  const [collapsed, setCollapsed] = useState(false)

  function addCondition() {
    onChange({
      ...block,
      conditions: [
        ...block.conditions,
        {
          id: crypto.randomUUID(),
          lhs: 'payment_method',
          operator: '==',
          value: 'card',
        },
      ],
    })
  }

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
            <p className="text-xs font-medium text-slate-500 mb-2">CONDITIONS</p>
            <div className="space-y-2">
              {block.conditions.map((cond) => (
                <ConditionRowEditor
                  key={cond.id}
                  row={cond}
                  onChange={(updated) =>
                    onChange({
                      ...block,
                      conditions: block.conditions.map((c) =>
                        c.id === cond.id ? updated : c
                      ),
                    })
                  }
                  onRemove={() =>
                    onChange({
                      ...block,
                      conditions: block.conditions.filter((c) => c.id !== cond.id),
                    })
                  }
                />
              ))}
              <Button type="button" variant="ghost" size="sm" onClick={addCondition}>
                <Plus size={12} /> Add Condition
              </Button>
            </div>
          </div>

          {/* Output */}
          <div>
            <p className="text-xs font-medium text-slate-500 mb-2">OUTPUT</p>
            <div className="flex gap-4 mb-3">
              {(['priority', 'volume_split'] as const).map((t) => (
                <label key={t} className="flex items-center gap-1.5 text-xs cursor-pointer">
                  <input
                    type="radio"
                    checked={block.outputType === t}
                    onChange={() => onChange({ ...block, outputType: t })}
                    className="accent-brand-500"
                  />
                  {t === 'priority' ? 'Priority' : 'Volume Split'}
                </label>
              ))}
            </div>
            {block.outputType === 'priority' ? (
              <PriorityEditor
                gateways={block.priorityGateways}
                onChange={(gws) => onChange({ ...block, priorityGateways: gws })}
              />
            ) : (
              <VolumeSplitEditor
                gateways={block.volumeGateways}
                onChange={(gws) => onChange({ ...block, volumeGateways: gws })}
              />
            )}
          </div>
        </div>
      )}
    </div>
  )
}

// ---- Build Euclid payload ----
function buildAlgorithmData(rules: RuleBlock[], defaultOutput: DefaultOutput) {
  function buildOutput(type: 'priority' | 'volume_split', pg: GatewayEntry[], vg: VolSplitEntry[]): Record<string, unknown> {
    if (type === 'priority') {
      return {
        priority: pg.map((g) => ({ gateway_name: g.name, gateway_id: null })),
      }
    }
    return {
      volume_split: vg.map((g) => ({
        split: g.split,
        output: { gateway_name: g.name, gateway_id: null },
      })),
    }
  }

  function getRoutingType(type: 'priority' | 'volume_split'): string {
    return type === 'priority' ? 'priority' : 'volume_split'
  }

  return {
    globals: {},
    default_selection: buildOutput(
      defaultOutput.type,
      defaultOutput.priorityGateways,
      defaultOutput.volumeGateways
    ),
    rules: rules.map((r) => ({
      name: r.name,
      routing_type: getRoutingType(r.outputType),
      output: buildOutput(r.outputType, r.priorityGateways, r.volumeGateways),
      statements: [
        {
          condition: r.conditions.map((c) => ({
            lhs: c.lhs,
            comparison: OPERATOR_TO_API[c.operator] || c.operator,
            value: {
              type: ROUTING_KEYS[c.lhs]?.type === 'integer' ? 'number' : 'enum_variant',
              value: ROUTING_KEYS[c.lhs]?.type === 'integer' ? Number(c.value) : c.value,
            },
            metadata: {},
          })),
        },
      ],
    })),
  }
}

// ---- Main Page ----
export function EuclidRulesPage() {
  const { merchantId } = useMerchantStore()
  const [ruleName, setRuleName] = useState('')
  const [ruleDesc, setRuleDesc] = useState('')
  const [ruleBlocks, setRuleBlocks] = useState<RuleBlock[]>([])
  const [defaultOutput, setDefaultOutput] = useState<DefaultOutput>({
    type: 'priority',
    priorityGateways: [],
    volumeGateways: [],
  })
  const [showJson, setShowJson] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [createdId, setCreatedId] = useState<string | null>(null)
  const [activating, setActivating] = useState(false)
  const [activateError, setActivateError] = useState<string | null>(null)
  const [activateSuccess, setActivateSuccess] = useState(false)
  const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set())

  const { data: allAlgorithms, mutate } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )

  const { data: activeAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const activeIds = new Set((activeAlgorithms || []).map((a) => a.id))

  const algorithmData = buildAlgorithmData(ruleBlocks, defaultOutput)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!merchantId) { setSubmitError('Set a Merchant ID first.'); return }
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
      mutate()
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
    try {
      await apiPost('/routing/activate', {
        created_by: merchantId,
        routing_algorithm_id: id,
      })
      setActivateSuccess(true)
      mutate()
    } catch (err) {
      setActivateError(String(err))
    } finally {
      setActivating(false)
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
        conditions: [],
        outputType: 'priority',
        priorityGateways: [],
        volumeGateways: [],
      },
    ])
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-slate-900">Rule-Based Routing</h1>
        <p className="text-sm text-slate-500 mt-1">Create Euclid DSL declarative routing rules</p>
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
              ) : allAlgorithms.length === 0 ? (
                <p className="px-4 py-3 text-sm text-slate-400">No rules yet.</p>
              ) : (
                <table className="w-full text-sm">
                  <tbody>
                    {allAlgorithms.map((algo) => {
                      const isActive = activeIds.has(algo.id)
                      const isExpanded = expandedRuleIds.has(algo.id)
                      // Backend returns algorithm_data, map it to algorithm for display
                      const algorithm = algo.algorithm_data || algo.algorithm
                      return (
                        <>
                          <tr key={algo.id} className="border-b border-slate-100 dark:border-[#222226] last:border-0">
                            <td className="px-4 py-3">
                              <p className="font-medium truncate">{algo.name}</p>
                              <p className="text-xs text-slate-400 capitalize">{algorithm?.type}</p>
                            </td>
                            <td className="px-2 py-3">
                              <Badge variant={isActive ? 'green' : 'gray'}>
                                {isActive ? 'Active' : 'Inactive'}
                              </Badge>
                            </td>
                            <td className="px-2 py-3">
                              <div className="flex items-center gap-1">
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
                              </div>
                            </td>
                          </tr>
                          {isExpanded && (
                            <tr>
                              <td colSpan={3} className="px-4 py-3 bg-slate-50 dark:bg-[#151518]">
                                <div className="text-xs text-slate-600 space-y-2">
                                  <p><strong>ID:</strong> {algo.id}</p>
                                  <p><strong>Description:</strong> {algo.description || 'N/A'}</p>
                                  <p><strong>Algorithm For:</strong> {algo.algorithm_for}</p>
                                  {algo.created_at && (
                                    <p><strong>Created:</strong> {new Date(algo.created_at).toLocaleString()}</p>
                                  )}
                                  <div>
                                    <strong>Configuration:</strong>
                                    <pre className="mt-1 p-2 bg-slate-100 dark:bg-[#0f0f11] border border-transparent dark:border-[#222226] rounded text-xs overflow-auto max-h-48">
                                      {JSON.stringify(algorithm, null, 2)}
                                    </pre>
                                  </div>
                                </div>
                              </td>
                            </tr>
                          )}
                        </>
                      )
                    })}
                  </tbody>
                </table>
              )}
            </CardBody>
          </Card>
          {activateError && <ErrorMessage error={activateError} />}
          {activateSuccess && (
            <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-400">
              Rule activated successfully.
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
                  {ruleBlocks.map((block) => (
                    <RuleBlockEditor
                      key={block.id}
                      block={block}
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
                  <Button type="button" variant="secondary" size="sm" onClick={addRuleBlock}>
                    <Plus size={14} /> Add Rule Block
                  </Button>
                </div>

                {/* Default selection */}
                <div className="border border-slate-200 dark:border-[#1c1c24] rounded-xl px-4 py-3">
                  <p className="text-xs font-medium text-slate-500 mb-2">DEFAULT SELECTION (Fallback)</p>
                  <div className="flex gap-4 mb-3">
                    {(['priority', 'volume_split'] as const).map((t) => (
                      <label key={t} className="flex items-center gap-1.5 text-xs cursor-pointer">
                        <input
                          type="radio"
                          checked={defaultOutput.type === t}
                          onChange={() => setDefaultOutput({ ...defaultOutput, type: t })}
                          className="accent-brand-500"
                        />
                        {t === 'priority' ? 'Priority' : 'Volume Split'}
                      </label>
                    ))}
                  </div>
                  {defaultOutput.type === 'priority' ? (
                    <PriorityEditor
                      gateways={defaultOutput.priorityGateways}
                      onChange={(gws) =>
                        setDefaultOutput({ ...defaultOutput, priorityGateways: gws })
                      }
                    />
                  ) : (
                    <VolumeSplitEditor
                      gateways={defaultOutput.volumeGateways}
                      onChange={(gws) =>
                        setDefaultOutput({ ...defaultOutput, volumeGateways: gws })
                      }
                    />
                  )}
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
                  <Button type="submit" disabled={submitting}>
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
