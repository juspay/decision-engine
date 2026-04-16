import { useState } from 'react'
import useSWR from 'swr'
import { PieChart, Pie, Cell, Tooltip, Legend, ResponsiveContainer } from 'recharts'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { Plus, Trash2, Eye } from 'lucide-react'
import { validateVolumeSplitRule } from '../../features/routing/volumeSplit/schema'
import { toVolumeSplitCreatePayload } from '../../features/routing/volumeSplit/payload'
import { toVolumeSplitRuleDetailsState } from '../../features/routing/volumeSplit/state'
import { VolumeSplitGatewayFormEntry } from '../../features/routing/volumeSplit/types'

const COLORS = ['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899']

function makeId() { return Math.random().toString(36).slice(2) }

export function VolumeSplitPage() {
  const { merchantId } = useMerchantStore()

  const { data: active, mutate } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['active-routing', merchantId] : null,
    () => apiPost(`/routing/list/active/${merchantId}`)
  )

  const activeVol = active?.find(r => (r.algorithm_data || r.algorithm)?.type === 'volume_split')

  const [gateways, setGateways] = useState<VolumeSplitGatewayFormEntry[]>([
    { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
    { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
  ])
  const [ruleName, setRuleName] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)
  const [showCurrentConfig, setShowCurrentConfig] = useState(false)
  const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set())

  const total = gateways.reduce((s, g) => s + g.split, 0)

  function updateGateway(id: string, field: 'gatewayName' | 'gatewayId' | 'split', val: string | number) {
    setGateways(gs => gs.map(g => g.id === id ? { ...g, [field]: val } : g))
  }

  function addGateway() {
    setGateways(gs => [...gs, { id: makeId(), gatewayName: '', gatewayId: '', split: 0 }])
  }

  function removeGateway(id: string) {
    setGateways(gs => gs.filter(g => g.id !== id))
  }

  async function handleCreate() {
    if (!merchantId) return setError('Set a merchant ID first')
    const validationError = validateVolumeSplitRule({ ruleName, gateways })
    if (validationError) return setError(validationError)

    setSaving(true); setError(null); setSuccess(null)
    try {
      const payload = toVolumeSplitCreatePayload({ ruleName, gateways }, merchantId)
      await apiPost('/routing/create', payload)
      setSuccess(`Rule "${ruleName}" created successfully. Find it in the list below to activate.`)
      mutate()
      setRuleName('')
      setGateways([
        { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
        { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
      ])
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to create rule')
    } finally {
      setSaving(false)
    }
  }

  async function handleActivate(ruleId: string) {
    if (!merchantId) return
    try {
      await apiPost('/routing/activate', { created_by: merchantId, routing_algorithm_id: ruleId })
      mutate()
      setSuccess('Rule activated.')
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to activate')
    }
  }

  function toggleRuleExpand(id: string) {
    setExpandedRuleIds(prev => {
      const newSet = new Set(prev)
      if (newSet.has(id)) {
        newSet.delete(id)
      } else {
        newSet.add(id)
      }
      return newSet
    })
  }

  // Build pie data from active rule
  const algo = activeVol ? (activeVol.algorithm_data || activeVol.algorithm) : null
  const pieData = algo && 'data' in algo
    ? (algo.data as { split: number; output: { gateway_name: string; gateway_id: string | null } }[]).map(item => ({
        name: `${item.output?.gateway_name ?? '?'}${item.output?.gateway_id ? ` (${item.output.gateway_id})` : ''}`,
        value: item.split,
      }))
    : []

  return (
    <div className="space-y-6 max-w-4xl">
      <div>
        <h1 className="text-2xl font-bold text-slate-900">Volume Split Routing</h1>
        <p className="text-slate-500 mt-1 text-sm">Distribute payment traffic across gateways by percentage.</p>
      </div>

      {/* Active Configuration */}
      {activeVol && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold text-slate-800">Active Volume Split</h2>
              <p className="text-xs text-slate-500 mt-0.5">{activeVol.name}</p>
            </div>
            <div className="flex items-center gap-2">
              <Badge variant="green">Active</Badge>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => setShowCurrentConfig(!showCurrentConfig)}
              >
                <Eye size={14} className="mr-1" />
                {showCurrentConfig ? 'Hide' : 'View'}
              </Button>
            </div>
          </CardHeader>
          {showCurrentConfig && (
            <CardBody>
              <ResponsiveContainer width="100%" height={220}>
                <PieChart>
                  <Pie data={pieData} dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={80} label={({ name, value }) => `${name}: ${value}%`} labelLine={{ stroke: '#45454f' }}>
                    {pieData.map((_, i) => (
                      <Cell key={i} fill={COLORS[i % COLORS.length]} />
                    ))}
                  </Pie>
                  <Tooltip formatter={(v) => `${v}%`} contentStyle={{ backgroundColor: '#0d0d12', border: '1px solid #1c1c24', borderRadius: '8px', color: '#e8e8f4' }} />
                  <Legend wrapperStyle={{ color: '#8e8ea0' }} />
                </PieChart>
              </ResponsiveContainer>
              <div className="mt-4 text-xs text-slate-600">
                <p><strong>Rule ID:</strong> {activeVol.id}</p>
                <p><strong>Created:</strong> {activeVol.created_at ? new Date(activeVol.created_at).toLocaleString() : 'Unknown'}</p>
              </div>
            </CardBody>
          )}
        </Card>
      )}

      {/* Create Rule */}
      <Card>
        <CardHeader>
          <h2 className="font-medium text-slate-800">Create Volume Split Rule</h2>
        </CardHeader>
        <CardBody className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">Rule Name</label>
            <input
              value={ruleName}
              onChange={e => setRuleName(e.target.value)}
              placeholder="e.g. ab-test-split"
              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm w-64 focus:outline-none focus:ring-1 focus:ring-brand-500"
            />
          </div>

          <div className="space-y-2">
            <div className="grid grid-cols-[1fr_1fr_100px_32px] gap-2 text-xs font-medium text-slate-500 px-1">
              <span>Gateway Name</span>
              <span>Gateway ID</span>
              <span>Split %</span>
              <span />
            </div>
            {gateways.map(g => (
              <div key={g.id} className="grid grid-cols-[1fr_1fr_100px_32px] gap-2 items-center">
                <input
                  value={g.gatewayName}
                  onChange={e => updateGateway(g.id, 'gatewayName', e.target.value)}
                  placeholder="e.g. stripe"
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <input
                  value={g.gatewayId}
                  onChange={e => updateGateway(g.id, 'gatewayId', e.target.value)}
                  placeholder="optional gateway_id"
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <input
                  type="number"
                  min={0}
                  max={100}
                  value={g.split}
                  onChange={e => updateGateway(g.id, 'split', Number(e.target.value))}
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <button onClick={() => removeGateway(g.id)} className="text-slate-400 hover:text-red-500">
                  <Trash2 size={15} />
                </button>
              </div>
            ))}
            <div className="flex items-center gap-3">
              <button onClick={addGateway} className="flex items-center gap-1 text-sm text-brand-500 hover:text-brand-600">
                <Plus size={14} /> Add Gateway
              </button>
              <span className={`text-xs font-medium ${total === 100 ? 'text-emerald-400' : 'text-red-400'}`}>
                Total: {total}%{total !== 100 && ' (must be 100)'}
              </span>
            </div>
          </div>

          <ErrorMessage error={error} />
          {success && <p className="text-sm text-emerald-400">{success}</p>}

          <Button onClick={handleCreate} disabled={saving || !merchantId}>
            {saving ? <><Spinner size={14} /> Creating…</> : 'Create Rule'}
          </Button>
        </CardBody>
      </Card>

      <ActiveRulesList
        merchantId={merchantId}
        onActivate={handleActivate}
        expandedRuleIds={expandedRuleIds}
        onToggleExpand={toggleRuleExpand}
      />
    </div>
  )
}

function ActiveRulesList({
  merchantId,
  onActivate,
  expandedRuleIds,
  onToggleExpand
}: {
  merchantId: string;
  onActivate: (id: string) => void;
  expandedRuleIds: Set<string>;
  onToggleExpand: (id: string) => void;
}) {
  const { data: rules, isLoading } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['routing-list', merchantId] : null,
    () => apiPost(`/routing/list/${merchantId}`)
  )

  const volRules = rules?.filter(r => (r.algorithm_data || r.algorithm)?.type === 'volume_split') ?? []

  if (!merchantId) return null
  if (isLoading) return <div className="flex justify-center py-4"><Spinner /></div>
  if (!volRules.length) return null

  return (
    <Card>
      <CardHeader><h2 className="font-medium text-slate-800">Saved Volume Split Rules</h2></CardHeader>
      <CardBody className="p-0">
        <table className="w-full text-sm">
          <thead className="bg-slate-50 dark:bg-[#0a0a0f] text-xs text-slate-500 uppercase tracking-wider">
            <tr>
              <th className="text-left px-4 py-2">Name</th>
              <th className="text-left px-4 py-2">Split</th>
              <th className="px-4 py-2" />
            </tr>
          </thead>
          <tbody className="divide-y divide-[#1c1c24]">
            {volRules.map(r => {
              const details = toVolumeSplitRuleDetailsState(r)
              const itemText = details?.gateways.map(i => `${i.gatewayName}${i.gatewayId ? `(${i.gatewayId})` : ''}:${i.split}%`).join(' | ') || ''
              const algorithm = r.algorithm_data || r.algorithm
              const isExpanded = expandedRuleIds.has(r.id)
              return (
                <>
                  <tr key={r.id} className="hover:bg-slate-100 dark:bg-[#0f0f16] transition-colors">
                    <td className="px-4 py-2 font-medium text-slate-800">{r.name}</td>
                    <td className="px-4 py-2 text-slate-600 text-xs">
                      {itemText}
                    </td>
                    <td className="px-4 py-2 text-right">
                      <div className="flex items-center justify-end gap-2">
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => onToggleExpand(r.id)}
                        >
                          <Eye size={14} className="mr-1" />
                          {isExpanded ? 'Hide' : 'View'}
                        </Button>
                        <Button size="sm" variant="secondary" onClick={() => onActivate(r.id)}>
                          Activate
                        </Button>
                      </div>
                    </td>
                  </tr>
                  {isExpanded && (
                    <tr>
                      <td colSpan={3} className="px-4 py-3 bg-slate-50 dark:bg-[#151518]">
                        <div className="text-xs text-slate-600 space-y-2">
                          <p><strong>ID:</strong> {r.id}</p>
                          <p><strong>Description:</strong> {r.description || 'N/A'}</p>
                          {r.created_at && (
                            <p><strong>Created:</strong> {new Date(r.created_at).toLocaleString()}</p>
                          )}
                          <div>
                            <strong>Configuration:</strong>
                            <pre className="mt-1 p-2 bg-slate-100 dark:bg-[#0f0f11] border border-transparent dark:border-[#222226] rounded text-xs overflow-auto max-h-48">
                              {JSON.stringify(algorithm, null, 2)}
                            </pre>
                          </div>
                        </div>
                      </td>
                    </tr>
                  )}
                </>
              )
            })}
          </tbody>
        </table>
      </CardBody>
    </Card>
  )
}
