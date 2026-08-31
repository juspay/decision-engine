import { useEffect, useState } from 'react'
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
import { Plus, Trash2, GripVertical, CornerDownRight, ChevronDown, ChevronUp, Pencil, Check, X } from 'lucide-react'
import { Button } from '../../ui/Button'
import { FieldError } from '../../ui/FieldError'
import { SearchableSelect } from '../../ui/SearchableSelect'
import { SearchableMultiSelect } from '../../ui/SearchableMultiSelect'
import { GatewaySelect } from '../../ui/GatewaySelect'
import { gatewayOptions, type GatewayOption } from '../../../lib/connectors'
import { RoutingKeyConfig } from '../../../hooks/useDynamicRoutingConfig'
import type {
  RuleBlock, StatementGroup, ConditionRow,
  GatewayEntry, VolumeSplitEntry, VolumeSplitPriorityEntry,
} from '../../ui/RuleCodeEditor'
import {
  OPERATOR_LABELS, toLabel, createCondition, createStatementGroup,
} from '../../../features/routing/euclid/state'

// ---- Sortable gateway item ----
export function SortableGatewayItem({
  id,
  position,
  gatewayName,
  gatewayId,
  options,
  onEdit,
  onRemove,
}: {
  id: string
  position: number
  gatewayName: string
  gatewayId: string
  options: GatewayOption[]
  onEdit: (next: { gatewayName: string; gatewayId: string }) => void
  onRemove: () => void
}) {
  const [editing, setEditing] = useState(false)
  const [draftName, setDraftName] = useState(gatewayName)
  const [draftId, setDraftId] = useState(gatewayId)

  // Dragging a row while its inputs are open would fight the pointer for the same gestures.
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id, disabled: editing })
  const style = { transform: CSS.Transform.toString(transform), transition }

  function startEditing() {
    setDraftName(gatewayName)
    setDraftId(gatewayId)
    setEditing(true)
  }

  function commit() {
    const name = draftName.trim()
    // An empty name would leave a row that routes nowhere, so stay in edit mode until it has one.
    if (!name) return
    onEdit({ gatewayName: name, gatewayId: draftId.trim() })
    setEditing(false)
  }

  function cancel() {
    setEditing(false)
  }

  if (editing) {
    return (
      <div
        ref={setNodeRef}
        style={style}
        className="flex items-center gap-2 rounded-lg border border-brand-300 bg-white px-2 py-1.5 dark:border-brand-500/50 dark:bg-[#111118]"
        onKeyDown={(e) => {
          if (e.key === 'Escape') { e.preventDefault(); cancel() }
          if (e.key === 'Enter') { e.preventDefault(); commit() }
        }}
      >
        <span className="w-5 shrink-0 text-center text-sm tabular-nums text-slate-500">{position}.</span>
        <GatewaySelect
          value={draftName}
          onChange={(name, option) => {
            setDraftName(name)
            // The id belongs to the picked connector; a hand-typed name invalidates it.
            setDraftId(option?.gatewayId ?? '')
          }}
          onEnter={commit}
          options={options}
          className="flex-1"
        />
        <input
          value={draftId}
          onChange={(e) => setDraftId(e.target.value)}
          placeholder="Gateway ID (optional)"
          className="flex-1 rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none focus:border-brand-500 dark:border-[#222226]"
        />
        <button
          type="button"
          onClick={commit}
          disabled={!draftName.trim()}
          aria-label="Save gateway"
          className="text-emerald-700 transition-colors hover:text-emerald-700 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <Check size={14} />
        </button>
        <button
          type="button"
          onClick={cancel}
          aria-label="Cancel edit"
          className="text-slate-500 transition-colors hover:text-slate-600"
        >
          <X size={14} />
        </button>
      </div>
    )
  }

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex items-center gap-2 bg-slate-100 dark:bg-[#111118] border border-slate-200 dark:border-[#1c1c24] rounded-lg px-2 py-1.5"
    >
      <span {...attributes} {...listeners} className="cursor-grab text-slate-500">
        <GripVertical size={14} />
      </span>
      <button
        type="button"
        onClick={startEditing}
        aria-label={`Edit ${gatewayName}`}
        className="group flex min-w-0 flex-1 items-center gap-2 text-left hover:text-brand-600 dark:hover:text-brand-400"
      >
        <span className="truncate font-mono text-sm">
          {position}. {gatewayName}{gatewayId ? ` (${gatewayId})` : ''}
        </span>
        <Pencil size={12} className="shrink-0 text-slate-500 transition-colors group-hover:text-brand-600 dark:group-hover:text-brand-400" />
      </button>
      <button type="button" onClick={onRemove} aria-label={`Remove ${gatewayName}`} className="text-red-600 hover:text-red-600">
        <Trash2 size={12} />
      </button>
    </div>
  )
}

// ---- Priority output editor ----
export function PriorityEditor({
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

  const options = gatewayOptions(
    suggestions,
    gateways.map((g) => g.gatewayName),
    gateways.map((g) => g.gatewayId),
  )

  return (
    <div className="space-y-2">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={gateways.map((g) => g.id)} strategy={verticalListSortingStrategy}>
          {gateways.map((gw, idx) => (
            <SortableGatewayItem
              key={gw.id}
              id={gw.id}
              position={idx + 1}
              gatewayName={gw.gatewayName}
              gatewayId={gw.gatewayId}
              // The row's own gateway stays selectable while editing it; only the others are taken.
              options={gatewayOptions(
                suggestions,
                gateways.filter((g) => g.id !== gw.id).map((g) => g.gatewayName),
                gateways.filter((g) => g.id !== gw.id).map((g) => g.gatewayId),
              )}
              onEdit={(next) =>
                onChange(gateways.map((g) => (g.id === gw.id ? { ...g, ...next } : g)))
              }
              onRemove={() => onChange(gateways.filter((g) => g.id !== gw.id))}
            />
          ))}
        </SortableContext>
      </DndContext>
      {/* Enter and the Add button are shortcuts, not requirements: whatever has been typed is
          committed as soon as focus leaves the row, so a gateway typed but never explicitly added
          is not silently dropped on save. Tabbing between fields inside the row does not commit,
          and a following Add click is a no-op because add() clears the inputs. */}
      <div className="flex gap-2"
        onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) add() }}
      >
        <GatewaySelect
          value={newGatewayName}
          onChange={(name, option) => {
            setNewGatewayName(name)
            setNewGatewayId(option?.gatewayId ?? '')
          }}
          onEnter={add}
          options={options}
          className="flex-1"
        />
        <input
          value={newGatewayId}
          onChange={(e) => setNewGatewayId(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="Gateway ID (optional)"
          className="flex-1 rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none focus:border-brand-500 dark:border-[#222226]"
        />
        <Button type="button" size="sm" variant="secondary" onClick={add}>
          <Plus size={13} /> Add
        </Button>
      </div>
    </div>
  )
}

// ---- Volume split editor ----
export function VolumeSplitEditor({
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

  const options = gatewayOptions(
    suggestions,
    entries.map((e) => e.gatewayName),
    entries.map((e) => e.gatewayId),
  )

  return (
    <div className="space-y-2">
      {entries.map((e) => (
        <VolumeSplitRow
          key={e.id}
          entry={e}
          // The row's own gateway stays selectable while editing it; only the others are taken.
          options={gatewayOptions(
            suggestions,
            entries.filter((other) => other.id !== e.id).map((other) => other.gatewayName),
            entries.filter((other) => other.id !== e.id).map((other) => other.gatewayId),
          )}
          onEdit={(patch) => onChange(entries.map((x) => (x.id === e.id ? { ...x, ...patch } : x)))}
          onRemove={() => onChange(entries.filter((x) => x.id !== e.id))}
        />
      ))}
      {entries.length > 0 && (
        <p className={`text-xs font-medium ${total === 100 ? 'text-emerald-700' : 'text-amber-700'}`}>
          Total: {total}%{total !== 100 ? ' (must equal 100%)' : ' ✓'}
        </p>
      )}
      {/* Same commit-on-leave behaviour as the priority editor above. */}
      <div
        className="flex gap-2"
        onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) add() }}
      >
        <input
          type="number"
          value={newSplit}
          onChange={(e) => setNewSplit(e.target.value)}
          placeholder="Split %"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm w-20 focus:outline-none focus:border-brand-500"
        />
        <GatewaySelect
          value={newName}
          onChange={(name, option) => {
            setNewName(name)
            setNewId(option?.gatewayId ?? '')
          }}
          onEnter={add}
          options={options}
          className="flex-1"
        />
        <input
          value={newId}
          onChange={(e) => setNewId(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), add())}
          placeholder="Gateway ID (optional)"
          className="flex-1 rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none focus:border-brand-500 dark:border-[#222226]"
        />
        <Button type="button" size="sm" variant="secondary" onClick={add}>
          <Plus size={13} /> Add
        </Button>
      </div>
    </div>
  )
}

/** One split row of {@link VolumeSplitEditor}, readable at a glance and editable in place. */
function VolumeSplitRow({
  entry,
  options,
  onEdit,
  onRemove,
}: {
  entry: VolumeSplitEntry
  options: GatewayOption[]
  onEdit: (patch: Partial<VolumeSplitEntry>) => void
  onRemove: () => void
}) {
  const [editing, setEditing] = useState(false)
  const [draftSplit, setDraftSplit] = useState(String(entry.split))
  const [draftName, setDraftName] = useState(entry.gatewayName)
  const [draftId, setDraftId] = useState(entry.gatewayId)

  function startEditing() {
    setDraftSplit(String(entry.split))
    setDraftName(entry.gatewayName)
    setDraftId(entry.gatewayId)
    setEditing(true)
  }

  function commit() {
    const name = draftName.trim()
    // An empty name would leave a row that routes nowhere, so stay in edit mode until it has one.
    if (!name) return
    onEdit({ split: Number(draftSplit) || 0, gatewayName: name, gatewayId: draftId.trim() })
    setEditing(false)
  }

  if (editing) {
    return (
      <div
        className="flex items-center gap-2 rounded-lg border border-brand-300 bg-white px-2 py-1.5 dark:border-brand-500/50 dark:bg-[#111118]"
        onKeyDown={(e) => {
          if (e.key === 'Escape') { e.preventDefault(); setEditing(false) }
          if (e.key === 'Enter') { e.preventDefault(); commit() }
        }}
      >
        <input
          type="number"
          value={draftSplit}
          onChange={(e) => setDraftSplit(e.target.value)}
          aria-label="Split percentage"
          className="w-20 rounded-lg border border-slate-200 bg-transparent px-2 py-1 text-sm focus:outline-none focus:border-brand-500 dark:border-[#222226]"
        />
        <GatewaySelect
          value={draftName}
          onChange={(name, option) => {
            setDraftName(name)
            setDraftId(option?.gatewayId ?? '')
          }}
          onEnter={commit}
          options={options}
          className="flex-1"
        />
        <input
          value={draftId}
          onChange={(e) => setDraftId(e.target.value)}
          placeholder="Gateway ID (optional)"
          className="flex-1 rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none focus:border-brand-500 dark:border-[#222226]"
        />
        <button
          type="button"
          onClick={commit}
          disabled={!draftName.trim()}
          aria-label="Save split"
          className="text-emerald-700 transition-colors hover:text-emerald-700 disabled:cursor-not-allowed disabled:opacity-40"
        >
          <Check size={14} />
        </button>
        <button
          type="button"
          onClick={() => setEditing(false)}
          aria-label="Cancel edit"
          className="text-slate-500 transition-colors hover:text-slate-600"
        >
          <X size={14} />
        </button>
      </div>
    )
  }

  return (
    <div className="flex items-center gap-2 bg-slate-100 dark:bg-[#111118] border border-slate-200 dark:border-[#1c1c24] rounded-lg px-2 py-1.5">
      <span className="text-xs font-bold text-brand-600 w-10 shrink-0 tabular-nums">{entry.split}%</span>
      <button
        type="button"
        onClick={startEditing}
        aria-label={`Edit ${entry.gatewayName}`}
        className="group flex min-w-0 flex-1 items-center gap-2 text-left hover:text-brand-600 dark:hover:text-brand-400"
      >
        <span className="truncate font-mono text-sm">
          {entry.gatewayName}{entry.gatewayId ? ` (${entry.gatewayId})` : ''}
        </span>
        <Pencil size={12} className="shrink-0 text-slate-500 transition-colors group-hover:text-brand-600 dark:group-hover:text-brand-400" />
      </button>
      <button type="button" onClick={onRemove} aria-label={`Remove ${entry.gatewayName}`} className="text-red-600 hover:text-red-600">
        <Trash2 size={12} />
      </button>
    </div>
  )
}

// ---- Volume split priority editor ----
export function VolumeSplitPriorityEditor({
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
        <p className={`text-xs font-medium ${total === 100 ? 'text-emerald-700' : 'text-amber-700'}`}>
          Total: {total}%{total !== 100 ? ' (must equal 100%)' : ' ✓'}
        </p>
      )}
      {entries.map((entry, idx) => (
        <div
          key={entry.id}
          className="rounded-lg border border-slate-200 dark:border-[#222226] overflow-hidden"
        >
          <div className="flex items-center gap-2 px-3 py-2 bg-slate-50 dark:bg-[#111118] border-b border-slate-200 dark:border-[#1c1c24]">
            <span className="text-xs text-slate-500 font-medium shrink-0">Split {idx + 1}:</span>
            <input
              type="number"
              value={entry.split}
              onChange={(e) => updateEntry(entry.id, { split: Number(e.target.value) })}
              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded px-2 py-0.5 text-xs w-16 focus:outline-none"
            />
            <span className="text-xs text-slate-500">%</span>
            <button
              type="button"
              onClick={() => onChange(entries.filter((e) => e.id !== entry.id))}
              className="ml-auto text-red-600 hover:text-red-600"
            >
              <Trash2 size={12} />
            </button>
          </div>
          <div className="p-3">
            <p className="mb-2 text-[11px] font-semibold uppercase tracking-widest text-slate-500 dark:text-[#8d96a8] leading-4">Priority list for this split</p>
            <PriorityEditor
              gateways={entry.gateways}
              suggestions={suggestions}
              onChange={(gws) => updateEntry(entry.id, { gateways: gws })}
            />
          </div>
        </div>
      ))}
      {/* Same commit-on-leave behaviour as the editors above. */}
      <div
        className="flex gap-2 items-center"
        onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) addSplit() }}
      >
        <input
          type="number"
          value={newSplit}
          onChange={(e) => setNewSplit(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), addSplit())}
          placeholder="Split %"
          className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm w-24 focus:outline-none focus:border-brand-500"
        />
        <Button type="button" size="sm" variant="secondary" onClick={addSplit}>
          <Plus size={13} /> Add split
        </Button>
      </div>
    </div>
  )
}

// ---- Condition row ----
export function ConditionRowEditor({
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
  const isMetadata = keyInfo?.type === 'udf' || keyInfo?.type === 'global_ref'
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
    // Fixed tracks so the columns line up down the whole rule. Widths are sized to the content a
    // field actually holds — a stretched box for a two-digit amount reads worse than a small one.
    <div className="flex items-center gap-3">
      <SearchableSelect
        className="w-[13.5rem] shrink-0"
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
        className="cond-select w-[10.5rem] shrink-0"
      >
        {operators.map((op) => (
          <option key={op} value={op}>{OPERATOR_LABELS[op] || op}</option>
        ))}
      </select>
      <div className="flex items-center gap-2">
        {isEnum && isMulti ? (
          <div data-cy="cond-val" className="w-[14rem]">
            <SearchableMultiSelect
              values={selectedValues}
              onChange={(vals) => onChange({ ...row, value: vals })}
              options={(keyInfo?.values || []).map((v: string) => ({ value: v, label: toLabel(v) }))}
              placeholder="Select values…"
            />
          </div>
        ) : isEnum ? (
          <SearchableSelect
            className="w-[12rem]"
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
            className="rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none dark:border-[#222226] w-32"
          />
        ) : isMetadata ? (
          <>
            <input
              type="text"
              value={row.metadataKey || ''}
              onChange={(e) => onChange({ ...row, metadataKey: e.target.value })}
              placeholder="key"
              className="rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none dark:border-[#222226] w-40"
            />
            <span className="shrink-0 text-sm text-slate-500 dark:text-slate-400 select-none">=</span>
            <input
              type="text"
              value={row.value as string}
              onChange={(e) => onChange({ ...row, value: e.target.value })}
              placeholder="value"
              className="rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none dark:border-[#222226] w-44"
            />
          </>
        ) : (
          <input
            type="text"
            value={row.value as string}
            onChange={(e) => onChange({ ...row, value: e.target.value })}
            placeholder="value"
            className="rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none dark:border-[#222226] w-52"
          />
        )}
      </div>
      <button type="button" onClick={onRemove} aria-label="Remove condition" className="shrink-0 text-red-600 transition-colors hover:text-red-600">
        <Trash2 size={15} />
      </button>
    </div>
  )
}

// ---- Condition group ----
export function ConditionGroupEditor({
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
          <div key={cond.id} data-testid="condition-row" className="flex items-center gap-3 px-4 py-3">
            {group.conditions.length > 1 && (
              <span className="w-10 shrink-0 text-[11px] font-bold uppercase tracking-widest text-sky-700 select-none leading-4">
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
        {/* Sits directly under the last condition, because that is where a new row lands — below
            the nested branches it would point at the wrong insertion point. */}
        <div className="flex items-center gap-4 px-4 py-2.5">
          <button
            type="button"
            onClick={addCondition}
            className="flex items-center gap-1 text-[13px] font-medium text-brand-600 transition-colors hover:text-brand-700 dark:text-brand-400 leading-[18px]"
          >
            <Plus size={13} /> Add condition
          </button>
          {/* A nested branch has no group-level footer, so its remove control rides along here. */}
          {depth > 0 && canRemove && (
            <button
              type="button"
              onClick={onRemove}
              className="ml-auto flex items-center gap-1 text-[13px] font-medium text-red-600 transition-colors hover:text-red-600 leading-[18px]"
            >
              <Trash2 size={13} /> Remove group
            </button>
          )}
        </div>
      </div>

      {/* Nested OR branches — shown only at depth 0 */}
      {depth === 0 && group.nested.length > 0 && (
        <div className="border-t border-slate-100 dark:border-[#1c1c24] px-3 pt-3 pb-2 space-y-2">
          <div className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-widest text-slate-500 dark:text-[#8d96a8] leading-4">
            <CornerDownRight size={11} />
            Then match any of (nested OR)
          </div>
          {group.nested.map((nestedGroup, nIdx) => (
            <div key={nestedGroup.id} className="pl-3 border-l-2 border-sky-200 dark:border-sky-800">
              {nIdx > 0 && (
                <p className="mb-1 text-[11px] font-bold text-sky-700 leading-4">OR</p>
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

      {/* Group-level actions only — "Add condition" lives with the conditions above. */}
      {depth === 0 && (
        <div className="flex flex-wrap items-center gap-4 border-t border-slate-100 px-4 py-2.5 dark:border-[#1c1c24]">
          {depth === 0 && (
            <button
              type="button"
              onClick={addNestedBranch}
              className="flex items-center gap-1 text-[13px] font-medium text-brand-600 transition-colors hover:text-brand-700 dark:text-brand-400 leading-[18px]"
            >
              <CornerDownRight size={13} /> Add nested branch
            </button>
          )}
          {canRemove && (
            <button
              type="button"
              onClick={onRemove}
              className="ml-auto flex items-center gap-1 text-[13px] font-medium text-red-600 transition-colors hover:text-red-600 leading-[18px]"
            >
              <Trash2 size={13} /> Remove group
            </button>
          )}
        </div>
      )}
    </div>
  )
}

const OUTPUT_TYPE_LABELS: Record<string, string> = {
  priority: 'Priority',
  volume_split: 'Volume Split',
}

// ---- Rule block ----
export function RuleBlockEditor({
  block,
  index,
  onChange,
  onRemove,
  routingKeys,
  gatewaySuggestions = [],
  error,
}: {
  block: RuleBlock
  index?: number
  onChange: (b: RuleBlock) => void
  onRemove: () => void
  routingKeys: Record<string, RoutingKeyConfig>
  gatewaySuggestions?: string[]
  /** Validation message for this block, shown against the section that produced it. */
  error?: string | null
}) {
  const [collapsed, setCollapsed] = useState(false)

  // A message inside a collapsed block would be invisible, so an errored block opens itself.
  useEffect(() => {
    if (error) setCollapsed(false)
  }, [error])

  function addGroup() {
    onChange({ ...block, statements: [...block.statements, createStatementGroup(routingKeys)] })
  }

  return (
    <div className={`overflow-hidden rounded-xl border ${
      error ? 'border-red-300 dark:border-red-500/50' : 'border-slate-200 dark:border-[#1c1c24]'
    }`}>
      {/* Header — clicking anywhere on it collapses the block, except the name field and the
          action buttons, which stop the click from reaching it. */}
      <div
        onClick={() => setCollapsed(!collapsed)}
        className="flex cursor-pointer items-center justify-between gap-3 border-b border-slate-200 bg-slate-50 px-5 py-3 dark:border-[#1c1c24] dark:bg-[#111118]"
      >
        <div className="flex min-w-0 flex-1 items-baseline gap-2">
          <input
            value={block.name}
            onChange={(e) => onChange({ ...block, name: e.target.value })}
            onClick={(e) => e.stopPropagation()}
            placeholder="Rule name"
            className="input-bare min-w-0 max-w-[16rem] flex-1 cursor-text text-[15px] font-semibold text-slate-800 focus:outline-none dark:text-slate-100 leading-[22px]"
          />
          {/* Rules are evaluated top-down, so the first match wins — say so where it matters. */}
          <span className="shrink-0 truncate text-xs text-slate-500 dark:text-[#78849a]">
            {index === 0 ? '(Highest priority matching check)' : index !== undefined ? `(Checked if rule ${index} does not match)` : ''}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-3">
          <button type="button" onClick={(e) => { e.stopPropagation(); onRemove() }} aria-label="Delete rule" className="text-red-600 transition-colors hover:text-red-600">
            <Trash2 size={15} />
          </button>
          <button
            type="button"
            onClick={(e) => { e.stopPropagation(); setCollapsed(!collapsed) }}
            aria-label={collapsed ? 'Expand rule' : 'Collapse rule'}
            className="text-slate-500 transition-colors hover:text-slate-600"
          >
            {collapsed ? <ChevronDown size={15} /> : <ChevronUp size={15} />}
          </button>
        </div>
      </div>

      {!collapsed && (
        <div className="divide-y divide-slate-100 dark:divide-[#1c1c24]">
          {/* IF section */}
          <div className="space-y-2.5 px-5 py-5">
            <p className="mb-3 text-xs font-semibold uppercase tracking-widest text-slate-500 dark:text-[#8d96a8]">If</p>
            {block.statements.map((group, idx) => (
              <div key={group.id} className="space-y-2">
                {idx > 0 && (
                  <div className="flex items-center gap-3">
                    <span className="h-px flex-1 bg-slate-200 dark:bg-[#222]" />
                    <span className="text-[11px] font-bold uppercase tracking-widest text-sky-700 px-1 leading-4">or</span>
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
              className="mt-1 flex items-center gap-1 text-[13px] font-medium text-brand-600 transition-colors hover:text-brand-700 dark:text-brand-400 leading-[18px]"
            >
              <Plus size={13} /> Add OR group
            </button>
          </div>

          {/* THEN section */}
          <div data-cy="then-section" className="px-5 py-5">
            <div className="mb-3 flex items-center gap-3">
              <p className="shrink-0 text-xs font-semibold uppercase tracking-widest text-slate-500 dark:text-[#8d96a8]">Then route</p>
              <div className="flex overflow-hidden rounded-lg border border-slate-200 text-xs dark:border-[#222226]">
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
            <FieldError message={error} />
          </div>
        </div>
      )}
    </div>
  )
}

