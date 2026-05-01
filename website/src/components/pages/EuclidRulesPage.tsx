import { useEffect, useRef, useState } from 'react'
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
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { SearchableSelect } from '../ui/SearchableSelect'
import { SearchableMultiSelect } from '../ui/SearchableMultiSelect'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { useDynamicRoutingConfig, RoutingKeyConfig } from '../../hooks/useDynamicRoutingConfig'
import { EuclidAlgorithmData } from '../../types/api'
import {
  RuleCodeEditor, serializeToDSL, parseDSL, CODE_EDITOR_PLACEHOLDER,
  type RuleBlock, type StatementGroup, type ConditionRow,
  type GatewayEntry, type VolumeSplitEntry, type VolumeSplitPriorityEntry,
} from '../ui/RuleCodeEditor'
import { Plus, Trash2, GripVertical, ChevronDown, ChevronUp, PowerOff, CornerDownRight, CopyPlus, MoreVertical } from 'lucide-react'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { CopyButton } from '../ui/CopyButton'

const OPERATOR_TO_API: Record<string, string> = {
  '==': 'equal',
  '!=': 'not_equal',
  '>': 'greater_than',
  '<': 'less_than',
  '>=': 'greater_than_equal',
  '<=': 'less_than_equal',
  'in': 'equal',
  'not_in': 'not_equal',
}

const OPERATOR_LABELS: Record<string, string> = {
  '==': 'equals',
  '!=': 'not equals',
  '>': 'greater than',
  '<': 'less than',
  '>=': 'greater than or equal',
  '<=': 'less than or equal',
  'in': 'is one of',
  'not_in': 'is not one of',
}

function toLabel(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
}


type DefaultOutput = {
  priorityGateways: GatewayEntry[]
}

type RuleOutputValidationError = {
  ruleId: string
  message: string
}

function createCondition(routingKeys: Record<string, RoutingKeyConfig>): ConditionRow {
  const firstKey = Object.keys(routingKeys)[0] || 'payment_method'
  const firstKeyConfig = routingKeys[firstKey]
  const firstKeyValues = firstKeyConfig?.values || []
  return {
    id: crypto.randomUUID(),
    lhs: firstKey,
    operator: '==',
    value: firstKeyConfig?.type === 'enum' ? (firstKeyValues[0] || '') : '',
    metadataKey: firstKeyConfig?.type === 'udf' && firstKey !== 'metadata' ? firstKey : undefined,
  }
}

function createStatementGroup(routingKeys: Record<string, RoutingKeyConfig>): StatementGroup {
  return {
    id: crypto.randomUUID(),
    conditions: [createCondition(routingKeys)],
    nested: [],
  }
}

function formatOp(comparison: string): string {
  const map: Record<string, string> = {
    equal: '=', not_equal: '≠',
    greater_than: '>', less_than: '<',
    greater_than_equal: '≥', less_than_equal: '≤',
  }
  return map[comparison] ?? comparison.replace(/_/g, ' ')
}

function formatScalar(value: unknown): string {
  if (value === null || value === undefined) return ''
  if (Array.isArray(value)) return value.map(formatScalar).filter(Boolean).join(', ')
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

function getMetadataVariant(value: unknown): { key?: string; value?: unknown } | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const metadata = value as { key?: unknown; value?: unknown }
  return {
    key: metadata.key === undefined || metadata.key === null ? undefined : String(metadata.key),
    value: metadata.value,
  }
}

function formatConditionSubject(cond: EuclidAlgorithmData['rules'][number]['statements'][number]['condition'][number]): string {
  const metadata = cond.value?.type === 'metadata_variant' ? getMetadataVariant(cond.value.value) : null
  const lhs = toLabel(String(cond.lhs ?? ''))
  return metadata?.key ? `${lhs}[${metadata.key}]` : lhs
}

function formatConditionValue(cond: EuclidAlgorithmData['rules'][number]['statements'][number]['condition'][number]): string {
  const metadata = cond.value?.type === 'metadata_variant' ? getMetadataVariant(cond.value.value) : null
  return formatScalar(metadata ? metadata.value : cond.value?.value)
}

function formatRoutingType(type: string | undefined): string {
  if (type === 'volume_split_priority') return 'Split + Priority'
  if (type === 'volume_split') return 'Volume Split'
  if (type === 'priority') return 'Priority'
  return type ? toLabel(type) : 'Priority'
}


function RuleBreakdown({ algo }: { algo: RoutingAlgorithm }) {
  const algorithm = algo.algorithm_data || algo.algorithm
  const data = algorithm?.data as EuclidAlgorithmData | undefined
  const defaultSel = data?.default_selection || data?.defaultSelection

  return (
    <div className="space-y-2.5">
      {(data?.rules ?? []).map((rule, i) => {
        const output = rule.output as Record<string, unknown> | undefined
        // Backend may return { priority: [...] }, { type, data: [...] }, { volume_split: [...] }, or { volume_split_priority: [...] }
        const rawPriority = output?.priority ?? output?.data
        const rawVolume = output?.volume_split ?? (output?.type === 'volume_split' ? output?.data : undefined)
        const rawVolumeSplitPriority = output?.volume_split_priority
        const priorityGateways = (
          Array.isArray(rawPriority) ? rawPriority : []
        ) as { gateway_name: string; gateway_id: string | null }[]
        const volumeSplits = (
          Array.isArray(rawVolume) ? rawVolume : []
        ) as { split: number; output: { gateway_name: string } }[]
        const volumeSplitPriorityEntries = (
          Array.isArray(rawVolumeSplitPriority) ? rawVolumeSplitPriority : []
        ) as { split: number; output: { gateway_name: string; gateway_id: string | null }[] }[]
        const isVolumeSplit = rule.routing_type === 'volume_split' || volumeSplits.length > 0
        const isVolumeSplitPriority = rule.routing_type === 'volume_split_priority' || volumeSplitPriorityEntries.length > 0

        type CondEntry = { op: 'IF' | 'AND' | 'OR'; cond: typeof rule.statements[0]['condition'][0] }
        // Each statement becomes its own visual group, separated by OR dividers.
        const condGroups: CondEntry[][] = []
        rule.statements.forEach((s) => {
          const group: CondEntry[] = []

          s.condition.forEach((cond) => {
            group.push({ op: group.length === 0 ? 'IF' : 'AND', cond })
          })

          ;(s.nested ?? []).forEach((nestedGroup, ni) => {
            nestedGroup.condition.forEach((cond, ci) => {
              if (ci === 0) group.push({ op: (group.length === 0 || ni === 0) ? (group.length === 0 ? 'IF' : 'AND') : 'OR', cond })
              else group.push({ op: 'AND', cond })
            })
          })

          if (group.length > 0) condGroups.push(group)
        })

        return (
          <div key={i} className="overflow-hidden rounded-xl border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0d1018]">
            <div className="flex items-center justify-between gap-2 border-b border-slate-100 dark:border-[#1e2330] bg-slate-50 dark:bg-[#10131c] px-3 py-1.5">
              <span className="text-[11px] font-semibold uppercase tracking-wide text-slate-400 dark:text-[#4e5870]">
                {rule.name || `Rule ${i + 1}`}
              </span>
              {rule.routing_type && (
                <span className={`shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-semibold leading-4 ${
                  rule.routing_type === 'volume_split_priority'
                    ? 'border-purple-500/25 bg-purple-500/10 text-purple-700 dark:text-purple-300'
                    : rule.routing_type === 'volume_split'
                    ? 'border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300'
                    : 'border-brand-500/25 bg-brand-500/10 text-brand-700 dark:text-brand-300'
                }`}>
                  {formatRoutingType(rule.routing_type)}
                </span>
              )}
            </div>
            <div className="space-y-1.5 px-3 py-2.5">
              {condGroups.map((group, gi) => (
                <div key={gi}>
                  {gi > 0 && (
                    <div className="flex items-center gap-2 my-1.5">
                      <span className="h-px flex-1 bg-slate-200 dark:bg-[#1e2330]" />
                      <span className="text-[10px] font-bold text-sky-500">OR</span>
                      <span className="h-px flex-1 bg-slate-200 dark:bg-[#1e2330]" />
                    </div>
                  )}
                  <div className="rounded-lg border border-slate-200 dark:border-[#1e2330] divide-y divide-slate-100 dark:divide-[#1e2330] overflow-hidden">
                    {group.map(({ op, cond }, ci) => (
                      <div key={ci} className="flex items-center gap-2 text-xs px-2 py-1.5">
                        <span className={`w-7 shrink-0 text-right text-[10px] font-bold select-none ${op === 'OR' ? 'text-sky-400 dark:text-sky-500' : 'text-slate-300 dark:text-[#3a4258]'}`}>
                          {op}
                        </span>
                        <span className="rounded-md bg-slate-100 dark:bg-[#1a1f2a] px-2 py-1 text-slate-700 dark:text-[#c8d0de]">
                          {formatConditionSubject(cond)}{' '}
                          <span className="font-mono text-slate-400 dark:text-[#5d6880]">{formatOp(String(cond.comparison ?? ''))}</span>{' '}
                          <span className="font-medium">{formatConditionValue(cond)}</span>
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
              <div className="flex items-start gap-2 pt-0.5 text-xs">
                <span className="w-7 shrink-0 text-right text-[10px] font-bold text-brand-500 select-none">→</span>
                <div className="flex flex-wrap gap-1">
                  {isVolumeSplitPriority
                    ? volumeSplitPriorityEntries.map((e, j) => (
                        <span key={j} className="rounded-full bg-purple-50 dark:bg-purple-900/20 px-2 py-0.5 text-[11px] font-medium text-purple-700 dark:text-purple-300">
                          {e.split}%: {e.output.map(g => g.gateway_name).join(', ')}
                        </span>
                      ))
                    : isVolumeSplit
                    ? volumeSplits.map((s, j) => (
                        <span key={j} className="rounded-full bg-emerald-50 dark:bg-emerald-900/20 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                          {s.output.gateway_name} {s.split}%
                        </span>
                      ))
                    : priorityGateways.map((g, j) => (
                        <span key={j} className="rounded-full bg-brand-50 dark:bg-brand-900/20 px-2 py-0.5 text-[11px] font-medium text-brand-700 dark:text-brand-300">
                          {j + 1}. {g.gateway_name}
                        </span>
                      ))
                  }
                  {!isVolumeSplitPriority && !isVolumeSplit && priorityGateways.length === 0 && (
                    <span className="text-slate-400 italic">No output configured</span>
                  )}
                </div>
              </div>
            </div>
          </div>
        )
      })}

      {(() => {
        const defRaw = defaultSel as Record<string, unknown> | undefined
        const defGateways = (Array.isArray(defRaw?.priority) ? defRaw!.priority : Array.isArray(defRaw?.data) ? defRaw!.data : []) as { gateway_name: string }[]
        return defGateways.length > 0 ? (
          <div className="flex items-center gap-2 px-1 text-xs">
            <span className="text-slate-400 dark:text-[#4e5870]">Default:</span>
            <div className="flex flex-wrap gap-1">
              {defGateways.map((g, i) => (
                <span key={i} className="rounded-full bg-slate-100 dark:bg-[#1a1f2a] px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:text-[#8090a8]">
                  {g.gateway_name}
                </span>
              ))}
            </div>
          </div>
        ) : null
      })()}

      {(!data?.rules || data.rules.length === 0) && (
        <p className="text-xs text-slate-400 italic">No rules configured.</p>
      )}
    </div>
  )
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
  suggestions = [],
  highlightMissing = false,
}: {
  gateways: GatewayEntry[]
  onChange: (gws: GatewayEntry[]) => void
  suggestions?: string[]
  highlightMissing?: boolean
}) {
  const [newGatewayName, setNewGatewayName] = useState('')
  const [newGatewayId, setNewGatewayId] = useState('')
  const listId = useRef(`gateway-suggestions-${Math.random().toString(36).slice(2)}`).current
  const gatewayNameInputRef = useRef<HTMLInputElement>(null)
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  )

  useEffect(() => {
    if (highlightMissing) gatewayNameInputRef.current?.focus()
  }, [highlightMissing])

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
      { id: crypto.randomUUID(), gatewayName: newGatewayName.trim(), gatewayId: newGatewayId.trim() },
    ])
    setNewGatewayName('')
    setNewGatewayId('')
  }

  const unusedSuggestions = suggestions.filter((s) => !gateways.some((g) => g.gatewayName === s))

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
      <datalist id={listId}>
        {unusedSuggestions.map((s) => <option key={s} value={s} />)}
      </datalist>
      <div className="flex gap-2">
        <input
          ref={gatewayNameInputRef}
          value={newGatewayName}
          onChange={(e) => setNewGatewayName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          list={listId}
          placeholder="Gateway name"
          aria-invalid={highlightMissing}
          aria-describedby={highlightMissing ? `${listId}-missing-output` : undefined}
          className={`border bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 ${
            highlightMissing
              ? 'border-red-400 ring-2 ring-red-400/35 focus:ring-red-400 dark:border-red-400 dark:ring-red-500/35'
              : 'border-slate-200 focus:ring-brand-500 dark:border-[#222226]'
          }`}
        />
        <input
          value={newGatewayId}
          onChange={(e) => setNewGatewayId(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="Gateway ID (optional)"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <Button
          type="button"
          size="sm"
          variant="secondary"
          onClick={add}
          className={highlightMissing ? 'ring-2 ring-red-400/45 dark:ring-red-500/40' : ''}
        >
          <Plus size={13} /> Add
        </Button>
      </div>
      {highlightMissing && (
        <p id={`${listId}-missing-output`} className="text-xs font-medium text-red-500 dark:text-red-300">
          Enter a gateway name here, then click Add to include it in the rule output.
        </p>
      )}
    </div>
  )
}

// ---- Volume split editor ----
function VolumeSplitEditor({
  entries,
  onChange,
  suggestions = [],
}: {
  entries: VolumeSplitEntry[]
  onChange: (e: VolumeSplitEntry[]) => void
  suggestions?: string[]
}) {
  const [newSplit, setNewSplit] = useState('')
  const [newName, setNewName] = useState('')
  const [newId, setNewId] = useState('')
  const listId = `vs-suggestions-${Math.random().toString(36).slice(2)}`

  const total = entries.reduce((s, e) => s + e.split, 0)

  function add() {
    if (!newName.trim() || !newSplit) return
    onChange([
      ...entries,
      { id: crypto.randomUUID(), split: Number(newSplit), gatewayName: newName.trim(), gatewayId: newId.trim() },
    ])
    setNewSplit('')
    setNewName('')
    setNewId('')
  }

  const unusedSuggestions = suggestions.filter((s) => !entries.some((e) => e.gatewayName === s))

  return (
    <div className="space-y-2">
      {entries.map((e) => (
        <div
          key={e.id}
          className="flex items-center gap-2 bg-slate-100 dark:bg-[#111118] border border-slate-200 dark:border-[#1c1c24] rounded-lg px-2 py-1.5"
        >
          <span className="text-xs font-bold text-brand-500 w-10 shrink-0 tabular-nums">{e.split}%</span>
          <span className="text-sm flex-1 font-mono">
            {e.gatewayName}{e.gatewayId ? ` (${e.gatewayId})` : ''}
          </span>
          <button type="button" onClick={() => onChange(entries.filter((x) => x.id !== e.id))} className="text-red-400 hover:text-red-600">
            <Trash2 size={12} />
          </button>
        </div>
      ))}
      {entries.length > 0 && (
        <p className={`text-xs font-medium ${total === 100 ? 'text-emerald-500' : 'text-amber-500'}`}>
          Total: {total}%{total !== 100 ? ' (must equal 100%)' : ' ✓'}
        </p>
      )}
      <datalist id={listId}>
        {unusedSuggestions.map((s) => <option key={s} value={s} />)}
      </datalist>
      <div className="flex gap-2">
        <input
          type="number"
          value={newSplit}
          onChange={(e) => setNewSplit(e.target.value)}
          placeholder="Split %"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          list={listId}
          placeholder="Gateway name"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <input
          value={newId}
          onChange={(e) => setNewId(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="Gateway ID (optional)"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <Button type="button" size="sm" variant="secondary" onClick={add}>
          <Plus size={13} /> Add
        </Button>
      </div>
    </div>
  )
}

// ---- Volume split priority editor ----
function VolumeSplitPriorityEditor({
  entries,
  onChange,
  suggestions = [],
}: {
  entries: VolumeSplitPriorityEntry[]
  onChange: (e: VolumeSplitPriorityEntry[]) => void
  suggestions?: string[]
}) {
  const [newSplit, setNewSplit] = useState('')

  const total = entries.reduce((s, e) => s + e.split, 0)

  function addSplit() {
    if (!newSplit) return
    onChange([...entries, { id: crypto.randomUUID(), split: Number(newSplit), gateways: [] }])
    setNewSplit('')
  }

  function updateEntry(id: string, patch: Partial<VolumeSplitPriorityEntry>) {
    onChange(entries.map((e) => (e.id === id ? { ...e, ...patch } : e)))
  }

  return (
    <div className="space-y-3">
      {entries.length > 0 && (
        <p className={`text-xs font-medium ${total === 100 ? 'text-emerald-500' : 'text-amber-500'}`}>
          Total: {total}%{total !== 100 ? ' (must equal 100%)' : ' ✓'}
        </p>
      )}
      {entries.map((entry, idx) => (
        <div
          key={entry.id}
          className="rounded-lg border border-slate-200 dark:border-[#222226] overflow-hidden"
        >
          <div className="flex items-center gap-2 px-3 py-2 bg-slate-50 dark:bg-[#111118] border-b border-slate-200 dark:border-[#1c1c24]">
            <span className="text-xs text-slate-400 font-medium shrink-0">Split {idx + 1}:</span>
            <input
              type="number"
              value={entry.split}
              onChange={(e) => updateEntry(entry.id, { split: Number(e.target.value) })}
              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded px-2 py-0.5 text-xs w-16 focus:outline-none"
            />
            <span className="text-xs text-slate-400">%</span>
            <button
              type="button"
              onClick={() => onChange(entries.filter((e) => e.id !== entry.id))}
              className="ml-auto text-red-400 hover:text-red-600"
            >
              <Trash2 size={12} />
            </button>
          </div>
          <div className="p-3">
            <p className="text-[10px] font-semibold uppercase tracking-widest text-slate-400 mb-2">Priority list for this split</p>
            <PriorityEditor
              gateways={entry.gateways}
              suggestions={suggestions}
              onChange={(gws) => updateEntry(entry.id, { gateways: gws })}
            />
          </div>
        </div>
      ))}
      <div className="flex gap-2 items-center">
        <input
          type="number"
          value={newSplit}
          onChange={(e) => setNewSplit(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), addSplit())}
          placeholder="Split %"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm w-24 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <Button type="button" size="sm" variant="secondary" onClick={addSplit}>
          <Plus size={13} /> Add split
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
  const isUdf = keyInfo?.type === 'udf'
  const isStr = keyInfo?.type === 'str_value'
  const isMulti = row.operator === 'in' || row.operator === 'not_in'

  const operators = isInt
    ? ['>', '<', '>=', '<=', '==', '!=']
    : isEnum
    ? ['==', '!=', 'in', 'not_in']
    : ['==', '!=']

  const selectedValues = Array.isArray(row.value) ? row.value : []

  function handleOperatorChange(op: string) {
    const switchingToMulti = op === 'in' || op === 'not_in'
    const switchingFromMulti = row.operator === 'in' || row.operator === 'not_in'
    let newValue: string | string[] = row.value
    if (switchingToMulti && !Array.isArray(row.value)) {
      newValue = row.value ? [row.value as string] : []
    } else if (!switchingToMulti && switchingFromMulti) {
      newValue = Array.isArray(row.value) ? (row.value[0] ?? '') : ''
    }
    onChange({ ...row, operator: op, value: newValue })
  }

  return (
    <div className="flex items-start gap-2 flex-wrap">
      <SearchableSelect
        dataCy="cond-lhs"
        value={row.lhs}
        onChange={(newKey) => {
          const newConfig = routingKeys[newKey]
          const defaultValue = newConfig?.type === 'enum' ? (newConfig.values?.[0] ?? '') : ''
          onChange({
            ...row,
            lhs: newKey,
            value: defaultValue,
            metadataKey: newConfig?.type === 'udf' && newKey !== 'metadata' ? newKey : undefined,
            operator: '==',
          })
        }}
        options={Object.keys(routingKeys).map((k) => ({ value: k, label: toLabel(k) }))}
      />
      <select
        value={row.operator}
        onChange={(e) => handleOperatorChange(e.target.value)}
        aria-label="Condition operator"
        className="cond-select min-w-[9.5rem]"
      >
        {operators.map((op) => (
          <option key={op} value={op}>{OPERATOR_LABELS[op] || op}</option>
        ))}
      </select>
      {isEnum && isMulti ? (
        <div data-cy="cond-val">
          <SearchableMultiSelect
            values={selectedValues}
            onChange={(vals) => onChange({ ...row, value: vals })}
            options={(keyInfo?.values || []).map((v: string) => ({ value: v, label: toLabel(v) }))}
            placeholder="Select values…"
          />
        </div>
      ) : isEnum ? (
        <SearchableSelect
          dataCy="cond-val"
          value={row.value as string}
          onChange={(v) => onChange({ ...row, value: v })}
          options={(keyInfo?.values || []).map((v: string) => ({ value: v, label: toLabel(v) }))}
        />
      ) : isInt ? (
        <input
          type="number"
          value={row.value as string}
          onChange={(e) => onChange({ ...row, value: e.target.value })}
          placeholder="value"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs w-24 focus:outline-none"
        />
      ) : isUdf ? (
        <>
          <input
            type="text"
            value={row.metadataKey ?? ''}
            onChange={(e) => onChange({ ...row, metadataKey: e.target.value })}
            placeholder="metadata key"
            aria-label="Metadata key"
            className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs w-36 focus:outline-none"
          />
          <input
            type="text"
            value={row.value as string}
            onChange={(e) => onChange({ ...row, value: e.target.value })}
            placeholder="value"
            aria-label="Metadata value"
            className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs w-32 focus:outline-none"
          />
        </>
      ) : isStr ? (
        <input
          type="text"
          value={row.value as string}
          onChange={(e) => onChange({ ...row, value: e.target.value })}
          placeholder="value"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs w-32 focus:outline-none"
        />
      ) : (
        <input
          type="text"
          value={row.value as string}
          onChange={(e) => onChange({ ...row, value: e.target.value })}
          placeholder="value"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-xs w-32 focus:outline-none"
        />
      )}
      <button type="button" onClick={onRemove} aria-label="Remove condition" className="text-red-400 hover:text-red-600 mt-1">
        <Trash2 size={12} />
      </button>
    </div>
  )
}

// ---- Condition group ----
function ConditionGroupEditor({
  group,
  onChange,
  onRemove,
  canRemove,
  routingKeys,
  depth = 0,
}: {
  group: StatementGroup
  onChange: (g: StatementGroup) => void
  onRemove: () => void
  canRemove: boolean
  routingKeys: Record<string, RoutingKeyConfig>
  depth?: number
}) {
  function addCondition() {
    onChange({ ...group, conditions: [...group.conditions, createCondition(routingKeys)] })
  }

  function addNestedBranch() {
    onChange({ ...group, nested: [...group.nested, createStatementGroup(routingKeys)] })
  }

  return (
    <div className="rounded-lg border border-slate-200 dark:border-[#222733] bg-white dark:bg-[#0f141d]">
      <div className="space-y-0 divide-y divide-slate-100 dark:divide-[#1c1c24]">
        {group.conditions.map((cond, idx) => (
          <div key={cond.id} className="flex items-start gap-2 px-3 py-2.5 flex-wrap">
            {group.conditions.length > 1 && (
              <span className="w-8 shrink-0 text-[10px] font-bold uppercase tracking-widest text-slate-400 select-none mt-1.5">
                {idx === 0 ? 'IF' : 'AND'}
              </span>
            )}
            <ConditionRowEditor
              row={cond}
              routingKeys={routingKeys}
              onChange={(updated) =>
                onChange({ ...group, conditions: group.conditions.map((c) => (c.id === cond.id ? updated : c)) })
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
      </div>

      {/* Nested OR branches — shown only at depth 0 */}
      {depth === 0 && group.nested.length > 0 && (
        <div className="border-t border-slate-100 dark:border-[#1c1c24] px-3 pt-3 pb-2 space-y-2">
          <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-widest text-slate-400">
            <CornerDownRight size={11} />
            Then match any of (nested OR)
          </div>
          {group.nested.map((nestedGroup, nIdx) => (
            <div key={nestedGroup.id} className="pl-3 border-l-2 border-sky-200 dark:border-sky-800">
              {nIdx > 0 && (
                <p className="text-[10px] font-bold text-sky-500 mb-1">OR</p>
              )}
              <ConditionGroupEditor
                group={nestedGroup}
                routingKeys={routingKeys}
                canRemove={true}
                depth={1}
                onChange={(updated) =>
                  onChange({ ...group, nested: group.nested.map((n) => (n.id === nestedGroup.id ? updated : n)) })
                }
                onRemove={() =>
                  onChange({ ...group, nested: group.nested.filter((n) => n.id !== nestedGroup.id) })
                }
              />
            </div>
          ))}
        </div>
      )}

      <div className="flex items-center gap-3 border-t border-slate-100 dark:border-[#1c1c24] px-3 py-2 flex-wrap">
        <button
          type="button"
          onClick={addCondition}
          className="flex items-center gap-1 text-xs text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 transition-colors"
        >
          <Plus size={12} /> Add condition
        </button>
        {depth === 0 && (
          <button
            type="button"
            onClick={addNestedBranch}
            className="flex items-center gap-1 text-xs text-sky-400 hover:text-sky-600 transition-colors"
          >
            <CornerDownRight size={12} /> Add nested branch
          </button>
        )}
        {canRemove && (
          <button
            type="button"
            onClick={onRemove}
            className="ml-auto flex items-center gap-1 text-xs text-red-400 hover:text-red-600 transition-colors"
          >
            <Trash2 size={12} /> Remove group
          </button>
        )}
      </div>
    </div>
  )
}

const OUTPUT_TYPE_LABELS: Record<string, string> = {
  priority: 'Priority',
  volume_split: 'Volume Split',
}

// ---- Rule block ----
function RuleBlockEditor({
  block,
  onChange,
  onRemove,
  routingKeys,
  gatewaySuggestions = [],
  highlightMissingOutput = false,
}: {
  block: RuleBlock
  onChange: (b: RuleBlock) => void
  onRemove: () => void
  routingKeys: Record<string, RoutingKeyConfig>
  gatewaySuggestions?: string[]
  highlightMissingOutput?: boolean
}) {
  const [collapsed, setCollapsed] = useState(false)

  function addGroup() {
    onChange({ ...block, statements: [...block.statements, createStatementGroup(routingKeys)] })
  }

  return (
    <div className="border border-slate-200 dark:border-[#1c1c24] rounded-xl overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-slate-50 dark:bg-[#111118] border-b border-slate-200 dark:border-[#1c1c24]">
        <input
          value={block.name}
          onChange={(e) => onChange({ ...block, name: e.target.value })}
          placeholder="Rule name"
          className="bg-transparent text-sm font-semibold focus:outline-none text-slate-700 dark:text-slate-200 w-full"
        />
        <div className="flex items-center gap-2 ml-2 shrink-0">
          <button type="button" onClick={onRemove} aria-label="Delete rule" className="text-red-400 hover:text-red-600">
            <Trash2 size={14} />
          </button>
          <button
            type="button"
            onClick={() => setCollapsed(!collapsed)}
            aria-label={collapsed ? 'Expand rule' : 'Collapse rule'}
            className="text-slate-400 hover:text-slate-600"
          >
            {collapsed ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
          </button>
        </div>
      </div>

      {!collapsed && (
        <div className="divide-y divide-slate-100 dark:divide-[#1c1c24]">
          {/* IF section */}
          <div className="px-4 py-4 space-y-2">
            <p className="text-[11px] font-semibold uppercase tracking-widest text-slate-400 mb-3">If</p>
            {block.statements.map((group, idx) => (
              <div key={group.id} className="space-y-2">
                {idx > 0 && (
                  <div className="flex items-center gap-3">
                    <span className="h-px flex-1 bg-slate-200 dark:bg-[#222]" />
                    <span className="text-[11px] font-bold uppercase tracking-widest text-sky-500 px-1">or</span>
                    <span className="h-px flex-1 bg-slate-200 dark:bg-[#222]" />
                  </div>
                )}
                <ConditionGroupEditor
                  group={group}
                  routingKeys={routingKeys}
                  canRemove={block.statements.length > 1}
                  onChange={(updated) =>
                    onChange({ ...block, statements: block.statements.map((s) => (s.id === group.id ? updated : s)) })
                  }
                  onRemove={() =>
                    onChange({ ...block, statements: block.statements.filter((s) => s.id !== group.id) })
                  }
                />
              </div>
            ))}
            <button
              type="button"
              onClick={addGroup}
              className="flex items-center gap-1 text-xs text-sky-500 hover:text-sky-600 font-medium transition-colors mt-1"
            >
              <Plus size={12} /> Add OR group
            </button>
          </div>

          {/* THEN section */}
          <div className="px-4 py-4">
            <div className="flex items-center gap-3 mb-3">
              <p className="text-[11px] font-semibold uppercase tracking-widest text-slate-400 shrink-0">Then route</p>
              <div className="flex rounded-lg border border-slate-200 dark:border-[#222226] overflow-hidden text-[11px]">
                {Object.keys(OUTPUT_TYPE_LABELS).map((type) => (
                  <button
                    key={type}
                    type="button"
                    onClick={() => onChange({ ...block, outputType: type as RuleBlock['outputType'] })}
                    className={`px-2.5 py-1 transition-colors ${
                      block.outputType === type
                        ? 'bg-brand-500 text-white font-semibold'
                        : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-[#1c1c24]'
                    }`}
                  >
                    {OUTPUT_TYPE_LABELS[type]}
                  </button>
                ))}
              </div>
            </div>
            {block.outputType === 'priority' && (
              <PriorityEditor
                gateways={block.priorityGateways}
                suggestions={gatewaySuggestions}
                highlightMissing={highlightMissingOutput}
                onChange={(gws) => onChange({ ...block, priorityGateways: gws })}
              />
            )}
            {block.outputType === 'volume_split' && (
              <VolumeSplitEditor
                entries={block.volumeSplitEntries}
                suggestions={gatewaySuggestions}
                onChange={(entries) => onChange({ ...block, volumeSplitEntries: entries })}
              />
            )}
            {block.outputType === 'volume_split_priority' && (
              <VolumeSplitPriorityEditor
                entries={block.volumeSplitPriorityEntries}
                suggestions={gatewaySuggestions}
                onChange={(entries) => onChange({ ...block, volumeSplitPriorityEntries: entries })}
              />
            )}
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
    const scalarValue = Array.isArray(c.value) ? (c.value[0] ?? '') : c.value

    if (isMulti && Array.isArray(c.value)) {
      return {
        lhs: c.lhs,
        comparison: OPERATOR_TO_API[c.operator],
        value: { type: 'enum_variant_array', value: c.value },
        metadata: {},
      }
    }

    if (keyType === 'udf') {
      const metadataKey = c.metadataKey?.trim() ?? ''
      return {
        lhs: c.lhs,
        comparison: OPERATOR_TO_API[c.operator] || c.operator,
        value: {
          type: 'metadata_variant',
          value: { key: metadataKey, value: scalarValue },
        },
        metadata: {},
      }
    }

    const apiValueType =
      keyType === 'integer' ? 'number' :
      keyType === 'global_ref' ? 'global_ref' :
      keyType === 'str_value' ? 'str_value' :
      'enum_variant'
    return {
      lhs: c.lhs,
      comparison: OPERATOR_TO_API[c.operator] || c.operator,
      value: {
        type: apiValueType,
        value: keyType === 'integer' ? Number(scalarValue) : scalarValue,
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

function findIncompleteUdfCondition(
  rules: RuleBlock[],
  routingKeys: Record<string, RoutingKeyConfig>
): string | null {
  for (const rule of rules) {
    const visitGroup = (group: StatementGroup): string | null => {
      for (const condition of group.conditions) {
        if (routingKeys[condition.lhs]?.type !== 'udf') continue
        const metadataKey = condition.metadataKey?.trim()
        const metadataValue = Array.isArray(condition.value) ? '' : condition.value.trim()
        if (!metadataKey) {
          return `${rule.name}: add metadata key for ${toLabel(condition.lhs)}.`
        }
        if (!metadataValue) {
          return `${rule.name}: add metadata value for ${toLabel(condition.lhs)}.`
        }
      }
      for (const nested of group.nested) {
        const nestedError = visitGroup(nested)
        if (nestedError) return nestedError
      }
      return null
    }

    for (const statement of rule.statements) {
      const error = visitGroup(statement)
      if (error) return error
    }
  }

  return null
}

function findIncompleteRuleOutput(rules: RuleBlock[]): RuleOutputValidationError | null {
  for (const rule of rules) {
    if (rule.outputType === 'priority') {
      if (!rule.priorityGateways.some((gateway) => gateway.gatewayName.trim())) {
        return { ruleId: rule.id, message: `${rule.name}: add at least one priority gateway.` }
      }
      continue
    }

    if (rule.outputType === 'volume_split') {
      if (!rule.volumeSplitEntries.some((entry) => entry.gatewayName.trim())) {
        return { ruleId: rule.id, message: `${rule.name}: add at least one volume split gateway.` }
      }
      continue
    }

    if (rule.outputType === 'volume_split_priority') {
      const hasGateway = rule.volumeSplitPriorityEntries.some((entry) =>
        entry.gateways.some((gateway) => gateway.gatewayName.trim())
      )
      if (!hasGateway) {
        return { ruleId: rule.id, message: `${rule.name}: add at least one split priority gateway.` }
      }
    }
  }

  return null
}

// ---- Reverse-parse API → RuleBlocks ----
const API_OPERATOR_TO_UI: Record<string, string> = {
  equal: '==', not_equal: '!=',
  greater_than: '>', less_than: '<',
  greater_than_equal: '>=', less_than_equal: '<=',
}

function parseAlgorithmToRuleBlocks(algo: RoutingAlgorithm): { ruleBlocks: RuleBlock[]; defaultOutput: DefaultOutput } {
  const algorithm = algo.algorithm_data || algo.algorithm
  const data = algorithm?.data as EuclidAlgorithmData | undefined
  if (!data) return { ruleBlocks: [], defaultOutput: { priorityGateways: [] } }

  function parseGateways(arr: { gateway_name: string; gateway_id?: string | null }[]): GatewayEntry[] {
    return arr.map((g) => ({ id: crypto.randomUUID(), gatewayName: g.gateway_name, gatewayId: g.gateway_id ?? '' }))
  }

  function parseCondition(cond: { lhs: string; comparison: string; value: { type: string; value: unknown } }): ConditionRow {
    const isArray = cond.value?.type === 'enum_variant_array'
    let operator = API_OPERATOR_TO_UI[cond.comparison] ?? cond.comparison
    if (isArray && cond.comparison === 'equal') operator = 'in'
    if (isArray && cond.comparison === 'not_equal') operator = 'not_in'
    if (cond.value?.type === 'metadata_variant' && typeof cond.value.value === 'object' && cond.value.value !== null) {
      const metadata = cond.value.value as { key?: unknown; value?: unknown }
      return {
        id: crypto.randomUUID(),
        lhs: String(cond.lhs),
        operator,
        value: String(metadata.value ?? ''),
        metadataKey: String(metadata.key ?? ''),
      }
    }
    const value = isArray
      ? (Array.isArray(cond.value?.value) ? (cond.value.value as string[]) : [String(cond.value?.value)])
      : String(cond.value?.value ?? '')
    return { id: crypto.randomUUID(), lhs: String(cond.lhs), operator, value }
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
  const [defaultOutput, setDefaultOutput] = useState<DefaultOutput>({ priorityGateways: [] })
  const [editorMode, setEditorMode] = useState<'visual' | 'code'>('visual')
  const [codeText, setCodeText] = useState('')
  const [codeParseError, setCodeParseError] = useState<string | null>(null)
  const [showJson, setShowJson] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [missingOutputRuleId, setMissingOutputRuleId] = useState<string | null>(null)
  const [createdId, setCreatedId] = useState<string | null>(null)
  const [activating, setActivating] = useState(false)
  const [activateError, setActivateError] = useState<string | null>(null)
  const [activateSuccess, setActivateSuccess] = useState(false)
  const [deactivatingId, setDeactivatingId] = useState<string | null>(null)
  const [deactivateError, setDeactivateError] = useState<string | null>(null)
  const [deactivateSuccess, setDeactivateSuccess] = useState(false)
  const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set())
  const [pendingActivateId, setPendingActivateId] = useState<string | null>(null)
  const [pendingDeactivateId, setPendingDeactivateId] = useState<string | null>(null)
  const builderRef = useRef<HTMLDivElement>(null)
  const [openMenuId, setOpenMenuId] = useState<string | null>(null)

  function handleClone(source: RoutingAlgorithm) {
    const { ruleBlocks: clonedBlocks, defaultOutput: clonedDefault } = parseAlgorithmToRuleBlocks(source)
    setRuleName(`copy-of-${source.name}`)
    setRuleDesc(source.description && source.description !== 'N/A' ? source.description : '')
    setRuleBlocks(clonedBlocks)
    setDefaultOutput(clonedDefault)
    setEditorMode('visual')
    setCreatedId(null)
    setSubmitError(null)
    builderRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  const { data: allAlgorithms, mutate: mutateAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )

  const { data: activeAlgorithms, mutate: mutateActiveAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const activeIds = new Set((activeAlgorithms || []).map((a) => a.id))
  const activeVolumeAlgorithm = (activeAlgorithms || []).find(
    (a) => (a.algorithm_data || a.algorithm)?.type === 'volume_split'
  )
  const ruleAlgorithms = (allAlgorithms || [])
    .filter((algo) => {
      const algorithm = algo.algorithm_data || algo.algorithm
      return algorithm?.type !== 'volume_split'
    })
    .sort((a, b) => new Date(b.created_at ?? 0).getTime() - new Date(a.created_at ?? 0).getTime())

  const gatewaySuggestions = Array.from(new Set([
    ...ruleBlocks.flatMap((b) => [
      ...b.priorityGateways.map((g) => g.gatewayName),
      ...b.volumeSplitEntries.map((e) => e.gatewayName),
      ...b.volumeSplitPriorityEntries.flatMap((e) => e.gateways.map((g) => g.gatewayName)),
    ]),
    ...defaultOutput.priorityGateways.map((g) => g.gatewayName),
  ].filter(Boolean)))

  const algorithmData = buildAlgorithmData(ruleBlocks, defaultOutput, routingKeys)

  function resetConfigurator() {
    setRuleName('')
    setRuleDesc('')
    setRuleBlocks([])
    setDefaultOutput({ priorityGateways: [] })
    setEditorMode('visual')
    setCodeText('')
    setCodeParseError(null)
    setSubmitError(null)
    setMissingOutputRuleId(null)
    setShowJson(false)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!merchantId) { setSubmitError('Set a Merchant ID first.'); return }
    if (routingKeysUnavailable) {
      setSubmitError('Routing key config is unavailable. Ensure backend /config/routing-keys is reachable and valid.')
      return
    }
    if (!ruleName.trim()) { setSubmitError('Rule name is required.'); return }
    if (codeParseError) { setSubmitError(`Fix syntax error: ${codeParseError}`); return }
    const udfError = findIncompleteUdfCondition(ruleBlocks, routingKeys)
    if (udfError) { setSubmitError(udfError); return }
    const outputError = findIncompleteRuleOutput(ruleBlocks)
    if (outputError) {
      setSubmitError(outputError.message)
      setMissingOutputRuleId(outputError.ruleId)
      return
    }
    setSubmitting(true)
    setSubmitError(null)
    setMissingOutputRuleId(null)
    setCreatedId(null)
    try {
      const nextRuleName = ruleName.trim()
      const result = await apiPost<RoutingAlgorithm>('/routing/create', {
        name: nextRuleName,
        description: ruleDesc,
        created_by: merchantId,
        algorithm_for: 'payment',
        algorithm: { type: 'advanced', data: algorithmData },
      })
      setCreatedId(result.rule_id ?? result.id)
      resetConfigurator()
      mutateAlgorithms()
    } catch (err) {
      setSubmitError(String(err))
    } finally {
      setSubmitting(false)
    }
  }

  async function handleActivate(id: string) {
    if (!merchantId) return
    if (activeVolumeAlgorithm) {
      setPendingActivateId(id)
      return
    }
    await doActivate(id)
  }

  async function doActivate(id: string) {
    setActivating(true)
    setActivateError(null)
    setActivateSuccess(false)
    setDeactivateError(null)
    setDeactivateSuccess(false)
    try {
      await apiPost('/routing/activate', { created_by: merchantId, routing_algorithm_id: id })
      setActivateSuccess(true)
      await Promise.all([mutateAlgorithms(), mutateActiveAlgorithms()])
    } catch (err) {
      setActivateError(String(err))
    } finally {
      setActivating(false)
    }
  }

  async function handleDeactivate(id: string) {
    if (!merchantId) return
    setPendingDeactivateId(id)
  }

  async function doDeactivate(id: string) {
    setDeactivatingId(id)
    setDeactivateError(null)
    setDeactivateSuccess(false)
    setActivateError(null)
    setActivateSuccess(false)
    try {
      await apiPost('/routing/deactivate', { created_by: merchantId, routing_algorithm_id: id })
      setDeactivateSuccess(true)
      await Promise.all([mutateAlgorithms(), mutateActiveAlgorithms()])
    } catch (err) {
      setDeactivateError(String(err))
    } finally {
      setDeactivatingId(null)
    }
  }

  function toggleRuleExpand(id: string) {
    setExpandedRuleIds(prev => {
      const newSet = new Set(prev)
      if (newSet.has(id)) { newSet.delete(id) } else { newSet.add(id) }
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
        outputType: 'priority',
        priorityGateways: [],
        volumeSplitEntries: [],
        volumeSplitPriorityEntries: [],
      },
    ])
  }

function switchToCode() {
    if (ruleBlocks.length > 0) {
      setCodeText(serializeToDSL(ruleBlocks))
    } else if (!codeText.trim()) {
      setCodeText(CODE_EDITOR_PLACEHOLDER)
    }
    setCodeParseError(null)
    setEditorMode('code')
  }

  function switchToVisual() {
    setEditorMode('visual')
  }

  function handleCodeChange(text: string) {
    setCodeText(text)
    if (!text.trim()) { setCodeParseError(null); setRuleBlocks([]); return }
    const result = parseDSL(text)
    if (result.error) {
      setCodeParseError(result.error)
      setRuleBlocks([])
    } else if (result.rules !== null) {
      setCodeParseError(null)
      setRuleBlocks(result.rules)
    }
  }

  return (
    <div className="space-y-6">
      <ConfirmDialog
        open={pendingActivateId !== null}
        title="Switch to Rule-Based Routing?"
        description={`"${activeVolumeAlgorithm?.name}" (Volume Split) is currently active. Activating this rule will replace it.`}
        confirmLabel="Yes, activate"
        variant="primary"
        onConfirm={() => { const id = pendingActivateId!; setPendingActivateId(null); doActivate(id) }}
        onCancel={() => setPendingActivateId(null)}
      />
      <ConfirmDialog
        open={pendingDeactivateId !== null}
        title="Deactivate this rule?"
        description="The rule will be deactivated for this merchant. It will remain saved and can be reactivated at any time."
        confirmLabel="Deactivate"
        variant="danger"
        onConfirm={() => { const id = pendingDeactivateId!; setPendingDeactivateId(null); doDeactivate(id) }}
        onCancel={() => setPendingDeactivateId(null)}
      />
      <div>
        <h1 className="text-2xl font-semibold text-slate-900">Rule-Based Routing</h1>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Rule list */}
        <div className="lg:col-span-1 space-y-3">
          <Card>
            <CardHeader>
              <h2 className="text-sm font-semibold text-slate-800">Existing Rules</h2>
            </CardHeader>
            <div>
              {!merchantId ? (
                <p className="px-4 py-3 text-sm text-slate-400">Set merchant ID to load rules.</p>
              ) : !allAlgorithms ? (
                <p className="px-4 py-3 text-sm text-slate-400">Loading...</p>
              ) : ruleAlgorithms.length === 0 ? (
                <p className="px-4 py-3 text-sm text-slate-400">No rule-based rules yet.</p>
              ) : (
                <div>
                  {ruleAlgorithms.map((algo) => {
                    const isActive = activeIds.has(algo.id)
                    const isExpanded = expandedRuleIds.has(algo.id)

                    return (
                      <div
                        key={algo.id}
                        className={`border-b border-slate-100 dark:border-[#1e2330] last:border-b-0 transition-colors ${
                          isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : ''
                        }`}
                      >
                        <div className="px-5 py-4">
                          {/* Row 1: name + id (left) | action buttons (right) */}
                          <div className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-3">
                            <div className="min-w-0 overflow-hidden">
                              <button
                                type="button"
                                onClick={() => toggleRuleExpand(algo.id)}
                                className="w-full min-w-0 rounded-lg text-left group focus:outline-none focus-visible:outline-none focus-visible:ring-0"
                              >
                                <div className="flex min-w-0 items-center gap-1.5">
                                  <p className={`min-w-0 truncate font-medium group-hover:text-brand-600 dark:group-hover:text-brand-400 transition-colors ${
                                    isActive ? 'text-emerald-900 dark:text-emerald-100' : 'text-slate-900 dark:text-white'
                                  }`}>
                                    {algo.name}
                                  </p>
                                  {isActive && (
                                    <span className="shrink-0 inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400">
                                      ● Active
                                    </span>
                                  )}
                                  {isExpanded
                                    ? <ChevronUp size={12} className="text-slate-400 shrink-0 ml-auto" />
                                    : <ChevronDown size={12} className="text-slate-400 shrink-0 ml-auto" />
                                  }
                                </div>
                              </button>
                              <div className="mt-1 flex min-w-0 items-center gap-1">
                                <span className="block min-w-0 truncate text-[10px] font-mono leading-4 text-slate-500 dark:text-[#607087]">
                                  {algo.id}
                                </span>
                                <CopyButton text={algo.id} size={10} label="Copy routing ID" />
                              </div>
                            </div>

                            <div className="flex shrink-0 items-center gap-1.5">
                              {!isActive ? (
                                <button
                                  type="button"
                                  onClick={() => handleActivate(algo.id)}
                                  disabled={activating}
                                  className="group/badge relative inline-flex items-center justify-center min-w-[68px] px-2.5 py-0.5 rounded-full text-xs font-medium border transition-colors duration-150
                                    bg-slate-100 text-slate-500 border-slate-200 hover:bg-brand-50 hover:text-brand-600 hover:border-brand-200
                                    dark:bg-[#1a1f2a] dark:text-[#8090a8] dark:border-[#2a3040] dark:hover:bg-brand-900/20 dark:hover:text-brand-400 dark:hover:border-brand-800
                                    disabled:opacity-50 disabled:cursor-not-allowed"
                                >
                                  <span className="transition-opacity duration-150 group-hover/badge:opacity-0">Inactive</span>
                                  <span className="absolute inset-0 flex items-center justify-center gap-1 opacity-0 transition-opacity duration-150 group-hover/badge:opacity-100">
                                    <Plus size={10} /> Activate
                                  </span>
                                </button>
                              ) : (
                                <button
                                  type="button"
                                  onClick={() => handleDeactivate(algo.id)}
                                  disabled={deactivatingId === algo.id}
                                  className="inline-flex h-8 items-center justify-center gap-1.5 rounded-full border border-red-500/30 bg-red-500/10 px-3 text-xs font-semibold text-red-600 transition-colors hover:bg-red-500/15 disabled:cursor-not-allowed disabled:opacity-50 dark:border-red-500/35 dark:bg-red-500/15 dark:text-red-400 dark:hover:bg-red-500/20"
                                >
                                  <PowerOff size={13} />
                                  {deactivatingId === algo.id ? 'Deactivating…' : 'Deactivate'}
                                </button>
                              )}
                              <div className="relative">
                                <button
                                  type="button"
                                  onClick={(e) => { e.stopPropagation(); setOpenMenuId(openMenuId === algo.id ? null : algo.id) }}
                                  className="p-1 rounded-md text-slate-400 hover:text-slate-600 hover:bg-slate-100 dark:hover:text-slate-300 dark:hover:bg-[#1c1c24] transition-colors"
                                >
                                  <MoreVertical size={14} />
                                </button>
                                {openMenuId === algo.id && (
                                  <>
                                    <div className="fixed inset-0 z-10" onClick={() => setOpenMenuId(null)} />
                                    <div className="absolute right-0 top-full mt-1 z-20 min-w-[150px] rounded-lg border border-slate-200 bg-white shadow-lg dark:border-[#2a303a] dark:bg-[#11151d] py-1 overflow-hidden">
                                      <button
                                        type="button"
                                        onClick={() => { handleClone(algo); setOpenMenuId(null) }}
                                        className="flex w-full items-center gap-2 px-3 py-2 text-xs text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-[#1c2030] transition-colors"
                                      >
                                        <CopyPlus size={13} className="text-brand-500" />
                                        Clone to builder
                                      </button>
                                    </div>
                                  </>
                                )}
                              </div>
                            </div>
                          </div>

                          {/* Row 2: description (left) | date (right) — same row */}
                          {(algo.description && algo.description !== 'N/A' || algo.created_at) && (
                            <div className="flex items-center justify-between gap-2 mt-0.5">
                              {algo.description && algo.description !== 'N/A'
                                ? <p className="text-[11px] text-slate-500 dark:text-[#6d7a8d] truncate min-w-0">{algo.description}</p>
                                : <span />
                              }
                              {algo.created_at && (
                                <span className="shrink-0 text-[10px] text-slate-400 dark:text-[#4e5870] pr-[12px]">
                                  {new Date(algo.created_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}
                                </span>
                              )}
                            </div>
                          )}
                        </div>

                        {isExpanded && (
                          <div className="border-t border-slate-100 dark:border-[#1e2330] bg-slate-50/60 dark:bg-[#0c0f17] px-6 py-3">
                            {algo.description && algo.description !== 'N/A' && (
                              <p className="mb-3 text-xs text-slate-500 dark:text-[#6d7a8d]">{algo.description}</p>
                            )}
                            <RuleBreakdown algo={algo} />
                          </div>
                        )}
                      </div>
                    )
                  })}
                </div>
              )}
            </div>
          </Card>
          {activeVolumeAlgorithm && (
            <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
              <strong>Volume Split is active</strong> — activating a rule-based rule will automatically deactivate it.
            </div>
          )}
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
        <div ref={builderRef} className="lg:col-span-2 space-y-4">
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
                  <div className="flex items-center justify-between">
                    <p className="text-xs font-medium text-slate-500 uppercase tracking-wide">Rules</p>
                    <div className="flex rounded-lg border border-slate-200 dark:border-[#222226] overflow-hidden text-[11px]">
                      <button
                        type="button"
                        onClick={switchToVisual}
                        className={`px-3 py-1 transition-colors ${editorMode === 'visual' ? 'bg-brand-500 text-white font-semibold' : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-[#1c1c24]'}`}
                      >
                        Visual
                      </button>
                      <button
                        type="button"
                        onClick={switchToCode}
                        className={`px-3 py-1 transition-colors ${editorMode === 'code' ? 'bg-brand-500 text-white font-semibold' : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-[#1c1c24]'}`}
                      >
                        Code
                      </button>
                    </div>
                  </div>
                  {routingKeysLoading && (
                    <p className="text-sm text-slate-500">Loading routing keys from backend...</p>
                  )}
                  {routingKeysUnavailable && (
                    <ErrorMessage error="Routing keys are unavailable from backend (/config/routing-keys). Rule Builder is disabled until this is fixed." />
                  )}
                  {editorMode === 'visual' ? (
                    <>
                      {ruleBlocks.map((block) => (
                        <RuleBlockEditor
                          key={block.id}
                          block={block}
                          routingKeys={routingKeys}
                          gatewaySuggestions={gatewaySuggestions}
                          highlightMissingOutput={missingOutputRuleId === block.id}
                          onChange={(updated) => {
                            if (missingOutputRuleId === block.id) setMissingOutputRuleId(null)
                            setRuleBlocks((prev) => prev.map((b) => (b.id === block.id ? updated : b)))
                          }}
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
                        <Plus size={14} /> Add Rule
                      </Button>
                    </>
                  ) : (
                    <RuleCodeEditor
                      value={codeText}
                      onChange={handleCodeChange}
                      parseError={codeParseError}
                      routingKeys={routingKeys}
                      gatewaySuggestions={gatewaySuggestions}
                    />
                  )}
                </div>

                {/* Default selection */}
                <div className="border border-slate-200 dark:border-[#1c1c24] rounded-xl px-4 py-3">
                  <p className="text-xs font-medium text-slate-500 mb-1">Default Fallback</p>
                  <p className="mb-3 text-xs text-slate-400 dark:text-[#8d96a8]">
                    Used when no rule matches. Per-request overrides are possible via <code className="font-mono">fallback_output</code>.
                  </p>
                  <PriorityEditor
                    gateways={defaultOutput.priorityGateways}
                    suggestions={gatewaySuggestions}
                    onChange={(gws) => setDefaultOutput({ ...defaultOutput, priorityGateways: gws })}
                  />
                </div>

                <ErrorMessage error={submitError} />
                {createdId && (
                  <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-200">
                    <span className="min-w-0">
                      Rule created: <span className="font-mono">{createdId}</span>
                    </span>
                    <Button type="button" size="sm" onClick={() => handleActivate(createdId)} disabled={activating}>
                      Activate Now
                    </Button>
                  </div>
                )}
                <div className="flex gap-3">
                  <Button type="submit" disabled={submitting || routingKeysUnavailable}>
                    {submitting ? 'Creating...' : 'Create Rule'}
                  </Button>
                  <Button type="button" variant="secondary" size="sm" onClick={() => setShowJson(!showJson)}>
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
                <pre className="max-h-64 overflow-auto rounded-lg border border-slate-200/80 bg-slate-50/90 p-4 font-mono text-xs leading-6 text-slate-800 shadow-[inset_0_1px_0_rgba(255,255,255,0.75),0_16px_30px_-28px_rgba(15,23,42,0.18)] dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef] dark:shadow-none">
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
