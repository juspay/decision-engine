import { useRef, useState } from 'react'
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
import { SearchableSelect } from '../ui/SearchableSelect'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { useDynamicRoutingConfig, RoutingKeyConfig } from '../../hooks/useDynamicRoutingConfig'
import { EuclidAlgorithmData } from '../../types/api'
import { Plus, Trash2, GripVertical, ChevronDown, ChevronUp, Eye, PowerOff, CornerDownRight } from 'lucide-react'

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

// ---- Types ----
interface GatewayEntry {
  id: string
  gatewayName: string
  gatewayId: string
}

interface ConditionRow {
  id: string
  lhs: string
  operator: string
  value: string | string[]
}

interface StatementGroup {
  id: string
  conditions: ConditionRow[]
  nested: StatementGroup[]
}

interface VolumeSplitEntry {
  id: string
  split: number
  gatewayName: string
  gatewayId: string
}

interface VolumeSplitPriorityEntry {
  id: string
  split: number
  gateways: GatewayEntry[]
}

interface RuleBlock {
  id: string
  name: string
  statements: StatementGroup[]
  outputType: 'priority' | 'volume_split' | 'volume_split_priority'
  priorityGateways: GatewayEntry[]
  volumeSplitEntries: VolumeSplitEntry[]
  volumeSplitPriorityEntries: VolumeSplitPriorityEntry[]
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

function getRuleSummary(algo: RoutingAlgorithm): string {
  const algorithm = algo.algorithm_data || algo.algorithm
  const rules = (algorithm?.data as EuclidAlgorithmData | undefined)?.rules
  if (!rules || rules.length === 0) return ''
  const first = rules[0]
  const cond = first?.statements?.[0]?.condition?.[0]
  if (!cond) return ''
  const field = toLabel(String(cond.lhs ?? ''))
  const op = String(cond.comparison ?? '').replace(/_/g, ' ')
  const val = toLabel(String(cond.value?.value ?? ''))
  const summary = [field, op, val].filter(Boolean).join(' ')
  return rules.length > 1 ? `${summary} · +${rules.length - 1} more` : summary
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
}: {
  gateways: GatewayEntry[]
  onChange: (gws: GatewayEntry[]) => void
  suggestions?: string[]
}) {
  const [newGatewayName, setNewGatewayName] = useState('')
  const [newGatewayId, setNewGatewayId] = useState('')
  const listId = useRef(`gateway-suggestions-${Math.random().toString(36).slice(2)}`).current
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
          value={newGatewayName}
          onChange={(e) => setNewGatewayName(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          list={listId}
          placeholder="Gateway name"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm flex-1 focus:outline-none focus:ring-1 focus:ring-brand-500"
        />
        <input
          value={newGatewayId}
          onChange={(e) => setNewGatewayId(e.target.value)}
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
  const isStr = keyInfo?.type === 'str_value' || keyInfo?.type === 'udf'
  const isMulti = row.operator === 'in' || row.operator === 'not_in'

  const operators = isInt
    ? ['>', '<', '>=', '<=', '==', '!=']
    : isEnum
    ? ['==', '!=', 'in', 'not_in']
    : ['==', '!=']

  const selectedValues = Array.isArray(row.value) ? row.value : []

  function toggleEnumValue(v: string) {
    const updated = selectedValues.includes(v)
      ? selectedValues.filter((x) => x !== v)
      : [...selectedValues, v]
    onChange({ ...row, value: updated })
  }

  function handleOperatorChange(op: string) {
    const switchingToMulti = op === 'in' || op === 'not_in'
    const switchingFromMulti = row.operator === 'in' || row.operator === 'not_in'
    let newValue: string | string[] = row.value
    if (switchingToMulti && !Array.isArray(row.value)) {
      newValue = row.value ? [row.value] : []
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
          onChange({ ...row, lhs: newKey, value: defaultValue, operator: '==' })
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
        <div className="flex flex-wrap gap-x-3 gap-y-1 rounded-lg border border-slate-200 dark:border-[#222226] px-2 py-1.5 min-w-[8rem]">
          {(keyInfo?.values || []).map((v: string) => (
            <label key={v} className="flex items-center gap-1 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={selectedValues.includes(v)}
                onChange={() => toggleEnumValue(v)}
                className="accent-brand-500"
              />
              <span className="text-xs">{toLabel(v)}</span>
            </label>
          ))}
          {(keyInfo?.values || []).length === 0 && (
            <span className="text-xs text-slate-400">No enum values</span>
          )}
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

const OUTPUT_TYPE_LABELS: Record<RuleBlock['outputType'], string> = {
  priority: 'Priority',
  volume_split: 'Volume Split',
  volume_split_priority: 'Split + Priority',
}

// ---- Rule block ----
function RuleBlockEditor({
  block,
  onChange,
  onRemove,
  routingKeys,
  gatewaySuggestions = [],
}: {
  block: RuleBlock
  onChange: (b: RuleBlock) => void
  onRemove: () => void
  routingKeys: Record<string, RoutingKeyConfig>
  gatewaySuggestions?: string[]
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
                {(Object.keys(OUTPUT_TYPE_LABELS) as RuleBlock['outputType'][]).map((type) => (
                  <button
                    key={type}
                    type="button"
                    onClick={() => onChange({ ...block, outputType: type })}
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

    if (isMulti && Array.isArray(c.value)) {
      return {
        lhs: c.lhs,
        comparison: OPERATOR_TO_API[c.operator],
        value: { type: 'enum_variant_array', value: c.value },
        metadata: {},
      }
    }

    const apiValueType =
      keyType === 'integer' ? 'number' :
      keyType === 'str_value' || keyType === 'udf' ? 'str_value' :
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

  const gatewaySuggestions = Array.from(new Set([
    ...ruleBlocks.flatMap((b) => [
      ...b.priorityGateways.map((g) => g.gatewayName),
      ...b.volumeSplitEntries.map((e) => e.gatewayName),
      ...b.volumeSplitPriorityEntries.flatMap((e) => e.gateways.map((g) => g.gatewayName)),
    ]),
    ...defaultOutput.priorityGateways.map((g) => g.gatewayName),
  ].filter(Boolean)))

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
      setCreatedId(result.rule_id ?? result.id)
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
    if (!window.confirm('Deactivate this routing rule for the selected merchant? The saved rule will remain available.')) {
      return
    }
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

  return (
    <div className="space-y-6">
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
                    const summary = getRuleSummary(algo)

                    return (
                      <div key={algo.id}>
                        <div className="flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-start sm:justify-between">
                          <div className="min-w-0 flex-1">
                            <p className="truncate font-medium">{algo.name}</p>
                            {summary && (
                              <p className="text-xs text-slate-400 mt-0.5 truncate" title={summary}>{summary}</p>
                            )}
                          </div>

                          <div className="flex shrink-0 flex-wrap items-center gap-2 sm:justify-end">
                            <Badge variant={isActive ? 'green' : 'gray'}>
                              {isActive ? 'Active' : 'Inactive'}
                            </Badge>
                            <Button size="sm" variant="ghost" onClick={() => toggleRuleExpand(algo.id)}>
                              <Eye size={14} className="mr-1" />
                              {isExpanded ? 'Hide' : 'View'}
                            </Button>
                            {!isActive && (
                              <Button size="sm" variant="ghost" onClick={() => handleActivate(algo.id)} disabled={activating}>
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
                      gatewaySuggestions={gatewaySuggestions}
                      onChange={(updated) =>
                        setRuleBlocks((prev) => prev.map((b) => (b.id === block.id ? updated : b)))
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
                    <Plus size={14} /> Add Rule
                  </Button>
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
                  <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-400 flex items-center justify-between">
                    <span>Rule created (ID: {createdId})</span>
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
