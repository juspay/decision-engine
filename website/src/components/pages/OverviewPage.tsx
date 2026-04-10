import { useNavigate } from 'react-router-dom'
import { useEffect, useState } from 'react'
import useSWR from 'swr'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm, RuleConfig } from '../../types/api'
import { CheckCircle, XCircle, AlertCircle } from 'lucide-react'

function useHealth() {
  const [status, setStatus] = useState<'up' | 'down' | 'loading'>('loading')
  useEffect(() => {
    console.log(`\n[HEALTH CHECK] ${new Date().toISOString()}`)
    console.log('Fetching: GET /health')
    
    fetch('/health')
      .then((r) => {
        console.log(`[HEALTH CHECK] Response: ${r.status} ${r.statusText}`)
        setStatus(r.ok ? 'up' : 'down')
      })
      .catch((err) => {
        console.log(`[HEALTH CHECK ERROR] ${err.message}`)
        setStatus('down')
      })
  }, [])
  return status
}

export function OverviewPage() {
  const navigate = useNavigate()
  const { merchantId } = useMerchantStore()
  const health = useHealth()

  const { data: activeAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`),
    { shouldRetryOnError: false }
  )

  const { data: srConfig, error: srError } = useSWR<RuleConfig>(
    merchantId ? [`/rule/get`, 'successRate', merchantId] : null,
    () =>
      apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'successRate' })
  )

  const activeRouting =
    activeAlgorithms && activeAlgorithms.length > 0 ? activeAlgorithms[0] : null

  const hasRuleBasedRouting = (activeAlgorithms || []).some(a => 
    (a.algorithm_data || a.algorithm)?.type === 'advanced'
  )

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-slate-900">Overview</h1>
        <p className="text-sm text-slate-500 mt-1">
          Decision Engine routing health and status
        </p>
      </div>

      {!merchantId && (
        <div className="rounded-lg border border-yellow-200 bg-yellow-50 px-4 py-3 flex items-center gap-2 text-sm text-yellow-800">
          <AlertCircle size={16} />
          Set your Merchant ID in the top bar to load configuration.
        </div>
      )}

      {/* Health status */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardBody className="flex items-center gap-3">
            {health === 'up' ? (
              <CheckCircle className="text-green-500" size={24} />
            ) : health === 'down' ? (
              <XCircle className="text-red-500" size={24} />
            ) : (
              <div className="w-6 h-6 rounded-full border-2 border-gray-200 border-t-gray-500 animate-spin" />
            )}
            <div>
              <p className="text-xs text-slate-500">API Health</p>
              <p className="text-sm font-medium">
                {health === 'up' ? 'Healthy' : health === 'down' ? 'Down' : 'Checking...'}
              </p>
            </div>
          </CardBody>
        </Card>

        <Card 
          className="cursor-pointer hover:border-brand-300 transition-all"
          onClick={() => navigate('/routing')}
        >
          <CardBody>
            <p className="text-xs text-slate-500 mb-1">Active Routing Rule</p>
            {!merchantId ? (
              <Badge variant="gray">Not set</Badge>
            ) : activeRouting ? (
              <div>
                <Badge variant="green">Active</Badge>
                <p className="text-sm font-medium mt-1 truncate">{activeRouting.name}</p>
                <p className="text-xs text-slate-400">{(activeRouting.algorithm_data || activeRouting.algorithm)?.type}</p>
              </div>
            ) : (
              <Badge variant="gray">Not Configured</Badge>
            )}
          </CardBody>
        </Card>

        <Card 
          className="cursor-pointer hover:border-brand-300 transition-all"
          onClick={() => navigate('/routing/sr')}
        >
          <CardBody>
            <p className="text-xs text-slate-500 mb-1">Auth-Rate Config</p>
            {!merchantId ? (
              <Badge variant="gray">Not set</Badge>
            ) : srError ? (
              <Badge variant="gray">Not Configured</Badge>
            ) : srConfig?.data ? (
              <Badge variant="green">Configured</Badge>
            ) : (
              <Badge variant="gray">Not Configured</Badge>
            )}
          </CardBody>
        </Card>

        <Card 
          className="cursor-pointer hover:border-brand-300 transition-all"
          onClick={() => navigate('/routing/rules')}
        >
          <CardBody>
            <p className="text-xs text-slate-500 mb-1">Rule-Based Routing</p>
            {!merchantId ? (
              <Badge variant="gray">Not set</Badge>
            ) : hasRuleBasedRouting ? (
              <Badge variant="green">Configured</Badge>
            ) : (
              <Badge variant="gray">Not Configured</Badge>
            )}
          </CardBody>
        </Card>
      </div>

      {/* Active algorithm detail */}
      {activeRouting && (
        <Card 
          className="cursor-pointer hover:border-brand-300 transition-all"
          onClick={() => navigate('/routing')}
        >
          <CardHeader>
            <h2 className="text-sm font-semibold text-slate-800">Active Routing Configuration</h2>
          </CardHeader>
          <CardBody>
            <dl className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <dt className="text-slate-500">Name</dt>
                <dd className="font-medium">{activeRouting.name}</dd>
              </div>
              <div>
                <dt className="text-slate-500">Type</dt>
                <dd className="font-medium capitalize">{(activeRouting.algorithm_data || activeRouting.algorithm)?.type}</dd>
              </div>
              <div>
                <dt className="text-slate-500">Algorithm For</dt>
                <dd className="font-medium capitalize">{activeRouting.algorithm_for}</dd>
              </div>
              <div>
                <dt className="text-slate-500">ID</dt>
                <dd className="font-mono text-xs text-slate-600">{activeRouting.id}</dd>
              </div>
            </dl>
          </CardBody>
        </Card>
      )}
    </div>
  )
}
