import { useState } from 'react'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from 'recharts'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { DecideGatewayResponse } from '../../types/api'
import { ROUTING_APPROACH_COLORS } from '../../lib/constants'
import { Play, RefreshCw, ChevronDown, ChevronUp } from 'lucide-react'

const PAYMENT_METHOD_TYPES = ['CARD', 'UPI', 'WALLET', 'NETBANKING']
const PAYMENT_METHODS = ['CREDIT', 'DEBIT', 'PREPAID']
const CURRENCIES = ['USD', 'EUR', 'GBP', 'INR', 'SGD', 'AUD', 'CAD']
const CARD_BRANDS = ['VISA', 'MASTERCARD', 'AMEX', 'RUPAY', 'DINERS']
const AUTH_TYPES = ['THREE_DS', 'NO_THREE_DS']
const ALGORITHMS = ['SrBasedRouting', 'PlBasedRouting', 'NtwBasedRouting']

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

function approachColor(approach: string): string {
  for (const [k, v] of Object.entries(ROUTING_APPROACH_COLORS)) {
    if (approach.includes(k) || k.includes(approach)) return v
  }
  return 'bg-white/5 text-gray-600 ring-1 ring-inset ring-white/8'
}

export function DecisionExplorerPage() {
  const { merchantId } = useMerchantStore()
  const [form, setForm] = useState<FormState>({
    amount: '1000',
    currency: 'USD',
    payment_method_type: 'CARD',
    payment_method: 'CREDIT',
    card_brand: 'VISA',
    auth_type: 'THREE_DS',
    eligible_gateways: 'stripe, paypal, adyen',
    ranking_algorithm: 'SrBasedRouting',
    elimination_enabled: false,
  })
  const [result, setResult] = useState<DecideGatewayResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [filterOpen, setFilterOpen] = useState(false)

  function set(field: keyof FormState, value: string | boolean) {
    setForm(f => ({ ...f, [field]: value }))
  }

  async function run() {
    if (!merchantId) return setError('Set a merchant ID in the top bar')
    setLoading(true); setError(null)
    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    try {
      const res = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        merchant_id: merchantId,
        payment_info: {
          payment_id: `explorer_${Date.now()}`,
          amount: parseFloat(form.amount) || 1000,
          currency: form.currency,
          payment_type: 'ORDER_PAYMENT',
          payment_method_type: form.payment_method_type,
          payment_method: form.payment_method,
          auth_type: form.auth_type,
          card_brand: form.card_brand,
        },
        eligible_gateway_list: gateways,
        ranking_algorithm: form.ranking_algorithm,
        elimination_enabled: form.elimination_enabled,
      })
      setResult(res)
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

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-gray-900">Decision Explorer</h1>
        <p className="text-gray-500 mt-1 text-sm">
          Test a payment against the routing engine and understand exactly why a gateway was chosen.
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Input Form */}
        <Card>
          <CardHeader><h2 className="font-medium text-gray-800">Payment Parameters</h2></CardHeader>
          <CardBody className="space-y-3">
            {!merchantId && (
              <p className="text-xs text-amber-600 bg-amber-50 border border-amber-200 rounded px-3 py-2">
                Set a merchant ID in the top bar first.
              </p>
            )}

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
                placeholder="stripe, paypal, adyen"
                className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500" />
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-600 mb-1">Algorithm</label>
                <select value={form.ranking_algorithm} onChange={e => set('ranking_algorithm', e.target.value)}
                  className="w-full border border-gray-300 rounded px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                  {ALGORITHMS.map(a => <option key={a}>{a}</option>)}
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

            <ErrorMessage error={error} />

            <Button onClick={run} disabled={loading || !merchantId} className="w-full justify-center">
              {loading ? <><Spinner size={14} /> Running…</> : <><Play size={14} /> Run Decision</>}
            </Button>
          </CardBody>
        </Card>

        {/* Results */}
        <div className="space-y-4">
          {result ? (
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
            </>
          ) : (
            <Card>
              <CardBody className="py-16 text-center">
                <Play size={32} className="text-gray-300 mx-auto mb-3" />
                <p className="text-gray-400 text-sm">Fill in the parameters and click "Run Decision" to see the routing result.</p>
              </CardBody>
            </Card>
          )}
        </div>
      </div>
    </div>
  )
}
