import { useEffect, useRef, useState, useMemo } from 'react'
import CodeMirror, { EditorView } from '@uiw/react-codemirror'
import { autocompletion, CompletionContext, CompletionResult, startCompletion } from '@codemirror/autocomplete'
import { StreamLanguage } from '@codemirror/language'
import { linter, Diagnostic, forceLinting } from '@codemirror/lint'
import { placeholder as cmPlaceholder } from '@codemirror/view'
import { githubLightInit, githubDarkInit } from '@uiw/codemirror-theme-github'
import { tags as t } from '@lezer/highlight'
import { RoutingKeyConfig } from '../../hooks/useDynamicRoutingConfig'

// ---- Shared types ----
export interface GatewayEntry {
  id: string
  gatewayName: string
  gatewayId: string
}

export interface ConditionRow {
  id: string
  lhs: string
  operator: string
  value: string | string[]
}

export interface StatementGroup {
  id: string
  conditions: ConditionRow[]
  nested: StatementGroup[]
}

export interface VolumeSplitEntry {
  id: string
  split: number
  gatewayName: string
  gatewayId: string
}

export interface VolumeSplitPriorityEntry {
  id: string
  split: number
  gateways: GatewayEntry[]
}

export interface RuleBlock {
  id: string
  name: string
  statements: StatementGroup[]
  outputType: 'priority' | 'volume_split' | 'volume_split_priority'
  priorityGateways: GatewayEntry[]
  volumeSplitEntries: VolumeSplitEntry[]
  volumeSplitPriorityEntries: VolumeSplitPriorityEntry[]
}

// ---- Fuzzy match ----
function fuzzyScore(needle: string, haystack: string): number | null {
  const n = needle.toLowerCase()
  const h = haystack.toLowerCase()
  if (!n) return 0
  if (h.startsWith(n)) return 1000 - h.length
  let score = 0, ni = 0, lastHi = -1, run = 0
  for (let hi = 0; hi < h.length && ni < n.length; hi++) {
    if (h[hi] === n[ni]) {
      run = lastHi === hi - 1 ? run + 1 : 1
      score += run * 3
      if (hi === 0 || h[hi - 1] === '_' || h[hi - 1] === '-') score += 5
      lastHi = hi
      ni++
    }
  }
  if (ni < n.length) return null
  score -= h.length * 0.1
  return score
}

function fuzzyFilter(needle: string, items: string[]): string[] {
  if (!needle) return items
  return items
    .map(item => ({ item, score: fuzzyScore(needle, item) }))
    .filter((r): r is { item: string; score: number } => r.score !== null)
    .sort((a, b) => b.score - a.score)
    .map(r => r.item)
}

// ---- DSL serializer ----
const DSL_OP_TO_SYM: Record<string, string> = {
  '==': '=', '!=': '!=', '>': '>', '<': '<', '>=': '>=', '<=': '<=',
  'in': 'in', 'not_in': 'not in',
}
const DSL_SYM_TO_OP: Record<string, string> = {
  '=': '==', '!=': '!=', '>': '>', '<': '<', '>=': '>=', '<=': '<=',
  'in': 'in', 'not in': 'not_in',
}

function condToDSL(c: ConditionRow): string {
  if (Array.isArray(c.value)) return `${c.lhs} in [${c.value.join(', ')}]`
  return `${c.lhs} ${DSL_OP_TO_SYM[c.operator] ?? c.operator} ${c.value}`
}

function stmtToDSLLines(stmt: StatementGroup): string[] {
  const lines: string[] = []
  if (stmt.conditions.length === 0 && stmt.nested.length > 0) {
    const promoted = stmt.nested[0].conditions
    lines.push(promoted.length > 1
      ? `(${promoted.map(condToDSL).join(' and ')})`
      : promoted.length === 1 ? condToDSL(promoted[0]) : '')
    const remaining = stmt.nested.slice(1)
    if (remaining.length > 0) {
      lines.push(`and (${remaining.map(n => n.conditions.map(condToDSL).join(' and ')).join(' or ')})`)
    }
    return lines.filter(Boolean)
  }
  stmt.conditions.forEach((c, i) => lines.push(i === 0 ? condToDSL(c) : `and ${condToDSL(c)}`))
  if (stmt.nested.length > 0) {
    const inner = stmt.nested.map(n => n.conditions.map(condToDSL).join(' and ')).join(' or ')
    lines.push(`and (${inner})`)
  }
  return lines
}

export function serializeToDSL(rules: RuleBlock[]): string {
  return rules.map(rule => {
    const lines: string[] = []
    rule.statements.forEach((stmt, si) => {
      if (si > 0) lines.push('or')
      stmtToDSLLines(stmt).forEach(l => lines.push(l))
    })
    if (rule.outputType === 'priority') {
      const gws = rule.priorityGateways.map(g => g.gatewayId ? `${g.gatewayName}(${g.gatewayId})` : g.gatewayName).join(', ')
      lines.push(`=> priority: ${gws}`)
    } else if (rule.outputType === 'volume_split') {
      const gws = rule.volumeSplitEntries.map(e => `${e.split}% ${e.gatewayName}${e.gatewayId ? `(${e.gatewayId})` : ''}`).join(', ')
      lines.push(`=> volume_split: ${gws}`)
    } else {
      const gws = rule.volumeSplitPriorityEntries.map(e => `${e.split}% [${e.gateways.map(g => g.gatewayId ? `${g.gatewayName}(${g.gatewayId})` : g.gatewayName).join(', ')}]`).join(', ')
      lines.push(`=> volume_split_priority: ${gws}`)
    }
    return lines.join('\n')
  }).join('\n\n')
}

// ---- DSL parser ----
export interface DSLParseResult { rules: RuleBlock[] | null; error: string | null }

export class InvalidConditionError extends Error {}

function parseCondDSL(text: string): ConditionRow | null {
  text = text.trim()
  if (!text) return null
  const inM = /^(\w+)\s+in\s+\[([^\]]*)\]$/.exec(text)
  if (inM) {
    const values = inM[2].split(',').map(s => s.trim()).filter(Boolean)
    if (values.length === 0) throw new InvalidConditionError(`Empty value list in condition: "${text}"`)
    return { id: crypto.randomUUID(), lhs: inM[1], operator: 'in', value: values }
  }
  const notInM = /^(\w+)\s+not\s+in\s+\[([^\]]*)\]$/.exec(text)
  if (notInM) {
    const values = notInM[2].split(',').map(s => s.trim()).filter(Boolean)
    if (values.length === 0) throw new InvalidConditionError(`Empty value list in condition: "${text}"`)
    return { id: crypto.randomUUID(), lhs: notInM[1], operator: 'not_in', value: values }
  }
  const opM = /^(\w+)\s*(>=|<=|!=|>|<|=)\s*(.+)$/.exec(text)
  if (opM) {
    const value = opM[3].trim()
    if (!value) throw new InvalidConditionError(`Empty value in condition: "${text}"`)
    return { id: crypto.randomUUID(), lhs: opM[1], operator: DSL_SYM_TO_OP[opM[2]] ?? opM[2], value }
  }
  throw new InvalidConditionError(`Invalid condition syntax: "${text}"`)
}

interface ParseStmtResult {
  group: StatementGroup | null
  error: string | null
}

// Split `text` on `keyword` (e.g. " or ", " and ") without splitting inside [] or ()
function splitDepthAware(text: string, re: RegExp): string[] {
  const parts: string[] = []
  let depth = 0, start = 0
  for (let i = 0; i < text.length; i++) {
    if (text[i] === '(' || text[i] === '[') { depth++; continue }
    if (text[i] === ')' || text[i] === ']') { depth--; continue }
    if (depth > 0) continue
    const m = re.exec(text.slice(i))
    if (m && m.index === 0) {
      parts.push(text.slice(start, i).trim())
      i += m[0].length - 1
      start = i + 1
    }
  }
  parts.push(text.slice(start).trim())
  return parts.filter(Boolean)
}

// Strip a single layer of redundant wrapping parens e.g. `(foo = bar)` → `foo = bar`
function unwrapParens(s: string): string {
  s = s.trim()
  return s.startsWith('(') && s.endsWith(')') ? s.slice(1, -1).trim() : s
}

function parseStmtDSLWithError(text: string): ParseStmtResult {
  const conditions: ConditionRow[] = []
  const nested: StatementGroup[] = []
  for (const raw of text.split('\n')) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    const stripped = line.replace(/^(?:and|or)\s+/i, '').trim()
    if (stripped.startsWith('(') && stripped.endsWith(')')) {
      if (conditions.length === 0 && nested.length === 0) {
        for (const andPart of splitDepthAware(stripped.slice(1, -1), /^\s+and\s+/i)) {
          try {
            const c = parseCondDSL(unwrapParens(andPart))
            if (c) conditions.push(c)
          } catch (e) {
            if (e instanceof InvalidConditionError) return { group: null, error: e.message }
            throw e
          }
        }
      } else {
        for (const orPart of splitDepthAware(stripped.slice(1, -1), /^\s+or\s+/i)) {
          const conds: ConditionRow[] = []
          for (const andPart of splitDepthAware(unwrapParens(orPart), /^\s+and\s+/i)) {
            try {
              const c = parseCondDSL(unwrapParens(andPart))
              if (c) conds.push(c)
            } catch (e) {
              if (e instanceof InvalidConditionError) return { group: null, error: e.message }
              throw e
            }
          }
          if (conds.length) nested.push({ id: crypto.randomUUID(), conditions: conds, nested: [] })
        }
      }
    } else {
      try {
        const c = parseCondDSL(stripped)
        if (c) conditions.push(c)
      } catch (e) {
        if (e instanceof InvalidConditionError) return { group: null, error: e.message }
        throw e
      }
    }
  }
  return { group: { id: crypto.randomUUID(), conditions, nested }, error: null }
}

function validateCondBody(condBody: string): string | null {
  let isFirst = true
  for (const raw of condBody.split('\n')) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    if (/^or$/i.test(line)) { isFirst = true; continue }
    if (/^and$/i.test(line)) continue
    if (isFirst) { isFirst = false; continue }
    if (!/^(?:and|or)\s+/i.test(line))
      return `Missing 'and' or 'or' before: "${line.length > 40 ? line.slice(0, 40) + '…' : line}"`
  }
  return null
}

function splitOnKeywords(line: string): Array<{ sep: string; text: string }> {
  const result: Array<{ sep: string; text: string }> = []
  let depth = 0, segStart = 0, pendingSep = ''
  for (let i = 0; i < line.length; i++) {
    if (line[i] === '[') { depth++; continue }
    if (line[i] === ']') { depth--; continue }
    if (depth > 0) continue
    const m = /^(\s+)(and|or)(\s+)/i.exec(line.slice(i))
    if (m) {
      result.push({ sep: pendingSep, text: line.slice(segStart, i).trim() })
      pendingSep = m[2].toLowerCase()
      i += m[0].length - 1
      segStart = i + 1
    }
  }
  result.push({ sep: pendingSep, text: line.slice(segStart).trim() })
  return result.filter(r => r.text)
}

function expandInlineKeywords(condBody: string): string {
  return condBody.split('\n').flatMap(rawLine => {
    const trimmed = rawLine.trim()
    if (!trimmed || trimmed.startsWith('#') || /^=>/.test(trimmed) || /^(?:and|or)(?:\s|$)/i.test(trimmed))
      return [rawLine]
    const parts = splitOnKeywords(trimmed)
    if (parts.length <= 1) return [rawLine]
    return parts.map((p, i) => i === 0 ? p.text : `${p.sep} ${p.text}`)
  }).join('\n')
}

export function parseDSL(text: string): DSLParseResult {
  const rules: RuleBlock[] = []
  const blocks = text.split(/\n\s*\n/).map(b => b.trim()).filter(Boolean)

  for (let bi = 0; bi < blocks.length; bi++) {
    const block = blocks[bi]
    const arrowIdx = block.lastIndexOf('=>')
    if (arrowIdx === -1) continue

    const condBody = expandInlineKeywords(block.slice(0, arrowIdx).trim())
    const condError = validateCondBody(condBody)
    if (condError) return { rules: null, error: `Rule ${bi + 1}: ${condError}` }
    const outputStr = block.slice(arrowIdx + 2).trim()

    let outputType: RuleBlock['outputType'] = 'priority'
    let priorityGateways: GatewayEntry[] = []
    let volumeSplitEntries: VolumeSplitEntry[] = []
    const volumeSplitPriorityEntries: VolumeSplitPriorityEntry[] = []

    if (outputStr.startsWith('priority:')) {
      outputType = 'priority'
      const gwStr = outputStr.slice('priority:'.length).trim()
      priorityGateways = gwStr ? gwStr.split(',').flatMap(s => {
        const gm = /^([^\s(]+)(?:\(([^)]*)\))?$/.exec(s.trim())
        return gm && gm[1] ? [{ id: crypto.randomUUID(), gatewayName: gm[1], gatewayId: gm[2] ?? '' }] : []
      }) : []
    } else if (outputStr.startsWith('volume_split:')) {
      outputType = 'volume_split'
      const vsStr = outputStr.slice('volume_split:'.length).trim()
      volumeSplitEntries = vsStr ? vsStr.split(',').flatMap(s => {
        const vm = /^(\d+)%\s+([^\s(]+)(?:\(([^)]*)\))?$/.exec(s.trim())
        return vm ? [{ id: crypto.randomUUID(), split: Number(vm[1]), gatewayName: vm[2], gatewayId: vm[3] ?? '' }] : []
      }) : []
    } else if (outputStr.startsWith('volume_split_priority:')) {
      outputType = 'volume_split_priority'
      const vspStr = outputStr.slice('volume_split_priority:'.length).trim()
      const entryRe = /(\d+)%\s*\[([^\]]*)\]/g
      let em: RegExpExecArray | null
      while ((em = entryRe.exec(vspStr)) !== null) {
        const gateways = em[2].split(',').map(s => s.trim()).filter(Boolean)
          .map(s => {
            const gm = /^([^\s(]+)(?:\(([^)]*)\))?$/.exec(s)
            return { id: crypto.randomUUID(), gatewayName: gm?.[1] ?? s, gatewayId: gm?.[2] ?? '' }
          })
        volumeSplitPriorityEntries.push({ id: crypto.randomUUID(), split: Number(em[1]), gateways })
      }
    } else {
      return { rules: null, error: `Rule ${bi + 1}: output must start with priority:, volume_split:, or volume_split_priority:` }
    }

    if (outputType === 'priority' && priorityGateways.length === 0)
      return { rules: null, error: `Rule ${bi + 1}: priority output must have at least one gateway` }
    if (outputType === 'volume_split' && volumeSplitEntries.length === 0)
      return { rules: null, error: `Rule ${bi + 1}: volume_split output must have at least one gateway` }
    if (outputType === 'volume_split_priority' && volumeSplitPriorityEntries.length === 0)
      return { rules: null, error: `Rule ${bi + 1}: volume_split_priority output must have at least one gateway` }

    const normalizedCondBody = condBody.replace(/^([ \t]*)or\s+(?=\S)/gim, '$1or\n$1')
    const statements: StatementGroup[] = []
    for (const part of normalizedCondBody.split(/^[ \t]*or[ \t]*$/im)) {
      const { group, error: stmtError } = parseStmtDSLWithError(part.trim())
      if (stmtError) return { rules: null, error: `Rule ${bi + 1}: ${stmtError}` }
      if (group && (group.conditions.length > 0 || group.nested.length > 0)) statements.push(group)
    }
    if (!statements.length) statements.push({ id: crypto.randomUUID(), conditions: [], nested: [] })

    rules.push({
      id: crypto.randomUUID(),
      name: `Rule ${rules.length + 1}`,
      statements, outputType, priorityGateways, volumeSplitEntries, volumeSplitPriorityEntries,
    })
  }

  return { rules, error: null }
}

// ---- Placeholder ----
export const CODE_EDITOR_PLACEHOLDER = `currency in [EUR, GBP]
and amount > 100
and (payment_method = card or card_type = credit)
=> priority: adyen

currency = USD
=> volume_split: 70% stripe, 30% adyen`

// ---- DSL language (syntax highlighting tokens) ----
const dslLanguage = StreamLanguage.define<Record<string, never>>({
  startState: () => ({}),
  token(stream) {
    if (stream.eatSpace()) return null
    if (stream.match(/^#.*/)) return 'comment'
    if (stream.match('=>')) return 'meta'
    if (stream.match(/^(?:and|or|not)\b/i)) return 'keyword'
    if (stream.match(/^in\b/i)) return 'keyword'
    if (stream.match(/^(?:volume_split_priority|volume_split|priority)(?=\s*:)/)) return 'typeName'
    if (stream.match(/^(?:!=|>=|<=|>|<|=)/)) return 'operator'
    if (stream.match(/^\d+/)) return 'number'
    if (stream.match(/^[[\]%,:]/)) return 'punctuation'
    if (stream.match(/^\w+/)) return 'variableName'
    stream.next()
    return null
  },
})

// ---- Themes (base editor colours + DSL syntax — single source of truth per mode) ----
const dslStyles = (dark: boolean) => [
  { tag: t.keyword,      color: dark ? '#a78bfa' : '#7c3aed', fontWeight: '600' },
  { tag: t.meta,         color: dark ? '#fbbf24' : '#d97706', fontWeight: '700' },
  { tag: t.typeName,     color: dark ? '#34d399' : '#059669', fontWeight: '600' },
  { tag: t.operator,     color: dark ? '#60a5fa' : '#2563eb' },
  { tag: t.number,       color: dark ? '#fbbf24' : '#b45309' },
  { tag: t.punctuation,  color: '#64748b' },
  { tag: t.comment,      color: dark ? '#4e5870' : '#94a3b8', fontStyle: 'italic' },
  { tag: t.variableName, color: dark ? '#cbd5e1' : '#1e293b' },
]

const lightTheme = githubLightInit({ styles: dslStyles(false) })
const darkTheme  = githubDarkInit ({ styles: dslStyles(true)  })

// ---- Suggestion logic (same rules as before, now standalone) ----
const COMPLETE_COND = /\w+\s+(?:(?:in|not\s+in)\s*\[[^\]]+\]|(?:=|!=|>=|<=|>|<)\s+\S+)\s*$/

function getSuggestions(
  text: string,
  cursor: number,
  routingKeys: Record<string, RoutingKeyConfig>,
  gatewaySuggestions: string[],
): string[] {
  const lines = text.slice(0, cursor).split('\n')
  const line = lines[lines.length - 1]
  const prevLine = lines.length >= 2 ? lines[lines.length - 2].trim() : ''

  if (!line.trim()) {
    if (COMPLETE_COND.test(prevLine) || /^or$/i.test(prevLine))
      return ['and', 'and (', 'or', 'or (', '=>']
    return []
  }

  const afterBracket = line.includes(']') ? line.slice(line.lastIndexOf(']') + 1) : line

  // Cursor is at/after a closing `]` with nothing meaningful after it → offer conjunctions
  if (line.includes(']') && !afterBracket.trim())
    return ['and', 'and (', 'or', 'or (', '=>']

  const trimmed = afterBracket.trimStart().replace(/^(?:and|or)\s+/i, '').replace(/^\(/, '')

  // `and ` / `or ` / `(` consumed the whole segment → offer field names
  if (!trimmed)
    return Object.keys(routingKeys).slice(0, 12)

  if (/\)\s*$/.test(trimmed))
    return ['and', 'or', 'or (', '=>']

  const outPfxM = /=>\s*(\w*)$/.exec(line)
  if (outPfxM) {
    const gw1 = gatewaySuggestions[0] ?? 'gateway_name'
    const gw2 = gatewaySuggestions[1] ?? 'gateway_name'
    const opts = [
      `priority: ${gw1}`,
      `volume_split: 70% ${gw1}, 30% ${gw2}`,
      `volume_split_priority: 70% [${gw1}], 30% [${gw2}]`,
    ]
    if (!outPfxM[1]) return opts
    return opts.sort((a, b) =>
      ((fuzzyScore(outPfxM[1], b) ?? -Infinity) - (fuzzyScore(outPfxM[1], a) ?? -Infinity))
    )
  }

  if (/=>\s*(?:priority|volume_split(?:_priority)?):/.test(line)) {
    const gwPfxM = /(\w*)$/.exec(line)
    return fuzzyFilter(gwPfxM?.[1] ?? '', gatewaySuggestions).slice(0, 10)
  }

  const inValM = /(\w+)\s+(?:in|not\s+in)\s*\[(?:[^\]]*,\s*)?(\w*)$/.exec(trimmed)
  if (inValM) {
    const allVals: string[] = routingKeys[inValM[1]]?.values ?? []
    const bracketM = /\[([^\]]*)$/.exec(trimmed)
    const selected = bracketM ? bracketM[1].split(',').map(s => s.trim()).filter(Boolean) : []
    return fuzzyFilter(inValM[2], allVals.filter(v => !selected.includes(v))).slice(0, 12)
  }

  const cmpValM = /(\w+)\s+(?:>=|<=|!=|>|<|=)\s*(\w*)$/.exec(trimmed)
  if (cmpValM)
    return fuzzyFilter(cmpValM[2], routingKeys[cmpValM[1]]?.values ?? []).slice(0, 12)

  const opM = /^(\w+)\s+(\w*)$/.exec(trimmed)
  if (opM && routingKeys[opM[1]]) {
    const isInt = routingKeys[opM[1]]?.type === 'integer'
    const ops = isInt ? ['=', '!=', '>', '<', '>=', '<='] : ['=', '!=', 'in [', 'not in [']
    return fuzzyFilter(opM[2], ops)
  }

  if (/\)\s*$/.test(trimmed))
    return ['and', 'or', 'or (', '=>']

  const fieldM = /(\w+)$/.exec(trimmed)
  if (fieldM && fieldM[1].length >= 1)
    return fuzzyFilter(fieldM[1], ['and', 'and (', 'or', 'or (', '=>', ...Object.keys(routingKeys)]).slice(0, 12)

  return []
}

// ---- Completion source ----
function makeCompletionSource(
  routingKeysRef: React.RefObject<Record<string, RoutingKeyConfig>>,
  gatewaySuggestionsRef: React.RefObject<string[]>,
) {
  return (context: CompletionContext): CompletionResult | null => {
    const text = context.state.doc.toString()
    const cursor = context.pos
    const suggestions = getSuggestions(text, cursor, routingKeysRef.current ?? {}, gatewaySuggestionsRef.current ?? [])
    if (!suggestions.length) return null

    const before = text.slice(0, cursor)
    const tokenM = /(\w+)$/.exec(before)
    const from = tokenM ? cursor - tokenM[0].length : cursor

    return {
      from,
      options: suggestions.map(label => ({
        label,
        apply(view, _c, from, to) {
          const docText = view.state.doc.toString()
          const after = docText.slice(to)
          const afterTokenM = /^\w+/.exec(after)
          let endPos = afterTokenM ? to + afterTokenM[0].length : to
          if (label.includes(':')) endPos = view.state.doc.lineAt(to).to
          const afterStripped = docText.slice(endPos)

          let insert: string
          let cursorPos: number
          if (label.endsWith('[')) {
            insert = label + ']'
            cursorPos = from + label.length  // between [ and ]
          } else if (label.endsWith('(')) {
            insert = label + ')'
            cursorPos = from + label.length  // between ( and )
          } else {
            const needsSpace = !label.endsWith(':') && !/^\s/.test(afterStripped)
            insert = label + (needsSpace ? ' ' : '')
            cursorPos = from + insert.length
          }

          view.dispatch({
            changes: { from, to: endPos, insert },
            selection: { anchor: cursorPos },
          })
          setTimeout(() => startCompletion(view), 0)
        },
      })),
      validFor: /^\w*$/,
    }
  }
}

// ---- Linter ----
function makeLinter(parseErrorRef: React.RefObject<string | null>) {
  return linter((view): Diagnostic[] => {
    const err = parseErrorRef.current
    if (!err) return []
    const text = view.state.doc.toString()
    const ruleMatch = /Rule (\d+)/.exec(err)
    const ruleIdx = ruleMatch ? parseInt(ruleMatch[1], 10) - 1 : 0
    const blocks = text.split(/\n\s*\n/)
    let from = 0
    for (let i = 0; i < ruleIdx && i < blocks.length; i++) from += blocks[i].length + 2
    const to = Math.min(from + (blocks[ruleIdx]?.length ?? 1), text.length)
    return [{ from: Math.min(from, text.length), to: Math.max(Math.min(to, text.length), from + 1), severity: 'error', message: err }]
  })
}

// ---- Base editor theme (font, padding, autocomplete tooltip) ----
const editorBaseTheme = EditorView.theme({
  '&': {
    fontSize: '12px',
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
  },
  '.cm-scroller': { padding: '12px 16px', lineHeight: '1.65', minHeight: '280px' },
  '.cm-content': { padding: '0' },
  '.cm-gutters': { display: 'none' },
  '.cm-activeLine': { backgroundColor: 'transparent' },
  '.cm-tooltip': { borderRadius: '8px !important', border: 'none !important', boxShadow: '0 8px 32px rgba(0,0,0,0.18)' },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { maxHeight: '220px', borderRadius: '8px', overflowY: 'auto' },
  '.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
    fontSize: '11px',
    padding: '4px 12px',
  },
  '.cm-diagnostic': { padding: '2px 4px', borderRadius: '3px' },
})

// ---- Component ----
interface RuleCodeEditorProps {
  value: string
  onChange: (v: string) => void
  parseError: string | null
  routingKeys: Record<string, RoutingKeyConfig>
  gatewaySuggestions: string[]
}

export function RuleCodeEditor({ value, onChange, parseError, routingKeys, gatewaySuggestions }: RuleCodeEditorProps) {
  const [isDark, setIsDark] = useState(() => document.documentElement.classList.contains('dark'))
  const routingKeysRef = useRef(routingKeys)
  const gatewaySuggestionsRef = useRef(gatewaySuggestions)
  const parseErrorRef = useRef(parseError)
  const editorViewRef = useRef<EditorView | null>(null)

  useEffect(() => { routingKeysRef.current = routingKeys }, [routingKeys])
  useEffect(() => { gatewaySuggestionsRef.current = gatewaySuggestions }, [gatewaySuggestions])
  useEffect(() => {
    parseErrorRef.current = parseError
    if (editorViewRef.current) forceLinting(editorViewRef.current)
  }, [parseError])

  useEffect(() => {
    const observer = new MutationObserver(() =>
      setIsDark(document.documentElement.classList.contains('dark'))
    )
    observer.observe(document.documentElement, { attributeFilter: ['class'] })
    return () => observer.disconnect()
  }, [])

  // Stable extensions (use refs so they never need to be recreated)
  const stableExtensions = useMemo(() => [
    dslLanguage,
    cmPlaceholder(CODE_EDITOR_PLACEHOLDER),
    autocompletion({
      override: [makeCompletionSource(routingKeysRef, gatewaySuggestionsRef)],
      activateOnTyping: true,
      closeOnBlur: true,
    }),
    makeLinter(parseErrorRef),
    EditorView.lineWrapping,
    editorBaseTheme,
    EditorView.domEventHandlers({
      click: (_, view) => { startCompletion(view); return false },
    }),
    EditorView.updateListener.of((update) => {
      if (!update.docChanged) return
      for (const tr of update.transactions) {
        tr.changes.iterChanges((_fA, _tA, _fB, _tB, inserted) => {
          const ch = inserted.toString()
          if (ch === ' ' || ch === '\n') startCompletion(update.view)
        })
      }
    }),
  ], [])

  const extensions = stableExtensions

  return (
    <div>
      <CodeMirror
        value={value}
        onChange={(val) => onChange(val)}
        onCreateEditor={(view) => { editorViewRef.current = view }}
        theme={isDark ? darkTheme : lightTheme}
        extensions={extensions}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          autocompletion: false,
          highlightActiveLine: false,
          highlightSelectionMatches: false,
          closeBrackets: true,
          bracketMatching: true,
          indentOnInput: false,
          searchKeymap: false,
        }}
        className={`rounded-xl border overflow-hidden ${
          parseError
            ? 'border-red-400 dark:border-red-500'
            : 'border-slate-200 dark:border-[#222226]'
        }`}
      />
      {parseError
        ? <p className="mt-1.5 text-xs text-red-500 font-medium">{parseError}</p>
        : <p className="mt-1.5 text-[11px] text-slate-400">Tab / Enter to complete · ↑↓ navigate · <code className="font-mono">or</code> on its own line starts an OR group</p>
      }
    </div>
  )
}
