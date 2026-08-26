import { RoutingAlgorithm, EuclidAlgorithmData, EuclidCondition, EuclidStatement } from '../../../types/api'
import { toLabel, formatOp } from '../../../features/routing/euclid/state'
import { gatewayLabel, normalizeRuleOutput } from '../../../features/routing/euclid/summarize'

export function RuleBreakdown({ algo }: { algo: RoutingAlgorithm }) {
  const algorithm = algo.algorithm_data || algo.algorithm
  const data = algorithm?.data as EuclidAlgorithmData | undefined
  const defaultSel = data?.default_selection || data?.defaultSelection

  return (
    <div className="space-y-2.5">
      {(data?.rules ?? []).map((rule, i) => {
        const {
          priorityGateways, volumeSplits, volumeSplitPriorityEntries,
          isVolumeSplit, isVolumeSplitPriority,
        } = normalizeRuleOutput(rule)

        return (
          <div key={i} className="overflow-hidden rounded-xl border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0d1018]">
            <div className="flex items-center justify-between gap-2 border-b border-slate-100 dark:border-[#1e2330] bg-slate-50 dark:bg-[#10131c] px-3 py-1.5">
              <span className="text-[11px] font-semibold uppercase tracking-wide text-slate-400 dark:text-[#4e5870]">
                {rule.name || `Rule ${i + 1}`}
              </span>
              {rule.routing_type && (
                <span className={`text-[10px] font-semibold px-1.5 py-0.5 rounded-full ${
                  rule.routing_type === 'volume_split_priority'
                    ? 'bg-purple-100 dark:bg-purple-900/30 text-purple-600 dark:text-purple-400'
                    : rule.routing_type === 'volume_split'
                    ? 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-600 dark:text-emerald-400'
                    : 'bg-brand-100 dark:bg-brand-900/30 text-brand-600 dark:text-brand-400'
                }`}>
                  {rule.routing_type === 'volume_split_priority' ? 'Split + Priority'
                    : rule.routing_type === 'volume_split' ? 'Volume Split'
                    : 'Priority'}
                </span>
              )}
            </div>
            <div className="space-y-1.5 px-3 py-2.5">
              {(rule.statements ?? []).map((statement, gi) => (
                <div key={gi}>
                  {gi > 0 && (
                    <div className="flex items-center gap-2 my-1.5">
                      <span className="h-px flex-1 bg-slate-200 dark:bg-[#1e2330]" />
                      <span className="text-[10px] font-bold text-sky-500">OR</span>
                      <span className="h-px flex-1 bg-slate-200 dark:bg-[#1e2330]" />
                    </div>
                  )}
                  <StatementView statement={statement} />
                </div>
              ))}
              <div className="flex items-start gap-2 pt-0.5 text-xs">
                <span className="w-7 shrink-0 text-right text-[10px] font-bold text-brand-500 select-none">→</span>
                <div className="flex flex-wrap gap-1">
                  {isVolumeSplitPriority
                    ? volumeSplitPriorityEntries.map((e, j) => (
                        <span key={j} className="rounded-full bg-purple-50 dark:bg-purple-900/20 px-2 py-0.5 text-[11px] font-medium text-purple-700 dark:text-purple-300">
                          {e.split}%: {e.output.map(gatewayLabel).join(', ')}
                        </span>
                      ))
                    : isVolumeSplit
                    ? volumeSplits.map((s, j) => (
                        <span key={j} className="rounded-full bg-emerald-50 dark:bg-emerald-900/20 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                          {gatewayLabel(s.output)} {s.split}%
                        </span>
                      ))
                    : priorityGateways.map((g, j) => (
                        <span key={j} className="rounded-full bg-brand-50 dark:bg-brand-900/20 px-2 py-0.5 text-[11px] font-medium text-brand-700 dark:text-brand-300">
                          {j + 1}. {gatewayLabel(g)}
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
        const defGateways = (Array.isArray(defRaw?.priority) ? defRaw!.priority : Array.isArray(defRaw?.data) ? defRaw!.data : []) as { gateway_name: string; gateway_id?: string | null }[]
        // An empty default_selection is accepted by the API but leaves unmatched payments with a
        // blank gateway (interpreter.rs evaluates Priority([]) to a default-constructed connector),
        // so call it out rather than rendering nothing.
        return defGateways.length > 0 ? (
          <div className="flex items-center gap-2 px-1 text-xs">
            <span className="text-slate-400 dark:text-[#4e5870]">Default:</span>
            <div className="flex flex-wrap gap-1">
              {defGateways.map((g, i) => (
                <span key={i} className="rounded-full bg-slate-100 dark:bg-[#1a1f2a] px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:text-[#8090a8]">
                  {gatewayLabel(g)}
                </span>
              ))}
            </div>
          </div>
        ) : (
          <div className="flex items-center gap-2 px-1 text-xs">
            <span className="text-slate-400 dark:text-[#4e5870]">Default:</span>
            <span className="font-medium text-amber-600 dark:text-amber-400">
              Not set — unmatched payments have no gateway
            </span>
          </div>
        )
      })()}

      {(!data?.rules || data.rules.length === 0) && (
        <p className="text-xs text-slate-400 italic">No rules configured.</p>
      )}
    </div>
  )
}

/** One condition chip: `Payment Method = Card`. */
function ConditionChip({ cond, op }: { cond: EuclidCondition; op: string }) {
  return (
    <div className="flex items-center gap-2 px-2 py-1.5 text-xs">
      <span className="w-7 shrink-0 select-none text-right text-[10px] font-bold text-slate-300 dark:text-[#3a4258]">
        {op}
      </span>
      <span className="rounded-md bg-slate-100 px-2 py-1 text-slate-700 dark:bg-[#1a1f2a] dark:text-[#c8d0de]">
        {toLabel(String(cond.lhs ?? ''))}{' '}
        <span className="font-mono text-slate-400 dark:text-[#5d6880]">{formatOp(String(cond.comparison ?? ''))}</span>{' '}
        <span className="font-medium">
          {cond.value?.type === 'metadata_variant' && cond.value.value && typeof cond.value.value === 'object'
            ? `{ "${(cond.value.value as { key?: string }).key ?? ''}" : "${(cond.value.value as { value?: string }).value ?? ''}" }`
            : toLabel(String(cond.value?.value ?? ''))}
        </span>
      </span>
    </div>
  )
}

/**
 * A statement, drawn the way the interpreter reads it: the `condition` list is joined with AND,
 * and when `nested` is present at least one nested branch must also match. Flattening the branches
 * into the same list would turn `A and B and ((C and D) or E)` into `A and B and C and D or E`,
 * which is a different rule.
 */
function StatementView({ statement }: { statement: EuclidStatement }) {
  const conditions = statement.condition ?? []
  const nested = statement.nested ?? []

  return (
    <div className="overflow-hidden rounded-lg border border-slate-200 dark:border-[#1e2330]">
      <div className="divide-y divide-slate-100 dark:divide-[#1e2330]">
        {conditions.map((cond, ci) => (
          <ConditionChip key={ci} cond={cond} op={ci === 0 ? 'IF' : 'AND'} />
        ))}
      </div>

      {nested.length > 0 && (
        <div className="border-t border-slate-100 px-2 py-2 dark:border-[#1e2330]">
          <p className="mb-1.5 text-[10px] font-semibold uppercase tracking-widest text-slate-400 dark:text-[#4e5870]">
            {conditions.length > 0 ? 'and any of' : 'any of'}
          </p>
          <div className="space-y-1.5">
            {nested.map((branch, ni) => (
              <div key={ni}>
                {ni > 0 && <p className="mb-1 text-[10px] font-bold text-sky-500">OR</p>}
                <div className="border-l-2 border-sky-200 pl-2 dark:border-sky-800">
                  <StatementView statement={branch} />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
