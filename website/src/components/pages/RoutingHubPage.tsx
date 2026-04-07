import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { Card, CardBody } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { TrendingUp, Layers, PieChart, CreditCard, Shield } from 'lucide-react'

interface RoutingCard {
  id: string
  title: string
  description: string
  icon: React.ElementType
  route: string
  algorithmType: string
}

const ROUTING_CARDS: RoutingCard[] = [
  {
    id: 'sr',
    title: 'Auth-Rate Based Routing',
    description: 'Dynamically route to the best-performing gateway based on real-time authorization rates.',
    icon: TrendingUp,
    route: '/routing/sr',
    algorithmType: 'successRate',
  },
  {
    id: 'rules',
    title: 'Rule-Based Routing',
    description: 'Declarative Euclid DSL rules to route payments based on conditions and attributes.',
    icon: Layers,
    route: '/routing/rules',
    algorithmType: 'advanced',
  },
  {
    id: 'volume',
    title: 'Volume Split',
    description: 'Distribute payment traffic across gateways by configurable percentage splits.',
    icon: PieChart,
    route: '/routing/volume',
    algorithmType: 'volume_split',
  },
  {
    id: 'debit',
    title: 'Network Routing',
    description: 'Optimise debit network fees with acquirer-aware network-based routing.',
    icon: CreditCard,
    route: '/routing/debit',
    algorithmType: 'debitRouting',
  },
  {
    id: 'fallback',
    title: 'Fallback',
    description: 'Configure a single gateway fallback for guaranteed payment processing.',
    icon: Shield,
    route: '/routing/rules',
    algorithmType: 'single',
  },
]

export function RoutingHubPage() {
  const navigate = useNavigate()
  const { merchantId } = useMerchantStore()

  const { data: activeAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const { data: allAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )

  const activeIds = new Set((activeAlgorithms || []).map((a) => a.algorithm?.type))
  const activeAlgo = activeAlgorithms?.[0]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-gray-900">Routing Hub</h1>
        <p className="text-sm text-gray-500 mt-1">
          Choose and configure your payment routing strategy
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {ROUTING_CARDS.map((card) => {
          const Icon = card.icon
          const isActive = activeIds.has(card.algorithmType as "priority" | "volume_split" | "single" | "advanced")
          return (
            <Card key={card.id} className="flex flex-col hover:border-[#28282f] transition-colors">
              <CardBody className="flex-1 flex flex-col gap-3">
                <div className="flex items-start justify-between">
                  <div className="p-2 bg-brand-50 rounded-lg border border-[#1c2d50]">
                    <Icon size={20} className="text-brand-500" />
                  </div>
                  <Badge variant={isActive ? 'green' : 'gray'}>
                    {isActive ? 'Active' : 'Not Configured'}
                  </Badge>
                </div>
                <div>
                  <h3 className="font-semibold text-gray-900">{card.title}</h3>
                  <p className="text-sm text-gray-500 mt-1">{card.description}</p>
                </div>
                <div className="mt-auto pt-2">
                  <Button
                    variant={isActive ? 'secondary' : 'primary'}
                    size="sm"
                    onClick={() => navigate(card.route)}
                  >
                    {isActive ? 'Manage' : 'Setup'}
                  </Button>
                </div>
              </CardBody>
            </Card>
          )
        })}
      </div>

      {/* Active Configuration Section */}
      {activeAlgo && (
        <Card>
          <CardBody>
            <h2 className="text-sm font-semibold text-gray-800 mb-3">Active Configuration</h2>
            <div className="flex items-center gap-4 flex-wrap">
              <div>
                <span className="text-xs text-gray-500">Rule Name</span>
                <p className="text-sm font-medium">{activeAlgo.name}</p>
              </div>
              <div>
                <span className="text-xs text-gray-500">Type</span>
                <p className="text-sm font-medium capitalize">{activeAlgo.algorithm?.type}</p>
              </div>
              <div>
                <span className="text-xs text-gray-500">Algorithm For</span>
                <p className="text-sm font-medium capitalize">{activeAlgo.algorithm_for}</p>
              </div>
            </div>
          </CardBody>
        </Card>
      )}

      {/* All configured algorithms */}
      {allAlgorithms && allAlgorithms.length > 0 && (
        <Card>
          <CardBody>
            <h2 className="text-sm font-semibold text-gray-800 mb-3">All Routing Algorithms</h2>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-xs text-gray-500 border-b border-[#1c1c24]">
                    <th className="pb-2 pr-4">Name</th>
                    <th className="pb-2 pr-4">Type</th>
                    <th className="pb-2 pr-4">For</th>
                    <th className="pb-2">Status</th>
                  </tr>
                </thead>
                <tbody>
                  {allAlgorithms.map((algo) => {
                    const isActiveAlgo = (activeAlgorithms || []).some(
                      (a) => a.id === algo.id
                    )
                    return (
                      <tr key={algo.id} className="border-b border-[#16161c] hover:bg-[#0f0f16] transition-colors">
                        <td className="py-2 pr-4 font-medium">{algo.name}</td>
                        <td className="py-2 pr-4 capitalize">{algo.algorithm?.type}</td>
                        <td className="py-2 pr-4 capitalize">{algo.algorithm_for}</td>
                        <td className="py-2">
                          <Badge variant={isActiveAlgo ? 'green' : 'gray'}>
                            {isActiveAlgo ? 'Active' : 'Inactive'}
                          </Badge>
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          </CardBody>
        </Card>
      )}
    </div>
  )
}
