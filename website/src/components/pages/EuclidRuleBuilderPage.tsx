import { useEffect, useRef, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router-dom'
import useSWR from 'swr'
import { Plus, ArrowLeft } from 'lucide-react'
import { Card, CardBody } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { useMerchantStore } from '../../store/merchantStore'
import { useCanEditRouting } from '../../store/authStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { useDynamicRoutingConfig } from '../../hooks/useDynamicRoutingConfig'
import {
  RuleCodeEditor, serializeToDSL, parseDSL, CODE_EDITOR_PLACEHOLDER,
  type RuleBlock,
} from '../ui/RuleCodeEditor'
import { PriorityEditor, RuleBlockEditor } from '../routing/euclid/editors'
import { RuleBreakdown } from '../routing/euclid/RuleBreakdown'
import {
  buildAlgorithmData, createStatementGroup, parseAlgorithmToRuleBlocks,
  type DefaultOutput,
} from '../../features/routing/euclid/state'

export function EuclidRuleBuilderPage() {
  const canEditRouting = useCanEditRouting()
  const navigate = useNavigate()
  const { id: editId } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const cloneFromId = searchParams.get('cloneFrom')
  const isEdit = Boolean(editId)

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
  const [submitting, setSubmitting] = useState(false)
  const [submitError, setSubmitError] = useState<string | null>(null)

  const { data: allAlgorithms, mutate: mutateAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )
  const { data: activeAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const sourceId = editId || cloneFromId
  const sourceRule = sourceId ? (allAlgorithms || []).find((a) => a.id === sourceId) : undefined
  // The backend refuses to update an active algorithm (ensure_routing_algorithm_inactive), so say
  // that up front rather than letting Save fail.
  const isEditingActiveRule = isEdit && (activeAlgorithms || []).some((a) => a.id === editId)

  // Seed the form from the source rule exactly once. Without the guard, SWR revalidation would
  // overwrite whatever the user has typed since the page loaded.
  const seededFrom = useRef<string | null>(null)
  useEffect(() => {
    if (!sourceRule || seededFrom.current === sourceRule.id) return
    seededFrom.current = sourceRule.id
    const { ruleBlocks: parsedBlocks, defaultOutput: parsedDefault } = parseAlgorithmToRuleBlocks(sourceRule)
    setRuleName(isEdit ? sourceRule.name : `copy-of-${sourceRule.name}`)
    setRuleDesc(sourceRule.description && sourceRule.description !== 'N/A' ? sourceRule.description : '')
    setRuleBlocks(parsedBlocks)
    setDefaultOutput(parsedDefault)
    setEditorMode('visual')
  }, [sourceRule, isEdit])

  const gatewaySuggestions = Array.from(new Set([
    ...ruleBlocks.flatMap((b) => [
      ...b.priorityGateways.map((g) => g.gatewayName),
      ...b.volumeSplitEntries.map((e) => e.gatewayName),
      ...b.volumeSplitPriorityEntries.flatMap((e) => e.gateways.map((g) => g.gatewayName)),
    ]),
    ...defaultOutput.priorityGateways.map((g) => g.gatewayName),
  ].filter(Boolean)))

  const algorithmData = buildAlgorithmData(ruleBlocks, defaultOutput, routingKeys)

  function handleClear() {
    setRuleName('')
    setRuleDesc('')
    setRuleBlocks([])
    setDefaultOutput({ priorityGateways: [] })
    setEditorMode('visual')
    setCodeText('')
    setCodeParseError(null)
    setSubmitError(null)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!merchantId) { setSubmitError('Set a Merchant ID first.'); return }
    if (routingKeysUnavailable) {
      setSubmitError('Routing key config is unavailable. Ensure backend /config/routing-keys is reachable and valid.')
      return
    }
    if (!ruleName.trim()) { setSubmitError('Rule name is required.'); return }
    if (ruleBlocks.some(b =>
      (b.outputType === 'priority' && b.priorityGateways.length === 0) ||
      (b.outputType === 'volume_split' && b.volumeSplitEntries.length === 0) ||
      (b.outputType === 'volume_split_priority' && b.volumeSplitPriorityEntries.length === 0)
    )) { setSubmitError('Every rule needs at least one destination gateway in its THEN section.'); return }
    if (codeParseError) { setSubmitError(`Fix syntax error: ${codeParseError}`); return }

    setSubmitting(true)
    setSubmitError(null)
    try {
      const algorithm = { type: 'advanced', data: algorithmData }
      if (isEdit) {
        await apiPost('/routing/update', {
          created_by: merchantId,
          routing_algorithm_id: editId,
          name: ruleName.trim(),
          description: ruleDesc,
          algorithm,
        })
      } else {
        await apiPost<RoutingAlgorithm>('/routing/create', {
          name: ruleName.trim(),
          description: ruleDesc,
          created_by: merchantId,
          algorithm_for: 'payment',
          algorithm,
        })
      }
      await mutateAlgorithms()
      navigate('/routing/rules')
    } catch (err) {
      setSubmitError(String(err))
    } finally {
      setSubmitting(false)
    }
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
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <button
            type="button"
            onClick={() => navigate('/routing/rules')}
            className="mb-2 inline-flex items-center gap-2 text-sm font-medium text-slate-500 transition-colors hover:text-brand-600 dark:text-[#8d96a8] dark:hover:text-brand-400"
          >
            <ArrowLeft size={16} /> Rule-Based Routing
          </button>
          <h1 className="truncate text-2xl font-semibold text-slate-900 dark:text-white">
            {isEdit ? 'Edit Payment Rule' : 'Create Payment Rule'}
          </h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-[#8d96a8]">
            Create precise cascading routing paths for card transactions using conditional flags.
          </p>
        </div>
        <div className="flex shrink-0 rounded-lg border border-slate-200 text-xs dark:border-[#222226] overflow-hidden">
          <button
            type="button"
            onClick={switchToVisual}
            className={`px-3 py-1.5 transition-colors ${editorMode === 'visual' ? 'bg-brand-500 text-white font-semibold' : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-[#1c1c24]'}`}
          >
            Visual Builder
          </button>
          <button
            type="button"
            onClick={switchToCode}
            className={`px-3 py-1.5 transition-colors ${editorMode === 'code' ? 'bg-brand-500 text-white font-semibold' : 'text-slate-500 hover:bg-slate-100 dark:hover:bg-[#1c1c24]'}`}
          >
            Code
          </button>
        </div>
      </div>

      {isEdit && !sourceRule && allAlgorithms && (
        <ErrorMessage error="That rule no longer exists for this merchant." />
      )}
      {isEditingActiveRule && (
        <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
          <strong>This rule is active</strong> — active rules cannot be edited. Deactivate it from the
          rules list first, then come back.
        </div>
      )}

      <div className="flex flex-col gap-6 xl:flex-row xl:items-start">
      <form onSubmit={handleSubmit} className="min-w-0 flex-1 space-y-4">
        <Card>
          <CardBody className="space-y-5">
            {/* Inline labels share a fixed column so the two fields line up with each other. */}
            <div className="grid grid-cols-1 gap-x-12 gap-y-4 md:grid-cols-2">
              <div className="flex items-center gap-2">
                <label
                  htmlFor="rule-name"
                  className="shrink-0 whitespace-nowrap text-[13px] font-semibold text-slate-700 dark:text-slate-300"
                >
                  Rule Name *
                </label>
                <input
                  id="rule-name"
                  value={ruleName}
                  onChange={(e) => setRuleName(e.target.value)}
                  placeholder="my-rule"
                  className="min-w-0 flex-1 rounded-lg border border-slate-200 bg-transparent px-3.5 py-2.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                />
              </div>
              <div className="flex items-center gap-2">
                <label
                  htmlFor="rule-description"
                  className="shrink-0 whitespace-nowrap text-[13px] font-semibold text-slate-700 dark:text-slate-300"
                >
                  Description
                </label>
                <input
                  id="rule-description"
                  value={ruleDesc}
                  onChange={(e) => setRuleDesc(e.target.value)}
                  placeholder="Optional description"
                  className="min-w-0 flex-1 rounded-lg border border-slate-200 bg-transparent px-3.5 py-2.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                />
              </div>
            </div>

            <div className="space-y-3">
              <p className="border-b border-slate-100 pb-2.5 text-xs font-semibold uppercase tracking-widest text-slate-500 dark:border-[#1c1c24] dark:text-[#8d96a8]">
                Configured Rules
              </p>
              {routingKeysLoading && (
                <p className="text-sm text-slate-500">Loading routing keys from backend...</p>
              )}
              {routingKeysUnavailable && (
                <ErrorMessage error="Routing keys are unavailable from backend (/config/routing-keys). Rule Builder is disabled until this is fixed." />
              )}
              {editorMode === 'visual' ? (
                <>
                  {ruleBlocks.map((block, i) => (
                    <RuleBlockEditor
                      key={block.id}
                      block={block}
                      index={i}
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

            <div className="rounded-xl border border-dashed border-slate-300 px-4 py-4 dark:border-[#2a303a]">
              <p className="text-[15px] font-semibold text-slate-800 dark:text-slate-200">Default Fallback Gateway</p>
              <p className="mb-4 mt-1 text-[13px] text-slate-400 dark:text-[#8d96a8]">
                This gateway handles any transactions that do not match the custom rules configured
                above. Per-request overrides are possible via <code className="font-mono">fallback_output</code>.
              </p>
              <PriorityEditor
                gateways={defaultOutput.priorityGateways}
                suggestions={gatewaySuggestions}
                onChange={(gws) => setDefaultOutput({ ...defaultOutput, priorityGateways: gws })}
              />
            </div>

            <ErrorMessage error={submitError} />
          </CardBody>
        </Card>

        <div className="flex flex-wrap items-center gap-4">
          <Button
            type="submit"
            disabled={submitting || routingKeysUnavailable || isEditingActiveRule || !canEditRouting}
          >
            {submitting ? 'Saving...' : isEdit ? 'Save Changes' : 'Create Rule'}
          </Button>
          <Button type="button" variant="secondary" size="sm" onClick={handleClear}>
            Clear
          </Button>
          <button
            type="button"
            onClick={() => navigate('/routing/rules')}
            className="text-sm font-medium text-slate-500 transition-colors hover:text-slate-800 dark:text-[#8d96a8] dark:hover:text-white"
          >
            Cancel
          </button>
          <p className="text-sm text-slate-400 dark:text-[#6d7a8d]">
            {isEdit
              ? 'Saved changes apply the next time this rule is activated.'
              : 'A new rule is created inactive — activate it from the rules list.'}
          </p>
        </div>
      </form>

      <RuleSummary
        ruleName={ruleName}
        ruleBlocks={ruleBlocks}
        algorithmData={algorithmData}
      />
      </div>
    </div>
  )
}

/**
 * The right rail: what the rule currently says, in plain English, updating as it is edited.
 * The builder's controls are narrow by nature, so the page's spare width earns its keep by
 * answering "what did I just build?" instead of stretching a two-digit amount across the screen.
 */
function RuleSummary({
  ruleName,
  ruleBlocks,
  algorithmData,
}: {
  ruleName: string
  ruleBlocks: RuleBlock[]
  algorithmData: ReturnType<typeof buildAlgorithmData>
}) {
  // Render the payload that will actually be saved, through the same component the rules list
  // uses — so the rail cannot drift from either the request or the list's view of it.
  const preview = {
    id: 'preview',
    name: ruleName,
    description: '',
    created_by: '',
    algorithm: { type: 'advanced', data: algorithmData },
  } as unknown as RoutingAlgorithm

  return (
    <aside className="w-full shrink-0 xl:w-[24rem]">
      <div className="xl:sticky xl:top-6">
        <Card>
          <CardBody className="space-y-3">
            <div>
              <p className="text-xs font-semibold uppercase tracking-widest text-slate-500 dark:text-[#8d96a8]">
                Rule details
              </p>
              <p className="mt-1 truncate text-sm font-semibold text-slate-800 dark:text-slate-100">
                {ruleName.trim() || 'Untitled rule'}
              </p>
            </div>

            {ruleBlocks.length === 0 ? (
              <p className="text-[13px] leading-relaxed text-slate-500 dark:text-[#8d96a8]">
                No conditions yet — every payment will take the default fallback.
              </p>
            ) : (
              <RuleBreakdown algo={preview} />
            )}
          </CardBody>
        </Card>
      </div>
    </aside>
  )
}
