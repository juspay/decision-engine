import { useState } from 'react'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell, PieChart, Pie } from 'recharts'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { DecideGatewayResponse, GatewayConnector } from '../../types/api'
import { ROUTING_APPROACH_COLORS } from '../../lib/constants'
import { Play, RefreshCw, ChevronDown, ChevronUp, Activity, Code, Plus, Trash2, PieChart as PieChartIcon } from 'lucide-react'

const PAYMENT_METHOD_TYPES = ['CARD', 'UPI', 'WALLET', 'NETBANKING', 'interac']
const PAYMENT_METHODS = ['CREDIT', 'DEBIT', 'PREPAID']
const CURRENCIES = ['USD', 'EUR', 'GBP', 'INR', 'SGD', 'AUD', 'CAD']
const CARD_BRANDS = ['VISA', 'MASTERCARD', 'AMEX', 'RUPAY', 'DINERS']
const AUTH_TYPES = ['THREE_DS', 'NO_THREE_DS']
const ALGORITHMS = ['SR_BASED_ROUTING', 'PL_BASED_ROUTING', 'NTW_BASED_ROUTING']

const ALGORITHM_LABELS: Record<string, string> = {
  'SR_BASED_ROUTING': 'Success Rate Based',
  'PL_BASED_ROUTING': 'Priority List Based',
  'NTW_BASED_ROUTING': 'Network Based'
}

type TabType = 'single' | 'batch' | 'rule' | 'volume'

interface FormState {
  amount: string
  currency: string
  payment_method_type: string
  payment_method: string
  card_brand: string
  auth_type: string
  eligible_gateways: string
  ranking_algorithm: string
  elimination_enabled: boolean
}

interface SimulationConfig {
  totalPayments: string
  successCount: string
  failureCount: string
}

interface SimulationResult {
  paymentId: string
  decidedGateway: string
  status: 'CHARGED' | 'FAILURE'
  timestamp: string
}

interface RuleEvaluateParams {
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

function approachColor(approach: string): string {
  for (const [k, v] of Object.entries(ROUTING_APPROACH_COLORS)) {
    if (approach.includes(k) || k.includes(approach)) return v
  }
  return 'bg-white/5 text-gray-600 ring-1 ring-inset ring-white/8'
}

const COLORS = ['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16']

export function DecisionExplorerPage() {
  const { merchantId } = useMerchantStore()
  const [activeTab, setActiveTab] = useState<TabType>('single')
  
  const [form, setForm] = useState<FormState>({
    amount: '1000',
    currency: 'USD',
    payment_method_type: 'CARD',
    payment_method: 'CREDIT',
    card_brand: 'VISA',
    auth_type: 'THREE_DS',
    eligible_gateways: 'stripe, adyen',
    ranking_algorithm: 'SR_BASED_ROUTING',
    elimination_enabled: false,
  })
  
  const [simulationConfig, setSimulationConfig] = useState<SimulationConfig>({
    totalPayments: '10',
    successCount: '7',
    failureCount: '3',
  })
  
  const [ruleParams, setRuleParams] = useState<RuleEvaluateParams[]>([
    { key: 'payment_method_type', type: 'enum_variant', value: 'credit', metadataKey: '' },
    { key: 'currency', type: 'enum_variant', value: 'USD', metadataKey: '' },
  ])
  
  const [fallbackConnectors, setFallbackConnectors] = useState<GatewayConnector[]>([
    { gateway_name: 'stripe', gateway_id: 'mca_001' },
    { gateway_name: 'adyen', gateway_id: 'mca_002' },
  ])
  
  const [volumePayments, setVolumePayments] = useState<string>('100')
  
  const [result, setResult] = useState<DecideGatewayResponse | null>(null)
  const [ruleResult, setRuleResult] = useState<RuleEvaluateResponse | null>(null)
  const [volumeDistribution, setVolumeDistribution] = useState<{ name: string; count: number; percentage: number }[]>([])
  const [simulationResults, setSimulationResults] = useState<SimulationResult[]>([])
  const [isSimulating, setIsSimulating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [filterOpen, setFilterOpen] = useState(false)
  const [responseOpen, setResponseOpen] = useState(false)
  const [volumeResponseOpen, setVolumeResponseOpen] = useState(false)

  function set(field: keyof FormState, value: string | boolean) {
    setForm(f => ({ ...f, [field]: value }))
  }

  function addRuleParam() {
    setRuleParams([...ruleParams, { key: '', type: 'enum_variant', value: '', metadataKey: '' }])
  }

  function removeRuleParam(index: number) {
    setRuleParams(ruleParams.filter((_, i) => i !== index))
  }

  function updateRuleParam(index: number, field: keyof RuleEvaluateParams, value: string) {
    setRuleParams(ruleParams.map((p, i) => i === index ? { ...p, [field]: value } : p))
  }

  function updateRuleParamMetadataKey(index: number, value: string) {
    setRuleParams(ruleParams.map((p, i) => i === index ? { ...p, metadataKey: value } : p))
  }

  function addFallbackConnector() {
    setFallbackConnectors([...fallbackConnectors, { gateway_name: '', gateway_id: '' }])
  }

  function removeFallbackConnector(index: number) {
    setFallbackConnectors(fallbackConnectors.filter((_, i) => i !== index))
  }

  function updateFallbackConnector(index: number, field: keyof GatewayConnector, value: string) {
    setFallbackConnectors(fallbackConnectors.map((c, i) => i === index ? { ...c, [field]: value } : c))
  }

  async function run() {
    if (!merchantId) return setError('Set a merchant ID in the top bar')
    setLoading(true); setError(null)
    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    try {
      const res = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        merchantId: merchantId,
        paymentInfo: {
          paymentId: `explorer_${Date.now()}`,
          amount: parseFloat(form.amount) || 1000,
          currency: form.currency,
          paymentType: 'ORDER_PAYMENT',
          paymentMethodType: form.payment_method_type,
          paymentMethod: form.payment_method,
          authType: form.auth_type,
          cardBrand: form.card_brand,
        },
        eligibleGatewayList: gateways,
        rankingAlgorithm: form.ranking_algorithm,
        eliminationEnabled: form.elimination_enabled,
      })
      setResult(res)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  async function runSimulation() {
    if (!merchantId) return setError('Set a merchant ID in the top bar')
    
    const total = parseInt(simulationConfig.totalPayments) || 0
    const success = parseInt(simulationConfig.successCount) || 0
    const failure = parseInt(simulationConfig.failureCount) || 0
    
    if (total <= 0) return setError('Total Payments must be greater than 0')
    if (success + failure !== total) {
      return setError('Success + Failure count must equal Total Payments')
    }
    
    setIsSimulating(true)
    setError(null)
    setSimulationResults([])
    
    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    const results: SimulationResult[] = []
    
    const outcomes: ('CHARGED' | 'FAILURE')[] = [
      ...Array(success).fill('CHARGED'),
      ...Array(failure).fill('FAILURE'),
    ]
    
    for (let i = outcomes.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [outcomes[i], outcomes[j]] = [outcomes[j], outcomes[i]]
    }
    
    try {
      for (let i = 0; i < total; i++) {
        const paymentId = `sim_${Date.now()}_${i}`
        
        const decideRes = await apiPost<DecideGatewayResponse>('/decide-gateway', {
          merchantId: merchantId,
          paymentInfo: {
            paymentId: paymentId,
            amount: parseFloat(form.amount) || 1000,
            currency: form.currency,
            paymentType: 'ORDER_PAYMENT',
            paymentMethodType: form.payment_method_type,
            paymentMethod: form.payment_method,
            authType: form.auth_type,
            cardBrand: form.card_brand,
          },
          eligibleGatewayList: gateways,
          rankingAlgorithm: form.ranking_algorithm,
          eliminationEnabled: form.elimination_enabled,
        })
        
        const decidedGateway = decideRes.decided_gateway
        const outcome = outcomes[i]
        
        await apiPost('/update-gateway-score', {
          merchantId: merchantId,
          gateway: decidedGateway,
          gatewayReferenceId: null,
          status: outcome,
          paymentId: paymentId,
          enforceDynamicRoutingFailure: null,
        })
        
        results.push({
          paymentId,
          decidedGateway,
          status: outcome,
          timestamp: new Date().toISOString(),
        })
        
        setSimulationResults([...results])
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Simulation failed')
    } finally {
      setIsSimulating(false)
    }
  }

  async function runRuleEvaluation() {
    setLoading(true)
    setError(null)
    setRuleResult(null)
    setVolumeDistribution([])
    
    try {
      const parameters: Record<string, { type: string; value: string | number | { key: string; value: string } }> = {}
      ruleParams.forEach(p => {
        if (p.key) {
          if (p.type === 'metadata_variant') {
            parameters[p.key] = { 
              type: p.type, 
              value: { key: p.metadataKey || p.key, value: p.value } 
            }
          } else if (p.type === 'number') {
            parameters[p.key] = { type: p.type, value: parseFloat(p.value) || 0 }
          } else {
            parameters[p.key] = { type: p.type, value: p.value }
          }
        }
      })
      
      const res = await apiPost<RuleEvaluateResponse>('/routing/evaluate', {
        created_by: merchantId || 'test_user',
        fallback_output: fallbackConnectors.filter(c => c.gateway_name),
        parameters,
      })
      
      setRuleResult(res)
      
      if (res.output.type === 'volume_split' && res.output.splits) {
        const totalPayments = parseInt(volumePayments) || 100
        const distribution = res.output.splits.map(item => ({
          name: item.connector.gateway_name,
          count: Math.round((item.split / 100) * totalPayments),
          percentage: item.split,
        }))
        setVolumeDistribution(distribution)
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  async function runVolumeSplit() {
    setLoading(true)
    setError(null)
    setVolumeDistribution([])
    
    try {
      const res = await apiPost<RuleEvaluateResponse>('/routing/evaluate', {
        created_by: merchantId || 'test_user',
        fallback_output: [
          { gateway_name: 'stripe', gateway_id: 'mca_001' },
          { gateway_name: 'adyen', gateway_id: 'mca_002' },
        ],
        parameters: {},
      })
      
      setRuleResult(res)
      
      if (res.output.type === 'volume_split' && res.output.splits) {
        const totalPayments = parseInt(volumePayments) || 100
        const distribution = res.output.splits.map(item => ({
          name: item.connector.gateway_name,
          count: Math.round((item.split / 100) * totalPayments),
          percentage: item.split,
        }))
        setVolumeDistribution(distribution)
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  const scoreData = result?.gateway_priority_map
    ? Object.entries(result.gateway_priority_map)
        .sort(([, a], [, b]) => b - a)
        .map(([name, score]) => ({ name, score: Math.round(score * 1000) / 10 }))
    : []

  const gatewayStats = simulationResults.reduce((acc, curr) => {
    if (!acc[curr.decidedGateway]) {
      acc[curr.decidedGateway] = { total: 0, success: 0, failure: 0 }
    }
    acc[curr.decidedGateway].total++
    if (curr.status === 'CHARGED') acc[curr.decidedGateway].success++
    else acc[curr.decidedGateway].failure++
    return acc
  }, {} as Record<string, { total: number; success: number; failure: number }>)

  const pieData = volumeDistribution.map(d => ({ name: d.name, value: d.count }))

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-gray-900">Decision Explorer</h1>
        <p className="text-gray-500 mt-1 text-sm">
          Test payment routing with different algorithms: Success Rate, Priority List, Rule-Based, or Volume Split.
        </p>
      </div>

      <div className="flex gap-2 border-b border-[#1c1c24]">
        <button
          onClick={() => setActiveTab('single')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'single' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-gray-500 hover:text-gray-700'}`}
        >
          Single Test
        </button>
        <button
          onClick={() => setActiveTab('batch')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'batch' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-gray-500 hover:text-gray-700'}`}
        >
          Batch Simulation
        </button>
        <button
          onClick={() => setActiveTab('rule')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'rule' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-gray-500 hover:text-gray-700'}`}
        >
          Rule-Based
        </button>
        <button
          onClick={() => setActiveTab('volume')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'volume' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-gray-500 hover:text-gray-700'}`}
        >
          Volume Split
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <h2 className="font-medium text-gray-800">
              {activeTab === 'rule' ? 'Rule Evaluation Parameters' : 
               activeTab === 'volume' ? 'Volume Split Configuration' : 
               'Payment Parameters'}
            </h2>
          </CardHeader>
          <CardBody className="space-y-3">
            {!merchantId && activeTab !== 'volume' && (
              <p className="text-xs text-amber-600 bg-amber-50 border border-amber-200 rounded px-3 py-2">
                Set a merchant ID in the top bar first.
              </p>
            )}

            {activeTab === 'rule' ? (
              <>
                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-1">Parameters</label>
                  <div className="space-y-2">
                    {ruleParams.map((param, idx) => (
                      <div key={idx} className="space-y-2">
                        <div className="flex gap-2 items-center">
                          <input
                            placeholder="Key (e.g. payment_method_type)"
                            value={param.key}
                            onChange={e => updateRuleParam(idx, 'key', e.target.value)}
                            className="flex-1 border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                          />
                          <select
                            value={param.type}
                            onChange={e => updateRuleParam(idx, 'type', e.target.value as RuleEvaluateParams['type'])}
                            className="w-36 border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                          >
                            <option value="enum_variant">enum_variant</option>
                            <option value="str_value">str_value</option>
                            <option value="number">number</option>
                            <option value="metadata_variant">metadata_variant</option>
                          </select>
                          <button
                            onClick={() => removeRuleParam(idx)}
                            className="p-1.5 text-gray-400 hover:text-red-500"
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                        {param.type === 'metadata_variant' ? (
                          <div className="flex gap-2 items-center pl-1">
                            <input
                              placeholder="Metadata Key"
                              value={param.metadataKey || ''}
                              onChange={e => updateRuleParamMetadataKey(idx, e.target.value)}
                              className="flex-1 border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                            <input
                              placeholder="Metadata Value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </div>
                        ) : (
                          <div className="flex gap-2 items-center pl-1">
                            <input
                              placeholder="Value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                  <button
                    onClick={addRuleParam}
                    className="mt-2 flex items-center gap-1 text-xs text-brand-500 hover:text-brand-600"
                  >
                    <Plus size={12} /> Add Parameter
                  </button>
                </div>

                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-1">Fallback Connectors</label>
                  <div className="space-y-2">
                    {fallbackConnectors.map((connector, idx) => (
                      <div key={idx} className="flex gap-2 items-center">
                        <input
                          placeholder="Gateway Name"
                          value={connector.gateway_name}
                          onChange={e => updateFallbackConnector(idx, 'gateway_name', e.target.value)}
                          className="flex-1 border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                        <input
                          placeholder="Gateway ID"
                          value={connector.gateway_id || ''}
                          onChange={e => updateFallbackConnector(idx, 'gateway_id', e.target.value)}
                          className="flex-1 border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                        <button
                          onClick={() => removeFallbackConnector(idx)}
                          className="p-1.5 text-gray-400 hover:text-red-500"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    ))}
                  </div>
                  <button
                    onClick={addFallbackConnector}
                    className="mt-2 flex items-center gap-1 text-xs text-brand-500 hover:text-brand-600"
                  >
                    <Plus size={12} /> Add Connector
                  </button>
                </div>
              </>
            ) : activeTab === 'volume' ? (
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">Number of Payments</label>
                <input
                  type="text"
                  value={volumePayments}
                  onChange={e => setVolumePayments(e.target.value)}
                  className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <p className="text-xs text-gray-500 mt-1">
                  Enter the total number of payments to visualize how they would be distributed across connectors.
                </p>
              </div>
            ) : (
              <>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">Amount</label>
                    <input value={form.amount} onChange={e => set('amount', e.target.value)}
                      className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500" />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">Currency</label>
                    <select value={form.currency} onChange={e => set('currency', e.target.value)}
                      className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {CURRENCIES.map(c => <option key={c}>{c}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">Payment Method Type</label>
                    <select value={form.payment_method_type} onChange={e => set('payment_method_type', e.target.value)}
                      className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {PAYMENT_METHOD_TYPES.map(p => <option key={p}>{p}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">Payment Method</label>
                    <select value={form.payment_method} onChange={e => set('payment_method', e.target.value)}
                      className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {PAYMENT_METHODS.map(p => <option key={p}>{p}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">Card Brand</label>
                    <select value={form.card_brand} onChange={e => set('card_brand', e.target.value)}
                      className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {CARD_BRANDS.map(b => <option key={b}>{b}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">Auth Type</label>
                    <select value={form.auth_type} onChange={e => set('auth_type', e.target.value)}
                      className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {AUTH_TYPES.map(a => <option key={a}>{a}</option>)}
                    </select>
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium text-gray-600 mb-1">Eligible Gateways (comma-separated)</label>
                  <input value={form.eligible_gateways} onChange={e => set('eligible_gateways', e.target.value)}
                    placeholder="stripe, adyen"
                    className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500" />
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-gray-600 mb-1">Algorithm</label>
                    <select value={form.ranking_algorithm} onChange={e => set('ranking_algorithm', e.target.value)}
                      className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {ALGORITHMS.map(a => <option key={a} value={a}>{ALGORITHM_LABELS[a]}</option>)}
                    </select>
                  </div>
                  <div className="flex items-end pb-1">
                    <label className="flex items-center gap-2 text-sm text-gray-700 cursor-pointer">
                      <input type="checkbox" checked={form.elimination_enabled}
                        onChange={e => set('elimination_enabled', e.target.checked)}
                        className="rounded" />
                      Elimination enabled
                    </label>
                  </div>
                </div>

                {activeTab === 'batch' && (
                  <div className="border-t border-[#1c1c24] pt-4 mt-4 space-y-3">
                    <h3 className="text-sm font-medium text-gray-800 flex items-center gap-2">
                      <Activity size={14} />
                      Simulation Configuration
                    </h3>
                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-xs font-medium text-gray-600 mb-1">Total Payments</label>
                        <input
                          type="text"
                          value={simulationConfig.totalPayments}
                          onChange={e => setSimulationConfig(s => ({ ...s, totalPayments: e.target.value }))}
                          className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-medium text-gray-600 mb-1">Success Count</label>
                        <input
                          type="text"
                          value={simulationConfig.successCount}
                          onChange={e => setSimulationConfig(s => ({ ...s, successCount: e.target.value }))}
                          className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-medium text-gray-600 mb-1">Failure Count</label>
                        <input
                          type="text"
                          value={simulationConfig.failureCount}
                          onChange={e => setSimulationConfig(s => ({ ...s, failureCount: e.target.value }))}
                          className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                      </div>
                    </div>
                    <p className="text-xs text-gray-500">
                      Will run {simulationConfig.totalPayments || 0} payments: {simulationConfig.successCount || 0} SUCCESS, {simulationConfig.failureCount || 0} FAILURE
                    </p>
                  </div>
                )}
              </>
            )}

            <ErrorMessage error={error} />

            {activeTab === 'rule' ? (
              <Button onClick={runRuleEvaluation} disabled={loading} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Evaluating…</> : <><Play size={14} /> Evaluate Rules</>}
              </Button>
            ) : activeTab === 'volume' ? (
              <Button onClick={runVolumeSplit} disabled={loading} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Calculating…</> : <><PieChartIcon size={14} /> Visualize Distribution</>}
              </Button>
            ) : activeTab === 'batch' ? (
              <Button onClick={runSimulation} disabled={isSimulating || !merchantId} className="w-full justify-center">
                {isSimulating ? (
                  <>
                    <Spinner size={14} />
                    Simulating {simulationResults.length}/{simulationConfig.totalPayments || 0}...
                  </>
                ) : (
                  <>
                    <Activity size={14} /> Run Batch Simulation
                  </>
                )}
              </Button>
            ) : (
              <Button onClick={run} disabled={loading || !merchantId} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Running…</> : <><Play size={14} /> Run Decision</>}
              </Button>
            )}
          </CardBody>
        </Card>

        <div className="space-y-4">
          {activeTab === 'volume' ? (
            volumeDistribution.length > 0 ? (
              <>
                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Volume Distribution Overview</h3>
                  </CardHeader>
                  <CardBody>
                    <div className="text-center mb-4">
                      <p className="text-3xl font-bold text-gray-900">{volumePayments}</p>
                      <p className="text-xs text-gray-500">Total Payments</p>
                    </div>
                    <div className="grid grid-cols-2 gap-4">
                      {volumeDistribution.map((item, idx) => (
                        <div key={idx} className="bg-gray-50 rounded-lg p-3">
                          <div className="flex items-center gap-2 mb-1">
                            <div
                              className="w-3 h-3 rounded"
                              style={{ backgroundColor: COLORS[idx % COLORS.length] }}
                            />
                            <span className="font-medium text-sm">{item.name}</span>
                          </div>
                          <div className="flex justify-between text-xs text-gray-500">
                            <span>{item.percentage}%</span>
                            <span className="font-medium text-gray-700">{item.count} payments</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Pie Chart</h3>
                  </CardHeader>
                  <CardBody>
                    <ResponsiveContainer width="100%" height={250}>
                      <PieChart>
                        <Pie
                          data={pieData}
                          cx="50%"
                          cy="50%"
                          innerRadius={60}
                          outerRadius={100}
                          paddingAngle={3}
                          dataKey="value"
                          label={({ name, percent }) => `${name} ${(percent * 100).toFixed(0)}%`}
                          labelLine={false}
                        >
                          {pieData.map((_, index) => (
                            <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                          ))}
                        </Pie>
                        <Tooltip 
                          formatter={(value: number) => [`${value} payments`, 'Count']}
                          contentStyle={{ backgroundColor: '#fff', border: '1px solid #e5e7eb', borderRadius: '8px' }}
                        />
                      </PieChart>
                    </ResponsiveContainer>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Bar Chart</h3>
                  </CardHeader>
                  <CardBody>
                    <ResponsiveContainer width="100%" height={volumeDistribution.length * 50 + 40}>
                      <BarChart data={volumeDistribution} layout="vertical" margin={{ left: 20, right: 40 }}>
                        <XAxis type="number" tick={{ fontSize: 12, fill: '#666' }} axisLine={{ stroke: '#e5e7eb' }} tickLine={false} />
                        <YAxis type="category" dataKey="name" tick={{ fontSize: 12, fill: '#666' }} width={80} axisLine={false} tickLine={false} />
                        <Tooltip 
                          formatter={(value: number) => [`${value} payments`, 'Count']}
                          contentStyle={{ backgroundColor: '#fff', border: '1px solid #e5e7eb', borderRadius: '8px' }}
                        />
                        <Bar dataKey="count" radius={[0, 6, 6, 0]}>
                          {volumeDistribution.map((_, index) => (
                            <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Percentage Distribution</h3>
                  </CardHeader>
                  <CardBody>
                    <div className="h-4 rounded-full overflow-hidden flex">
                      {volumeDistribution.map((item, idx) => (
                        <div
                          key={idx}
                          style={{ 
                            width: `${item.percentage}%`, 
                            backgroundColor: COLORS[idx % COLORS.length] 
                          }}
                          className="h-full transition-all duration-300"
                          title={`${item.name}: ${item.percentage}%`}
                        />
                      ))}
                    </div>
                    <div className="flex flex-wrap gap-3 mt-3">
                      {volumeDistribution.map((item, idx) => (
                        <div key={idx} className="flex items-center gap-1.5 text-xs">
                          <div
                            className="w-2.5 h-2.5 rounded-sm"
                            style={{ backgroundColor: COLORS[idx % COLORS.length] }}
                          />
                          <span className="text-gray-600">{item.name}</span>
                          <span className="font-medium">{item.percentage}%</span>
                        </div>
                      ))}
                    </div>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Connector Summary</h3>
                  </CardHeader>
                  <CardBody className="p-0">
                    <table className="w-full text-sm">
                      <thead className="bg-gray-50 text-xs text-gray-500">
                        <tr>
                          <th className="text-left px-4 py-2">Connector</th>
                          <th className="text-right px-4 py-2">Payments</th>
                          <th className="text-right px-4 py-2">Percentage</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-gray-100">
                        {volumeDistribution.map((item, idx) => (
                          <tr key={idx} className="hover:bg-gray-50">
                            <td className="px-4 py-2">
                              <div className="flex items-center gap-2">
                                <div
                                  className="w-3 h-3 rounded"
                                  style={{ backgroundColor: COLORS[idx % COLORS.length] }}
                                />
                                <span className="font-medium">{item.name}</span>
                              </div>
                            </td>
                            <td className="px-4 py-2 text-right font-medium">{item.count}</td>
                            <td className="px-4 py-2 text-right text-gray-500">{item.percentage}%</td>
                          </tr>
                        ))}
                        <tr className="bg-gray-50 font-medium">
                          <td className="px-4 py-2">Total</td>
                          <td className="px-4 py-2 text-right">{volumePayments}</td>
                          <td className="px-4 py-2 text-right">100%</td>
                        </tr>
                      </tbody>
                    </table>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Payment Log</h3>
                  </CardHeader>
                  <CardBody className="p-0 max-h-80 overflow-auto">
                    <table className="w-full text-sm">
                      <thead className="bg-gray-50 text-xs text-gray-500 sticky top-0">
                        <tr>
                          <th className="text-left px-4 py-2 w-20">#</th>
                          <th className="text-left px-4 py-2">Connector</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-gray-100">
                        {Array.from({ length: parseInt(volumePayments) || 0 }).map((_, idx) => {
                          let cumulative = 0
                          let connector = volumeDistribution[0]?.name || ''
                          let colorIdx = 0
                          
                          for (let i = 0; i < volumeDistribution.length; i++) {
                            cumulative += volumeDistribution[i].count
                            if (idx < cumulative) {
                              connector = volumeDistribution[i].name
                              colorIdx = i
                              break
                            }
                          }
                          
                          return (
                            <tr key={idx} className="hover:bg-gray-50">
                              <td className="px-4 py-1.5 text-gray-500 font-mono text-xs">{idx + 1}</td>
                              <td className="px-4 py-1.5">
                                <div className="flex items-center gap-2">
                                  <div
                                    className="w-2 h-2 rounded"
                                    style={{ backgroundColor: COLORS[colorIdx % COLORS.length] }}
                                  />
                                  <span className="font-medium">{connector}</span>
                                </div>
                              </td>
                            </tr>
                          )
                        })}
                      </tbody>
                    </table>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <button
                      onClick={() => setVolumeResponseOpen(o => !o)}
                      className="flex items-center justify-between w-full text-sm font-medium text-gray-800"
                    >
                      <span className="flex items-center gap-2">
                        <Code size={14} />
                        API Response
                      </span>
                      {volumeResponseOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                    </button>
                  </CardHeader>
                  {volumeResponseOpen && ruleResult && (
                    <CardBody className="p-0">
                      <pre className="text-xs text-gray-600 bg-[#0a0a0f] p-4 overflow-auto max-h-96 font-mono">
                        {JSON.stringify(ruleResult, null, 2)}
                      </pre>
                    </CardBody>
                  )}
                </Card>
              </>
            ) : (
              <Card>
                <CardBody className="py-16 text-center">
                  <PieChartIcon size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-gray-400 text-sm">Enter the number of payments and click "Visualize Distribution" to see how payments are split across connectors.</p>
                </CardBody>
              </Card>
            )
          ) : activeTab === 'rule' ? (
            ruleResult ? (
              <>
                <Card>
                  <CardBody>
                    <div className="flex items-start justify-between mb-3">
                      <div>
                        <p className="text-xs text-gray-500 uppercase tracking-wide mb-1">Output Type</p>
                        <p className="text-2xl font-bold text-gray-900">{ruleResult.output.type}</p>
                      </div>
                    </div>
                    
                    {ruleResult.output.type === 'single' && ruleResult.output.connector && (
                      <div className="border-t border-[#1c1c24] pt-3">
                        <p className="text-xs text-gray-400 mb-1">Selected Gateway</p>
                        <p className="text-lg font-semibold">{ruleResult.output.connector.gateway_name}</p>
                        {ruleResult.output.connector.gateway_id && (
                          <p className="text-xs text-gray-500">ID: {ruleResult.output.connector.gateway_id}</p>
                        )}
                      </div>
                    )}
                    
                    {ruleResult.output.type === 'priority' && ruleResult.output.connectors && (
                      <div className="border-t border-[#1c1c24] pt-3">
                        <p className="text-xs text-gray-400 mb-2">Priority List</p>
                        <div className="space-y-1">
                          {ruleResult.output.connectors.map((gw, idx) => (
                            <div key={idx} className="flex items-center gap-2 text-sm">
                              <span className="w-5 h-5 rounded-full bg-brand-500 text-white text-xs flex items-center justify-center">{idx + 1}</span>
                              <span className="font-medium">{gw.gateway_name}</span>
                              {gw.gateway_id && <span className="text-xs text-gray-500">({gw.gateway_id})</span>}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {ruleResult.output.type === 'volume_split' && (
                      <div className="border-t border-[#1c1c24] pt-3">
                        <p className="text-xs text-gray-400 mb-2">Volume Split Result</p>
                        <p className="text-sm text-gray-600">See Volume Split tab for detailed visualization.</p>
                      </div>
                    )}
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <button
                      onClick={() => setResponseOpen(o => !o)}
                      className="flex items-center justify-between w-full text-sm font-medium text-gray-800"
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
                      <pre className="text-xs text-gray-600 bg-[#0a0a0f] p-4 overflow-auto max-h-96 font-mono">
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
                  <p className="text-gray-400 text-sm">Configure rule parameters and click "Evaluate Rules" to test routing.</p>
                </CardBody>
              </Card>
            )
          ) : activeTab === 'batch' ? (
            simulationResults.length > 0 ? (
              <>
                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Simulation Progress</h3>
                  </CardHeader>
                  <CardBody>
                    <div className="mb-4">
                      <div className="flex justify-between text-xs text-gray-600 mb-1">
                        <span>Progress</span>
                        <span>{Math.round((simulationResults.length / (parseInt(simulationConfig.totalPayments) || 1)) * 100)}%</span>
                      </div>
                      <div className="w-full bg-gray-200 rounded-full h-2">
                        <div
                          className="bg-brand-500 h-2 rounded-full transition-all duration-300"
                          style={{ width: `${(simulationResults.length / (parseInt(simulationConfig.totalPayments) || 1)) * 100}%` }}
                        />
                      </div>
                    </div>
                    
                    {Object.keys(gatewayStats).length > 0 && (
                      <div className="space-y-2">
                        <h4 className="text-xs font-medium text-gray-700">Gateway Selection Summary</h4>
                        {Object.entries(gatewayStats).map(([gateway, stats]) => (
                          <div key={gateway} className="flex items-center justify-between text-sm">
                            <span className="font-medium">{gateway}</span>
                            <div className="flex gap-3 text-xs">
                              <span className="text-emerald-600">{stats.success} ✓</span>
                              <span className="text-red-500">{stats.failure} ✗</span>
                              <span className="text-gray-500">({stats.total} total)</span>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-gray-800">Transaction Log</h3>
                  </CardHeader>
                  <CardBody className="p-0 max-h-96 overflow-auto">
                    <table className="w-full text-sm">
                      <thead className="bg-[#0a0a0f] text-xs text-gray-500 sticky top-0">
                        <tr>
                          <th className="text-left px-3 py-2">#</th>
                          <th className="text-left px-3 py-2">Payment ID</th>
                          <th className="text-left px-3 py-2">Gateway</th>
                          <th className="text-left px-3 py-2">Outcome</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-[#1c1c24]">
                        {simulationResults.map((res, idx) => (
                          <tr key={res.paymentId} className="hover:bg-[#0f0f16]">
                            <td className="px-3 py-2 text-gray-500">{idx + 1}</td>
                            <td className="px-3 py-2 font-mono text-xs">{res.paymentId.slice(-8)}</td>
                            <td className="px-3 py-2 font-medium">{res.decidedGateway}</td>
                            <td className="px-3 py-2">
                              <Badge variant={res.status === 'CHARGED' ? 'green' : 'red'}>
                                {res.status}
                              </Badge>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </CardBody>
                </Card>
              </>
            ) : (
              <Card>
                <CardBody className="py-16 text-center">
                  <Activity size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-gray-400 text-sm">Configure simulation parameters and click "Run Batch Simulation" to test Success Rate routing.</p>
                </CardBody>
              </Card>
            )
          ) : (
            result ? (
              <>
                <Card>
                  <CardBody>
                    <div className="flex items-start justify-between mb-3">
                      <div>
                        <p className="text-xs text-gray-500 uppercase tracking-wide mb-1">Decided Gateway</p>
                        <p className="text-3xl font-bold text-gray-900">{result.decided_gateway}</p>
                      </div>
                      <div className="text-right space-y-1">
                        <div>
                          <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${approachColor(result.routing_approach)}`}>
                            {result.routing_approach}
                          </span>
                        </div>
                        {result.is_scheduled_outage && <Badge variant="red">Scheduled Outage</Badge>}
                        {result.latency != null && (
                          <p className="text-xs text-gray-400">{result.latency}ms</p>
                        )}
                      </div>
                    </div>
                    {result.routing_dimension && (
                      <div className="flex gap-4 text-sm text-gray-600 border-t border-[#1c1c24] pt-3">
                        <div>
                          <span className="text-xs text-gray-400">Dimension</span>
                          <p className="font-medium">{result.routing_dimension}</p>
                        </div>
                        {result.routing_dimension_level && (
                          <div>
                            <span className="text-xs text-gray-400">Level</span>
                            <p className="font-medium">{result.routing_dimension_level}</p>
                          </div>
                        )}
                        <div>
                          <span className="text-xs text-gray-400">Reset</span>
                          <p className="font-medium">{result.reset_approach}</p>
                        </div>
                      </div>
                    )}
                  </CardBody>
                </Card>

                {scoreData.length > 0 && (
                  <Card>
                    <CardHeader>
                      <div className="flex items-center justify-between">
                        <h3 className="text-sm font-medium text-gray-800">Gateway Scores</h3>
                        <Button size="sm" variant="ghost" onClick={run} className="text-xs">
                          <RefreshCw size={12} /> Refresh
                        </Button>
                      </div>
                    </CardHeader>
                    <CardBody>
                      <ResponsiveContainer width="100%" height={scoreData.length * 40 + 20}>
                        <BarChart data={scoreData} layout="vertical" margin={{ left: 10, right: 30 }}>
                          <XAxis type="number" domain={[0, 100]} tickFormatter={v => `${v}%`} tick={{ fontSize: 11, fill: '#66667a' }} axisLine={{ stroke: '#1c1c24' }} tickLine={false} />
                          <YAxis type="category" dataKey="name" tick={{ fontSize: 12, fill: '#8e8ea0' }} width={60} axisLine={false} tickLine={false} />
                          <Tooltip formatter={v => `${v}%`} contentStyle={{ backgroundColor: '#0d0d12', border: '1px solid #1c1c24', borderRadius: '8px', color: '#e8e8f4' }} />
                          <Bar dataKey="score" radius={[0, 4, 4, 0]}>
                            {scoreData.map((entry, i) => (
                              <Cell
                                key={i}
                                fill={
                                  entry.name === result.decided_gateway
                                    ? '#0069ED'
                                    : entry.score < 30 ? '#ef4444'
                                    : entry.score < 60 ? '#f59e0b'
                                    : '#10b981'
                                }
                              />
                            ))}
                          </Bar>
                        </BarChart>
                      </ResponsiveContainer>
                    </CardBody>
                  </Card>
                )}

                {result.filter_wise_gateways && (
                  <Card>
                    <CardHeader>
                      <button
                        onClick={() => setFilterOpen(o => !o)}
                        className="flex items-center justify-between w-full text-sm font-medium text-gray-800"
                      >
                        Filter Chain
                        {filterOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                      </button>
                    </CardHeader>
                    {filterOpen && (
                      <CardBody className="space-y-2">
                        {Object.entries(result.filter_wise_gateways).map(([filter, gateways]) => (
                          <div key={filter} className="flex items-start gap-3">
                            <span className="text-xs font-mono bg-[#111118] text-gray-600 rounded-md px-2 py-0.5 mt-0.5 shrink-0 border border-[#1c1c24]">{filter}</span>
                            <div className="flex flex-wrap gap-1">
                              {Array.isArray(gateways)
                                ? gateways.map(gw => (
                                    <span key={gw} className="text-xs bg-blue-500/10 text-blue-400 ring-1 ring-inset ring-blue-500/20 rounded-md px-2 py-0.5">{gw}</span>
                                  ))
                                : <span className="text-xs text-gray-400">—</span>
                              }
                            </div>
                          </div>
                        ))}
                      </CardBody>
                    )}
                  </Card>
                )}

                <Card>
                  <CardHeader>
                    <button
                      onClick={() => setResponseOpen(o => !o)}
                      className="flex items-center justify-between w-full text-sm font-medium text-gray-800"
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
                      <pre className="text-xs text-gray-600 bg-[#0a0a0f] p-4 overflow-auto max-h-96 font-mono">
                        {JSON.stringify(result, null, 2)}
                      </pre>
                    </CardBody>
                  )}
                </Card>
              </>
            ) : (
              <Card>
                <CardBody className="py-16 text-center">
                  <Play size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-gray-400 text-sm">Fill in the parameters and click "Run Decision" to see the routing result.</p>
                </CardBody>
              </Card>
            )
          )}
        </div>
      </div>
    </div>
  )
}
