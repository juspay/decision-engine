import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { Button } from '../ui/Button'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { apiPost } from '../../lib/api'
import {
  ABTestAlgorithmData,
  EuclidAlgorithmData,
  GatewayConnector,
  RoutingAlgorithm,
} from '../../types/api'
import { ChevronDown, ChevronUp, Code, Play, Plus, Trash2 } from 'lucide-react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RuleEvaluateParams {
  key: string
  type: 'enum_variant' | 'str_value' | 'number' | 'metadata_variant'
  value: string
  metadataKey?: string
}

interface RuleEvaluateResponse {
  payment_id: string | null
  status: string
  output: {
    type: 'single' | 'priority' | 'volume_split'
    connector?: GatewayConnector
    connectors?: GatewayConnector[]
    splits?: { connector: GatewayConnector; split: number }[]
  }
  evaluated_output?: GatewayConnector[]
  eligible_connectors?: GatewayConnector[]
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_RULE_PARAMS: RuleEvaluateParams[] = [
  { key: 'payment_method', type: 'enum_variant', value: '', metadataKey: '' },
  { key: 'currency', type: 'enum_variant', value: '', metadataKey: '' },
]

const DEFAULT_FALLBACK_CONNECTORS: GatewayConnector[] = [
  { gateway_name: 'stripe', gateway_id: 'gateway_001' },
  { gateway_name: 'adyen', gateway_id: 'gateway_002' },
]

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

function mapRoutingTypeToParamType(
  keyType?: 'enum' | 'integer' | 'udf' | 'str_value' | 'global_ref',
): RuleEvaluateParams['type'] {
  if (keyType === 'enum') return 'enum_variant'
  if (keyType === 'integer') return 'number'
  if (keyType === 'udf' || keyType === 'global_ref') return 'metadata_variant'
  return 'str_value'
}

function collectFromStatement(
  statement: { condition?: { lhs: string; value?: { type: string; value: unknown } }[]; nested?: unknown[] },
  seen: Map<string, RuleEvaluateParams>,
  routingKeysConfig: Record<string, { type: string; values?: string[] }>,
) {
  for (const cond of statement.condition ?? []) {
    if (!cond.lhs || seen.has(cond.lhs)) continue
    const v = cond.value
    if (!v) continue

    let type: RuleEvaluateParams['type'] = 'enum_variant'
    let value = ''

    if (v.type === 'enum_variant' && typeof v.value === 'string') {
      type = 'enum_variant'; value = v.value
    } else if (v.type === 'enum_variant_array' && Array.isArray(v.value)) {
      type = 'enum_variant'; value = (v.value as string[])[0] ?? ''
    } else if (v.type === 'number' && typeof v.value === 'number') {
      type = 'number'; value = String(v.value)
    } else if (v.type === 'str_value' && typeof v.value === 'string') {
      type = 'str_value'; value = v.value
    } else if (v.type === 'number_array' && Array.isArray(v.value)) {
      type = 'number'; value = String((v.value as number[])[0] ?? 0)
    } else if (v.type === 'metadata_variant') {
      type = 'metadata_variant'; value = typeof v.value === 'string' ? v.value : ''
    }

    const keyConfig = routingKeysConfig[cond.lhs]
    if (keyConfig?.type === 'enum') {
      const valid = keyConfig.values ?? []
      if (valid.length > 0 && !valid.includes(value)) value = valid[0]
    }

    seen.set(cond.lhs, { key: cond.lhs, type, value, metadataKey: '' })
  }

  for (const nested of (statement.nested as typeof statement[]) ?? []) {
    collectFromStatement(nested, seen, routingKeysConfig)
  }
}

function extractRuleParams(
  algo: RoutingAlgorithm,
  routingKeysConfig: Record<string, { type: string; values?: string[] }>,
): RuleEvaluateParams[] {
  const data = algo.algorithm_data || algo.algorithm
  if (!data || data.type !== 'advanced') return []
  const program = data.data as EuclidAlgorithmData
  if (!program?.rules) return []

  const seen = new Map<string, RuleEvaluateParams>()
  for (const rule of program.rules) {
    for (const stmt of rule.statements ?? []) {
      collectFromStatement(stmt, seen, routingKeysConfig)
    }
  }
  return Array.from(seen.values())
}

function isMissingAlgorithmError(message: string) {
  return message.toLowerCase().includes('no active routing algorithm')
    || message.toLowerCase().includes('active routing algorithm is not a volume split')
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface RuleEvaluationPanelProps {
  merchantId: string
  routingKeysConfig: Record<string, { type: string; values?: string[] }>
  routingConfigUnavailable: boolean
  routingKeysLoading: boolean
  resetSignal: number
  onRunComplete: () => void
  onOpenTrace: (paymentId: string, label: string) => void
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function RuleEvaluationPanel({
  merchantId,
  routingKeysConfig,
  routingConfigUnavailable,
  routingKeysLoading,
  resetSignal,
  onRunComplete,
  onOpenTrace,
}: RuleEvaluationPanelProps) {
  const navigate = useNavigate()

  // ---- routing key names ----
  const routingKeyNames = useMemo(() => Object.keys(routingKeysConfig).sort(), [routingKeysConfig])

  // ---- active / all algorithms (for prefill) ----
  const { data: activeAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['active-routing', merchantId] : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`),
    { revalidateOnFocus: false },
  )
  const { data: allAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['routing-list', merchantId] : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`),
    { revalidateOnFocus: false },
  )

  // ---- local state ----
  const [ruleParams, setRuleParams] = useState<RuleEvaluateParams[]>(DEFAULT_RULE_PARAMS)
  const [fallbackConnectors, setFallbackConnectors] = useState<GatewayConnector[]>(DEFAULT_FALLBACK_CONNECTORS)
  const [ruleResult, setRuleResult] = useState<RuleEvaluateResponse | null>(null)
  const [prefillSourceName, setPrefillSourceName] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [setupNeeded, setSetupNeeded] = useState(false)
  const [responseOpen, setResponseOpen] = useState(false)

  // ---- reset when parent signals ----
  useEffect(() => {
    if (resetSignal === 0) return
    const filled = DEFAULT_RULE_PARAMS.map(p =>
      p.type === 'enum_variant' && p.value === ''
        ? { ...p, value: routingKeysConfig[p.key]?.values?.[0] ?? '' }
        : p,
    )
    setRuleParams(filled)
    setFallbackConnectors(DEFAULT_FALLBACK_CONNECTORS)
    setRuleResult(null)
    setPrefillSourceName(null)
    setError(null)
    setSetupNeeded(false)
  }, [resetSignal])

  // ---- prefill from active algorithm ----
  useEffect(() => {
    if (!activeAlgorithms || !allAlgorithms || routingKeysLoading || routingConfigUnavailable) return

    const activeAbTest = activeAlgorithms.find(
      a => (a.algorithm_data || a.algorithm)?.type === 'ab_test',
    )
    const activeRuleBased = activeAlgorithms.find(a => {
      const t = (a.algorithm_data || a.algorithm)?.type
      return t === 'advanced' || t === 'priority' || t === 'single'
    })

    let source: RoutingAlgorithm | undefined = activeRuleBased
    let sourceSuffix = ''

    if (!source && activeAbTest) {
      const abData = (activeAbTest.algorithm_data || activeAbTest.algorithm)?.data as ABTestAlgorithmData | undefined
      if (abData?.control_algorithm_id) {
        source = allAlgorithms.find(a => a.id === abData.control_algorithm_id)
        sourceSuffix = ' (control arm)'
      }
    }

    if (!source) return

    const extracted = extractRuleParams(source, routingKeysConfig)
    if (extracted.length === 0) return

    setRuleParams(prev => {
      const isDefault = prev.every(p => p.value === '' || DEFAULT_RULE_PARAMS.some(d => d.key === p.key))
      return isDefault ? extracted : prev
    })
    setPrefillSourceName(source.name + sourceSuffix)
  }, [activeAlgorithms, allAlgorithms, routingKeysLoading, routingConfigUnavailable, routingKeysConfig])

  // ---- param handlers ----
  function addRuleParam() {
    const key = routingKeyNames[0] ?? ''
    const keyConfig = routingKeysConfig[key]
    const type = mapRoutingTypeToParamType(keyConfig?.type as never)
    const value = type === 'enum_variant' ? (keyConfig?.values?.[0] ?? '') : ''
    setRuleParams(p => [...p, { key, type, value, metadataKey: '' }])
  }

  function removeRuleParam(idx: number) {
    setPrefillSourceName(null)
    setRuleParams(p => p.filter((_, i) => i !== idx))
  }

  function updateRuleParam(idx: number, field: keyof RuleEvaluateParams, value: string) {
    setPrefillSourceName(null)
    setRuleParams(p => p.map((item, i) => i === idx ? { ...item, [field]: value } : item))
  }

  function updateRuleParamKey(idx: number, key: string) {
    setPrefillSourceName(null)
    const keyConfig = routingKeysConfig[key]
    const type = mapRoutingTypeToParamType(keyConfig?.type as never)
    const value = type === 'enum_variant' ? (keyConfig?.values?.[0] ?? '') : ''
    setRuleParams(p => p.map((item, i) => i === idx ? { ...item, key, type, value, metadataKey: '' } : item))
  }

  function updateRuleParamMetadataKey(idx: number, value: string) {
    setPrefillSourceName(null)
    setRuleParams(p => p.map((item, i) => i === idx ? { ...item, metadataKey: value } : item))
  }

  // ---- fallback handlers ----
  function addFallbackConnector() {
    setFallbackConnectors(p => [...p, { gateway_name: '', gateway_id: '' }])
  }

  function removeFallbackConnector(idx: number) {
    setFallbackConnectors(p => p.filter((_, i) => i !== idx))
  }

  function updateFallbackConnector(idx: number, field: keyof GatewayConnector, value: string) {
    setFallbackConnectors(p => p.map((item, i) => i === idx ? { ...item, [field]: value } : item))
  }

  // ---- evaluation ----
  async function runEvaluation() {
    if (routingConfigUnavailable) {
      setError('Routing key config unavailable. Fix /config/routing-keys and retry.')
      return
    }
    setLoading(true)
    setError(null)
    setSetupNeeded(false)
    setRuleResult(null)

    const previewPaymentId = `rule_decision_${Date.now()}`
    try {
      const parameters: Record<string, { type: string; value: string | number | { key: string; value: string } }> = {}
      ruleParams.forEach(p => {
        if (!p.key) return
        if (p.type === 'metadata_variant') {
          parameters[p.key] = { type: p.type, value: { key: p.metadataKey || p.key, value: p.value } }
        } else if (p.type === 'number') {
          parameters[p.key] = { type: p.type, value: parseFloat(p.value) || 0 }
        } else if (p.value !== '') {
          parameters[p.key] = { type: p.type, value: p.value }
        }
      })

      const res = await apiPost<RuleEvaluateResponse>('/routing/evaluate', {
        created_by: merchantId || 'test_user',
        payment_id: previewPaymentId,
        fallback_output: fallbackConnectors.filter(c => c.gateway_name),
        parameters,
      })

      setRuleResult(res)
      onRunComplete()
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : 'Request failed'
      if (isMissingAlgorithmError(message)) {
        setSetupNeeded(true)
      } else {
        setError(message)
      }
    } finally {
      setLoading(false)
    }
  }

  // ---- render ----
  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">

      {/* Left — form */}
      <Card className="!rounded-2xl self-start">
        <CardHeader className="!px-5 !py-4">
          <div>
            <SurfaceLabel>Rule Evaluation</SurfaceLabel>
            <h2 className="mt-1.5 font-medium text-slate-800 dark:text-white">
              Rule Evaluation Parameters
            </h2>
          </div>
        </CardHeader>

        <CardBody className="space-y-3 !px-5 !py-4">
          {!merchantId && (
            <p className="text-xs text-amber-600 bg-amber-50 border border-amber-200 rounded px-3 py-2">
              Set a merchant ID in the top bar first.
            </p>
          )}
          {routingKeysLoading && (
            <p className="text-sm text-slate-500">Loading routing keys from backend...</p>
          )}
          {routingConfigUnavailable && (
            <ErrorMessage error="Routing keys are unavailable from backend (/config/routing-keys). Rule Evaluation is disabled." />
          )}

          {/* Parameters */}
          <div className="space-y-2">
            <div className="flex items-center gap-2 flex-wrap">
              <p className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 dark:text-[#4e5870]">
                Parameters
              </p>
              {prefillSourceName && (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-300">
                  ⚗ Prefilled from: {prefillSourceName}
                </span>
              )}
            </div>
            <div className="space-y-1.5">
              {ruleParams.map((param, idx) => (
                <div key={idx} className="space-y-1.5">
                  <div className="group flex items-center gap-0 rounded-xl border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] overflow-hidden transition-shadow hover:shadow-sm">
                    <select
                      value={param.key}
                      onChange={e => updateRuleParamKey(idx, e.target.value)}
                      disabled={routingConfigUnavailable || routingKeysLoading}
                      className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm font-medium text-slate-700 dark:text-[#c8d0de] focus:outline-none cursor-pointer appearance-none"
                    >
                      {routingKeyNames.length === 0 ? (
                        <option value="">No keys available</option>
                      ) : (
                        routingKeyNames.map(name => <option key={name} value={name}>{name}</option>)
                      )}
                    </select>
                    <span className="shrink-0 border-x border-slate-100 dark:border-[#1e2330] bg-slate-50 dark:bg-[#10131c] px-2.5 py-2.5 text-[11px] font-bold text-slate-300 dark:text-[#3a4258] select-none">=</span>
                    {param.type === 'enum_variant' ? (
                      <select
                        value={param.value}
                        onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                        className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none cursor-pointer appearance-none"
                      >
                        {(routingKeysConfig[param.key]?.values || []).map(v => (
                          <option key={v} value={v}>{v}</option>
                        ))}
                      </select>
                    ) : param.type === 'number' ? (
                      <input
                        type="number"
                        placeholder="Value"
                        value={param.value}
                        onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                        className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none"
                      />
                    ) : param.type !== 'metadata_variant' ? (
                      <input
                        placeholder="Value"
                        value={param.value}
                        onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                        className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none"
                      />
                    ) : (
                      <span className="flex-1 px-3 py-2.5 text-sm text-slate-400 dark:text-[#3a4258] italic">see below</span>
                    )}
                    <button
                      onClick={() => removeRuleParam(idx)}
                      className="shrink-0 px-2.5 py-2.5 text-slate-300 dark:text-[#2a3040] hover:text-red-400 dark:hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                  {param.type === 'metadata_variant' && (
                    <div className="ml-3 flex gap-1.5">
                      <input
                        placeholder="Metadata key"
                        value={param.metadataKey || ''}
                        onChange={e => updateRuleParamMetadataKey(idx, e.target.value)}
                        className="flex-1 rounded-lg border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] px-3 py-2 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none focus:ring-1 focus:ring-brand-500"
                      />
                      <input
                        placeholder="Metadata value"
                        value={param.value}
                        onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                        className="flex-1 rounded-lg border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] px-3 py-2 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none focus:ring-1 focus:ring-brand-500"
                      />
                    </div>
                  )}
                </div>
              ))}
            </div>
            <button
              onClick={addRuleParam}
              disabled={routingConfigUnavailable || routingKeysLoading || routingKeyNames.length === 0}
              className="flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs font-medium text-brand-500 hover:bg-brand-50 dark:hover:bg-brand-500/10 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              <Plus size={12} /> Add Parameter
            </button>
          </div>

          {/* Fallback Gateways */}
          <div className="space-y-2">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 dark:text-[#4e5870]">
              Fallback Gateways
            </p>
            <div className="space-y-1.5">
              {fallbackConnectors.map((connector, idx) => (
                <div key={idx} className="group flex items-center gap-0 rounded-xl border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] overflow-hidden transition-shadow hover:shadow-sm">
                  <span className="shrink-0 flex items-center justify-center w-8 self-stretch bg-slate-50 dark:bg-[#10131c] border-r border-slate-100 dark:border-[#1e2330] text-[10px] font-bold text-slate-300 dark:text-[#3a4258] select-none">
                    {idx + 1}
                  </span>
                  <input
                    placeholder="gateway name"
                    value={connector.gateway_name}
                    onChange={e => updateFallbackConnector(idx, 'gateway_name', e.target.value)}
                    className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm font-medium text-slate-700 dark:text-[#c8d0de] focus:outline-none"
                  />
                  <span className="shrink-0 border-x border-slate-100 dark:border-[#1e2330] bg-slate-50 dark:bg-[#10131c] px-2 py-2.5 text-[11px] font-bold text-slate-300 dark:text-[#3a4258] select-none">/</span>
                  <input
                    placeholder="gateway id (optional)"
                    value={connector.gateway_id || ''}
                    onChange={e => updateFallbackConnector(idx, 'gateway_id', e.target.value)}
                    className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-500 dark:text-[#8090a8] focus:outline-none"
                  />
                  <button
                    onClick={() => removeFallbackConnector(idx)}
                    className="shrink-0 px-2.5 py-2.5 text-slate-300 dark:text-[#2a3040] hover:text-red-400 dark:hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              ))}
            </div>
            <button
              onClick={addFallbackConnector}
              className="flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs font-medium text-brand-500 hover:bg-brand-50 dark:hover:bg-brand-500/10 transition-colors"
            >
              <Plus size={12} /> Add Gateway
            </button>
          </div>
        </CardBody>

        <div className="border-t border-slate-200 dark:border-[#2a303a] px-5 py-4 space-y-3">
          <ErrorMessage error={error} />
          {setupNeeded && (
            <div className="rounded-lg border border-amber-200 bg-amber-50 dark:border-amber-500/30 dark:bg-amber-500/10 px-3 py-2.5 text-xs text-amber-700 dark:text-amber-300">
              <p className="font-semibold mb-1">Configure rule-based routing first</p>
              <p className="mb-2">Rule evaluation needs an active rule-based strategy before it can return a policy decision.</p>
              <Button size="sm" variant="secondary" onClick={() => navigate('/routing/rules')}>
                Configure routing
              </Button>
            </div>
          )}
          <Button
            onClick={runEvaluation}
            disabled={loading || routingConfigUnavailable || !merchantId}
            className="w-full justify-center"
          >
            {loading ? <><Spinner size={14} /> Evaluating…</> : <><Play size={14} /> Evaluate Rules</>}
          </Button>
        </div>
      </Card>

      {/* Right — results */}
      <div className="space-y-4">
        {ruleResult ? (
          <>
            <Card>
              <CardBody>
                <div className="flex items-start justify-between mb-3">
                  <div>
                    <p className="text-xs text-slate-500 uppercase tracking-wide mb-1">Status</p>
                    <p className="text-2xl font-bold text-slate-900">{ruleResult.status}</p>
                    <p className="text-xs text-slate-500 mt-1">output_type: {ruleResult.output.type}</p>
                  </div>
                  {ruleResult.payment_id && (
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => onOpenTrace(ruleResult.payment_id!, 'Rule Evaluation Decision')}
                    >
                      View decision trace
                    </Button>
                  )}
                </div>

                {ruleResult.output.type === 'single' && ruleResult.output.connector && (
                  <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-3">
                    <p className="text-xs text-slate-400 mb-1">Selected gateway_name</p>
                    <p className="text-lg font-semibold">{ruleResult.output.connector.gateway_name}</p>
                    {ruleResult.output.connector.gateway_id && (
                      <p className="text-xs text-slate-500">gateway_id: {ruleResult.output.connector.gateway_id}</p>
                    )}
                  </div>
                )}

                {ruleResult.output.type === 'priority' && ruleResult.output.connectors && (
                  <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-3">
                    <p className="text-xs text-slate-400 mb-2">Priority gateway_name list</p>
                    <div className="space-y-1">
                      {ruleResult.output.connectors.map((gw, idx) => (
                        <div key={idx} className="flex items-center gap-2 text-sm">
                          <span className="w-5 h-5 rounded-full bg-brand-500 text-white text-xs flex items-center justify-center">{idx + 1}</span>
                          <span className="font-medium">{gw.gateway_name}</span>
                          {gw.gateway_id && <span className="text-xs text-slate-500">({gw.gateway_id})</span>}
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {ruleResult.output.type === 'volume_split' && (
                  <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-3">
                    <p className="text-xs text-slate-400 mb-2">Volume Split Result</p>
                    <p className="text-sm text-slate-600">See Volume Split tab for detailed visualization.</p>
                  </div>
                )}
              </CardBody>
            </Card>

            <Card>
              <CardHeader>
                <button
                  onClick={() => setResponseOpen(o => !o)}
                  className="flex items-center justify-between w-full text-sm font-medium text-slate-800"
                >
                  <span className="flex items-center gap-2">
                    <Code size={14} />
                    API Response
                  </span>
                  {responseOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                </button>
              </CardHeader>
              {responseOpen && (
                <CardBody className="p-0">
                  <pre className="text-xs text-slate-600 bg-slate-50 dark:bg-[#0a0a0f] p-4 overflow-auto max-h-96 font-mono">
                    {JSON.stringify(ruleResult, null, 2)}
                  </pre>
                </CardBody>
              )}
            </Card>
          </>
        ) : (
          <Card>
            <CardBody className="py-16 text-center">
              <Play size={32} className="text-gray-300 mx-auto mb-3" />
              <p className="text-slate-400 text-sm">
                Configure rule parameters and click "Evaluate Rules" to test routing.
              </p>
            </CardBody>
          </Card>
        )}
      </div>
    </div>
  )
}
