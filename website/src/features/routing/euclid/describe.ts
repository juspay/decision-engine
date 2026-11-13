import { EuclidCondition, EuclidStatement, EuclidRule } from '../../../types/api'
import { toLabel, OPERATOR_LABELS } from './state'

/**
 * The one place a rule is turned into words.
 *
 * Every surface that shows a rule — the list table, the expandable breakdown, the builder's
 * summary rail — used to format it independently, and they disagreed about what nesting meant.
 * The authority is the interpreter (`src/euclid/interpreter.rs`, `eval_if_statement`):
 *
 *   a statement matches when EVERY entry in `condition` matches (AND)
 *   AND, when `nested` is present, at least ONE nested statement matches (OR)
 *
 * so `A, B` with nested `[C+D, E]` means `A and B and ((C and D) or E)` — not the flat
 * `A and B and C and D or E` that a naive walk produces, which is a different rule.
 */

const API_OP_TO_WORDS: Record<string, string> = {
  equal: 'equals',
  not_equal: 'does not equal',
  greater_than: 'greater than',
  less_than: 'less than',
  greater_than_equal: 'at least',
  less_than_equal: 'at most',
}

function operatorWords(comparison: string, isArray: boolean): string {
  if (isArray) return comparison === 'not_equal' ? 'is not one of' : 'is one of'
  return API_OP_TO_WORDS[comparison] ?? OPERATOR_LABELS[comparison] ?? comparison.replace(/_/g, ' ')
}

/** Enum values arrive as raw slugs (`three_ds`); show them the way the editor does. */
function valueWords(cond: EuclidCondition): string {
  const wrapper = cond.value as { type?: string; value?: unknown } | undefined
  const raw = wrapper?.value
  const type = wrapper?.type

  if (type === 'metadata_variant' && raw && typeof raw === 'object') {
    const meta = raw as { value?: unknown }
    return String(meta.value ?? '')
  }
  if (Array.isArray(raw)) {
    return raw.map((v) => toLabel(String(v))).join(', ')
  }
  if (type === 'enum_variant' || type === 'enum_variant_array') {
    return toLabel(String(raw ?? ''))
  }
  return String(raw ?? '')
}

function fieldWords(cond: EuclidCondition): string {
  const wrapper = cond.value as { type?: string; value?: unknown } | undefined
  if (wrapper?.type === 'metadata_variant' && wrapper.value && typeof wrapper.value === 'object') {
    const key = (wrapper.value as { key?: string }).key
    if (key && key !== cond.lhs) return `${toLabel(cond.lhs)}.${key}`
  }
  return toLabel(cond.lhs)
}

/** A single clause, e.g. `Authentication Type equals Three Ds`. */
export function describeCondition(cond: EuclidCondition): string {
  const wrapper = cond.value as { value?: unknown } | undefined
  const isArray = Array.isArray(wrapper?.value)
  const value = valueWords(cond)
  return `${fieldWords(cond)} ${operatorWords(cond.comparison, isArray)} ${value || '…'}`.trim()
}

/** True when a statement needs its own brackets to stay unambiguous inside a larger expression. */
function isCompound(stmt: EuclidStatement): boolean {
  const conditions = stmt.condition?.length ?? 0
  const nested = stmt.nested?.length ?? 0
  return conditions + (nested > 0 ? 1 : 0) > 1
}

/**
 * A statement as a fully bracketed boolean expression. Brackets are explicit rather than implied
 * by operator precedence, so the text cannot be misread.
 */
export function describeStatement(stmt: EuclidStatement): string {
  const clauses = (stmt.condition ?? []).map((c) => `(${describeCondition(c)})`)
  let text = clauses.join(' and ')

  const nested = stmt.nested ?? []
  if (nested.length > 0) {
    const branches = nested
      .map((n) => (isCompound(n) ? `(${describeStatement(n)})` : describeStatement(n)))
      .filter(Boolean)
      .join(' or ')
    if (branches) text = text ? `${text} and (${branches})` : `(${branches})`
  }

  return text
}

/** A rule's whole condition set. Statements are alternatives — any one matching is enough. */
export function describeRuleConditions(rule: EuclidRule): string {
  const statements = (rule.statements ?? []).map(describeStatement).filter(Boolean)
  if (statements.length === 0) return 'Always matches'
  if (statements.length === 1) return statements[0]
  return statements.map((s) => (s.includes(' and ') ? `(${s})` : s)).join(' or ')
}
